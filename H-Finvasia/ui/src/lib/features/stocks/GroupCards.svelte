<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import {
    api,
    events,
    type ContainerCardDto,
    type ContainerRowDto,
    type InstrumentDto,
    type QuoteDto,
  } from "../../api";
  import InstrumentMenu from "../../components/InstrumentMenu.svelte";
  import {
    instrumentDetail,
    instrumentKind,
    mergeOrder,
    moveRelative,
    nextCardOrder,
    sortInstrumentRows,
  } from "./cards";
  import {
    loadCollapsedGroups,
    loadGroupSort,
    saveCollapsedGroups,
    saveGroupSort,
    type SortMode,
  } from "../../layoutPrefs";

  interface Props {
    cards: ContainerCardDto[];
    active: string | null;
    sortMode: SortMode;
    onactivate: (name: string) => void;
    onadd: (group: string, key: string) => Promise<void>;
    onremove: (group: string, key: string) => Promise<void>;
    onreorder: (order: string[]) => Promise<void>;
    onrename: (oldName: string, newName: string) => Promise<void>;
    oncreate: (name: string) => Promise<void>;
    ondelete: (name: string) => Promise<void>;
    onreorderkeys: (group: string, keys: string[]) => Promise<void>;
    onactive: (key: string, label: string) => void;
    onbuy: (key: string) => void;
    onsell: (key: string) => void;
    onchain: (underlying: string) => void;
    search?: (text: string, limit: number) => Promise<InstrumentDto[]>;
  }
  let {
    cards,
    active,
    sortMode,
    onactivate,
    onadd,
    onremove,
    onreorder,
    onrename,
    oncreate,
    ondelete,
    onreorderkeys,
    onactive,
    onbuy,
    onsell,
    onchain,
    search = api.search,
  }: Props = $props();

  let order = $state<string[]>([]);
  let collapsed = $state<Record<string, boolean>>(loadCollapsedGroups());
  let cardSort = $state<Record<string, SortMode>>(loadGroupSort());
  let quotes = $state<Map<string, QuoteDto>>(new Map());
  let keySet = $state<Set<string>>(new Set());
  let error = $state<string | null>(null);
  let editing = $state<string | null>(null);
  let editingDraft = $state("");
  let creating = $state(false);
  let createDraft = $state("");
  let rowDragKey = $state<string | null>(null);
  let rowDragGroup = $state<string | null>(null);
  let armedKey = $state<string | null>(null);
  let dragName = $state<string | null>(null);
  let dropTarget = $state<string | null>(null);
  let dragMoved = false;
  let rowMenu = $state<{ x: number; y: number; row: ContainerRowDto; group: string } | null>(null);

  let searchFor = $state<string | null>(null);
  let query = $state("");
  let results = $state<InstrumentDto[]>([]);
  let searching = $state(false);
  let adding = $state(false);
  let searchError = $state<string | null>(null);
  let searchSeq = 0;

  const ordered = $derived.by(() => {
    const byName = new Map(cards.map((card) => [card.name, card]));
    return mergeOrder(order, cards.map((card) => card.name))
      .map((name) => byName.get(name))
      .filter((card): card is ContainerCardDto => card !== undefined);
  });

  // Follow the backend order whenever the card set/order changes (including a
  // rename). A local drag order survives refreshes until the drag is dropped.
  let lastCardSignature = "";
  $effect(() => {
    const names = cards.map((card) => card.name);
    const dragging = untrack(() => dragName) !== null;
    if (dragging) return;
    const signature = names.join("\u0000");
    if (signature === lastCardSignature) return;
    lastCardSignature = signature;
    order = nextCardOrder(order, names, false);
  });

  // Refresh the live-key index and drop quote overrides for instruments that
  // left every group, so the map cannot grow for the lifetime of the tab.
  $effect(() => {
    const allowed = new Set(cards.flatMap((card) => card.rows.map((row) => row.key)));
    keySet = allowed;
    untrack(() => {
      const stale = [...quotes.keys()].filter((key) => !allowed.has(key));
      if (stale.length > 0) {
        const next = new Map(quotes);
        for (const key of stale) next.delete(key);
        quotes = next;
      }
    });
  });

  // Debounced, sequence-guarded search; the timer is cleaned up per re-run.
  $effect(() => {
    const group = searchFor;
    const text = query.trim();
    if (!group || text.length === 0) {
      results = [];
      searching = false;
      searchError = null;
      return;
    }
    const seq = ++searchSeq;
    searching = true;
    searchError = null;
    const timer = setTimeout(async () => {
      try {
        const hits = await search(text, 20);
        if (seq === searchSeq) results = hits;
      } catch (e) {
        if (seq === searchSeq) {
          results = [];
          searchError = String(e);
        }
      } finally {
        if (seq === searchSeq) searching = false;
      }
    }, 170);
    return () => clearTimeout(timer);
  });

  onMount(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const off = await events.onDelta((delta) => {
          if (disposed) return;
          let changed = false;
          const next = new Map(quotes);
          for (const quote of delta.quotes) {
            if (keySet.has(quote.key)) {
              next.set(quote.key, quote);
              changed = true;
            }
          }
          if (changed) quotes = next;
        });
        if (disposed) off();
        else unlisten = off;
      } catch (e) {
        if (!disposed) error = String(e);
      }
    })();
    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  function focusOnMount(node: HTMLInputElement) {
    queueMicrotask(() => node.focus());
    return {};
  }

  function focusWhen(node: HTMLInputElement, active: boolean) {
    if (active) queueMicrotask(() => node.focus());
    return {
      update(next: boolean) {
        if (next) queueMicrotask(() => node.focus());
      },
    };
  }

  function modeFor(name: string): SortMode {
    return cardSort[name] ?? sortMode;
  }

  function setCardSort(name: string, mode: SortMode) {
    cardSort = { ...cardSort, [name]: mode };
    saveGroupSort(cardSort);
  }

  async function commitRename(oldName: string) {
    const next = editingDraft.trim();
    if (editing !== oldName) return;
    editing = null;
    editingDraft = "";
    if (next === "" || next === oldName) return;
    error = null;
    try {
      await onrename(oldName, next);
    } catch (e) {
      error = String(e);
    }
  }

  async function deleteCard(name: string, event: MouseEvent) {
    event.stopPropagation();
    if (!window.confirm(`Delete card "${name}"? Its instrument list is removed from the card view.`)) {
      return;
    }
    error = null;
    try {
      await ondelete(name);
    } catch (e) {
      error = String(e);
    }
  }

  async function createCard() {
    const name = createDraft.trim();
    if (name === "") return;
    error = null;
    try {
      await oncreate(name);
      creating = false;
      createDraft = "";
    } catch (e) {
      error = String(e);
    }
  }

  function arm(row: ContainerRowDto) {
    armedKey = row.key;
    onactive(row.key, row.symbol);
  }

  function startRowDrag(event: DragEvent, group: string, key: string) {
    if (modeFor(group) !== "custom") return;
    event.stopPropagation();
    rowDragKey = key;
    rowDragGroup = group;
    event.dataTransfer?.setData("text/plain", key);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  }

  function endRowDrag() {
    rowDragKey = null;
    rowDragGroup = null;
  }

  function dropRow(
    event: DragEvent,
    group: string,
    targetKey: string,
    orderedKeys: string[],
  ) {
    event.preventDefault();
    event.stopPropagation();
    if (rowDragGroup !== group || !rowDragKey || rowDragKey === targetKey) {
      endRowDrag();
      return;
    }
    const next = moveRelative(orderedKeys, rowDragKey, targetKey);
    endRowDrag();
    onreorderkeys(group, next).catch((e) => {
      error = String(e);
    });
  }

  function quoteFor(row: ContainerRowDto): QuoteDto | null {
    return quotes.get(row.key) ?? row.quote;
  }

  function isCollapsed(name: string): boolean {
    return collapsed[name] === true;
  }

  function toggleCollapse(name: string, event: MouseEvent) {
    event.stopPropagation();
    collapsed = { ...collapsed, [name]: !isCollapsed(name) };
    saveCollapsedGroups(collapsed);
  }

  async function openSearch(name: string, event: MouseEvent) {
    event.stopPropagation();
    searchFor = name;
    query = "";
    results = [];
    searchError = null;
    await tick();
    const input = [...document.querySelectorAll<HTMLInputElement>("input[data-card-search]")].find(
      (node) => node.dataset.cardSearch === name,
    );
    input?.focus();
  }

  function closeSearch() {
    searchFor = null;
    query = "";
    results = [];
    searchError = null;
    searching = false;
  }

  async function addInstrument(group: string, item: InstrumentDto) {
    if (adding) return;
    adding = true;
    searchError = null;
    try {
      await onadd(group, item.key);
      closeSearch();
    } catch (e) {
      searchError = String(e);
    } finally {
      adding = false;
    }
  }

  async function removeInstrument(group: string, row: ContainerRowDto) {
    error = null;
    try {
      await onremove(group, row.key);
    } catch (e) {
      error = String(e);
    }
  }

  function startDrag(event: DragEvent, name: string) {
    dragName = name;
    dragMoved = false;
    event.dataTransfer?.setData("text/plain", name);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  }

  function dragOver(event: DragEvent, target: string) {
    if (!dragName || dragName === target) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    const next = moveRelative(order, dragName, target);
    if (next.join("\u0000") !== order.join("\u0000")) {
      order = next;
      dragMoved = true;
    }
    dropTarget = target;
  }

  function drop(event: DragEvent) {
    event.preventDefault();
    dropTarget = null;
  }

  function endDrag() {
    const changed = dragName !== null && dragMoved;
    dragName = null;
    dropTarget = null;
    dragMoved = false;
    if (changed) {
      onreorder([...order]).catch((e) => {
        error = String(e);
      });
    }
  }
</script>

<div class="group-cards" role="list">
  {#if error}
    <div class="banner group-cards-error">
      <span>{error}</span>
      <button class="ghost" onclick={() => (error = null)}>Dismiss</button>
    </div>
  {/if}

  {#each ordered as card (card.name)}
    {@const mode = modeFor(card.name)}
    {@const rowList = sortInstrumentRows(
      card.rows,
      mode,
      card.rows.map((row) => row.key),
      card.custom_order,
    )}
    {@const hasLive = card.rows.some((row) => row.live || quoteFor(row) !== null)}
    <article
      class="group-card"
      class:active={card.name === active}
      class:dragging={dragName === card.name}
      class:drop-target={dropTarget === card.name}
      role="listitem"
      ondragover={(event) => dragOver(event, card.name)}
      ondrop={drop}
    >
      <header
        class="group-card-head"
        role="group"
        aria-label={`${card.name} group card; drag to reorder`}
        draggable="true"
        title="Drag to reorder this card"
        ondragstart={(event) => startDrag(event, card.name)}
        ondragend={endDrag}
      >
        <span class="group-card-dot" style="background: {card.color ?? 'var(--accent-2)'}"></span>
        {#if editing === card.name}
          <input
            class="group-card-edit"
            use:focusOnMount
            bind:value={editingDraft}
            aria-label={`Rename ${card.name}`}
            onclick={(event) => event.stopPropagation()}
            onkeydown={(event) => {
              if (event.key === "Enter") void commitRename(card.name);
              if (event.key === "Escape") {
                editing = null;
                editingDraft = "";
              }
            }}
            onblur={() => void commitRename(card.name)}
          />
        {:else}
          <button
            class="group-card-name"
            type="button"
            title={`${card.name === active ? "Active group" : "Switch to this group"}; double-click to rename`}
            onclick={() => onactivate(card.name)}
            ondblclick={(event) => {
              event.stopPropagation();
              editing = card.name;
              editingDraft = card.name;
            }}
          >
            {card.name}
          </button>
        {/if}
        <span class="group-card-count" title="Instruments in this group">{card.count}</span>
        {#if hasLive}<span class="group-card-live">live</span>{/if}
        <span class="spacer"></span>
        <select
          class="card-sort"
          aria-label={`Sort ${card.name}`}
          value={mode}
          title={`Sort ${card.name}`}
          onclick={(event) => event.stopPropagation()}
          onpointerdown={(event) => event.stopPropagation()}
          onchange={(event) => setCardSort(card.name, event.currentTarget.value as SortMode)}
        >
          <option value="name">Name</option>
          <option value="price">Price</option>
          <option value="added">Added</option>
          <option value="custom">Custom</option>
        </select>
        <button
          class="icon add-instrument"
          type="button"
          title={`Add a stock, option or other instrument to ${card.name}`}
          aria-label={`Add instrument to ${card.name}`}
          onclick={(event) => openSearch(card.name, event)}
        >
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="M8 3v10M3 8h10" />
          </svg>
        </button>
        <button
          class="icon group-card-toggle"
          type="button"
          title={isCollapsed(card.name) ? "Expand group" : "Collapse group"}
          aria-expanded={!isCollapsed(card.name)}
          onclick={(event) => toggleCollapse(card.name, event)}
        >
          {#if isCollapsed(card.name)}
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <path d="M4 6l4 4 4-4" />
            </svg>
          {:else}
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <path d="M4 10l4-4 4 4" />
            </svg>
          {/if}
        </button>
        <button
          class="icon group-card-delete"
          type="button"
          title={`Delete ${card.name}`}
          aria-label={`Delete ${card.name}`}
          onclick={(event) => void deleteCard(card.name, event)}
        >
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="M4 4l8 8M12 4l-8 8" />
          </svg>
        </button>
      </header>

      <div class="group-card-search">
        <input
          use:focusWhen={searchFor === card.name}
          data-card-search={card.name}
          type="search"
          aria-label={`Search instruments to add to ${card.name}`}
          placeholder="Search stock, option, future or index"
          value={searchFor === card.name ? query : ""}
          onfocus={() => {
            if (searchFor !== card.name) {
              searchFor = card.name;
              query = "";
              results = [];
            }
          }}
          oninput={(event) => {
            searchFor = card.name;
            query = event.currentTarget.value;
          }}
          onkeydown={(event) => {
            if (event.key === "Escape") closeSearch();
          }}
        />
        {#if searchFor === card.name}
          {#if searchError}
            <p class="hint down">{searchError}</p>
          {:else if searching && results.length === 0}
            <p class="hint">Searching...</p>
          {:else if query.trim() !== "" && results.length === 0}
            <p class="hint">No instruments matched.</p>
          {:else if results.length > 0}
            <div class="group-card-results" role="listbox">
              {#each results as item (item.key)}
                {@const kind = instrumentKind({
                  is_index: item.is_index,
                  instrument_type: item.instrument_type,
                  strike_type: item.strike_type,
                })}
                <div class="group-card-hit" role="option" aria-selected={false}>
                  <span class="kind kind-{kind.toLowerCase()}">{kind}</span>
                  <span class="sym" title={item.trading_symbol}>{item.trading_symbol}</span>
                  <span class="name">
                    {item.underlying ?? item.name ?? item.instrument_type}
                    {#if item.expiry}&middot; {item.expiry}{/if}
                  </span>
                  <button
                    class="tiny"
                    type="button"
                    disabled={adding}
                    onclick={() => addInstrument(card.name, item)}
                  >
                    Add
                  </button>
                </div>
              {/each}
            </div>
          {/if}
        {/if}
      </div>

      {#if !isCollapsed(card.name)}
        <div class="group-card-body">
          {#if card.rows.length === 0}
            <p class="hint group-card-empty">
              {card.count === 0
                ? "No instruments yet. Use + to add one."
                : "Waiting for the instrument master; stored members are kept."}
            </p>
          {:else}
            {#each rowList as row (row.key)}
              {@const quote = quoteFor(row)}
              {@const kind = instrumentKind(row)}
              <div
                class="group-card-row"
                class:armed={armedKey === row.key}
                role="option"
                aria-selected={armedKey === row.key}
                aria-label={`${row.symbol} in ${card.name}`}
                tabindex="0"
                class:row-draggable={mode === "custom"}
                class:dragging={rowDragKey === row.key && rowDragGroup === card.name}
                draggable={mode === "custom"}
                title={row.name ?? row.symbol}
                onmouseenter={() => arm(row)}
                onclick={() => arm(row)}
                oncontextmenu={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  arm(row);
                  rowMenu = { x: event.clientX, y: event.clientY, row, group: card.name };
                }}
                onkeydown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    arm(row);
                  }
                }}
                ondragstart={(event) => startRowDrag(event, card.name, row.key)}
                ondragover={(event) => {
                  if (mode === "custom" && rowDragGroup === card.name) event.preventDefault();
                }}
                ondrop={(event) =>
                  dropRow(
                    event,
                    card.name,
                    row.key,
                    rowList.map((item) => item.key),
                  )}
                ondragend={endRowDrag}
              >
                <div class="group-card-inst">
                  {#if mode === "custom"}<span class="row-grip" aria-hidden="true">::</span>{/if}
                  <span class="kind kind-{kind.toLowerCase()}">{kind}</span>
                  <span class="sym">{row.symbol}</span>
                  <span class="detail">{instrumentDetail(row)}</span>
                </div>
                <span class="num mono {quote?.trend ?? 'flat'}">{quote?.ltp?.text ?? "-"}</span>
                <span class="num mono {quote?.trend ?? 'flat'}">{quote?.pct ?? "-"}</span>
                <button
                  class="icon group-card-remove"
                  type="button"
                  title={`Remove ${row.symbol} from ${card.name}`}
                  aria-label={`Remove ${row.symbol} from ${card.name}`}
                  onclick={(event) => {
                    event.stopPropagation();
                    void removeInstrument(card.name, row);
                  }}
                >
                  <svg viewBox="0 0 16 16" aria-hidden="true">
                    <path d="M4 4l8 8M12 4l-8 8" />
                  </svg>
                </button>
              </div>
            {/each}
            {#if card.count > card.rows.length}
              <p class="hint group-card-empty">
                {card.count - card.rows.length} stored member(s) are not in the master yet.
              </p>
            {/if}
          {/if}
        </div>
      {/if}
    </article>
  {/each}

  {#if creating}
    <div class="group-card new-card">
      <header class="group-card-head">
        <span class="group-card-dot" style="background: var(--accent-2)"></span>
        <input
          class="group-card-edit"
          use:focusOnMount
          bind:value={createDraft}
          placeholder="New card name"
          aria-label="New card name"
          onkeydown={(event) => {
            if (event.key === "Enter") void createCard();
            if (event.key === "Escape") {
              creating = false;
              createDraft = "";
            }
          }}
        />
      </header>
      <div class="group-card-new-actions">
        <button class="tiny" disabled={createDraft.trim() === ""} onclick={createCard}>Create</button>
        <button
          class="tiny ghost"
          onclick={() => {
            creating = false;
            createDraft = "";
          }}>Cancel</button
        >
      </div>
    </div>
  {:else}
    <button
      class="group-card new-card-tile"
      type="button"
      onclick={() => {
        creating = true;
        createDraft = "";
      }}
    >
      <span class="new-card-plus">+</span>
      <span>New card</span>
      <small>Create an empty group and add instruments with its + button.</small>
    </button>
  {/if}
</div>

{#if rowMenu}
  <InstrumentMenu
    x={rowMenu.x}
    y={rowMenu.y}
    instrumentKey={rowMenu.row.key}
    symbol={rowMenu.row.symbol}
    {onbuy}
    {onsell}
    {onchain}
    extras={[
      {
        label: `Remove from ${rowMenu.group}`,
        action: () => void removeInstrument(rowMenu!.group, rowMenu!.row),
      },
    ]}
    onclose={() => (rowMenu = null)}
  />
{/if}
