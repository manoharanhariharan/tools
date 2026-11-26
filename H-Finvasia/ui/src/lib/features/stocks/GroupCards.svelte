<script lang="ts">
  import { onMount, untrack } from "svelte";
  import {
    api,
    events,
    type ContainerCardDto,
    type ContainerRowDto,
    type InstrumentDto,
    type QuoteDto,
  } from "../../api";
  import { instrumentDetail, instrumentKind, mergeOrder, moveRelative } from "./cards";
  import { loadCollapsedGroups, saveCollapsedGroups } from "../../layoutPrefs";

  interface Props {
    cards: ContainerCardDto[];
    active: string | null;
    onactivate: (name: string) => void;
    onadd: (group: string, key: string) => Promise<void>;
    onremove: (group: string, key: string) => Promise<void>;
    onreorder: (order: string[]) => Promise<void>;
  }
  let { cards, active, onactivate, onadd, onremove, onreorder }: Props = $props();

  let order = $state<string[]>([]);
  let collapsed = $state<Record<string, boolean>>(loadCollapsedGroups());
  let quotes = $state<Map<string, QuoteDto>>(new Map());
  let keySet = $state<Set<string>>(new Set());
  let error = $state<string | null>(null);
  let dragName = $state<string | null>(null);
  let dropTarget = $state<string | null>(null);
  let dragMoved = false;

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

  // Adopt the backend order once, then keep local drag order for later refreshes.
  $effect(() => {
    const names = cards.map((card) => card.name);
    const next = mergeOrder(order, names);
    if (next.join("\u0000") !== order.join("\u0000")) order = next;
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
        const hits = await api.search(text, 20);
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

  function openSearch(name: string, event: MouseEvent) {
    event.stopPropagation();
    if (searchFor === name) {
      closeSearch();
      return;
    }
    searchFor = name;
    query = "";
    results = [];
    searchError = null;
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
        <button
          class="group-card-name"
          type="button"
          title={card.name === active ? "Active group" : `Switch to ${card.name}`}
          onclick={() => onactivate(card.name)}
        >
          {card.name}
        </button>
        <span class="group-card-count" title="Instruments in this group">{card.count}</span>
        {#if card.name === active}<span class="group-card-live">live</span>{/if}
        <span class="spacer"></span>
        <button
          class="icon"
          type="button"
          title={`Add a stock, option or other instrument to ${card.name}`}
          aria-label={`Add instrument to ${card.name}`}
          onclick={(event) => openSearch(card.name, event)}>+</button
        >
        <button
          class="icon"
          type="button"
          title={isCollapsed(card.name) ? "Expand group" : "Collapse group"}
          aria-expanded={!isCollapsed(card.name)}
          onclick={(event) => toggleCollapse(card.name, event)}
        >
          {isCollapsed(card.name) ? "v" : "^"}
        </button>
      </header>

      {#if searchFor === card.name}
        <div class="group-card-search">
          <input
            use:focusOnMount
            type="search"
            aria-label={`Search instruments to add to ${card.name}`}
            placeholder="Search stock, option, future or index"
            bind:value={query}
            onkeydown={(event) => {
              if (event.key === "Escape") closeSearch();
            }}
          />
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
        </div>
      {/if}

      {#if !isCollapsed(card.name)}
        <div class="group-card-body">
          {#if card.rows.length === 0}
            <p class="hint group-card-empty">
              {card.count === 0
                ? "No instruments yet. Use + to add one."
                : "Waiting for the instrument master; stored members are kept."}
            </p>
          {:else}
            {#each card.rows as row (row.key)}
              {@const quote = quoteFor(row)}
              {@const kind = instrumentKind(row)}
              <div class="group-card-row" title={row.name ?? row.symbol}>
                <div class="group-card-inst">
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
                  onclick={() => removeInstrument(card.name, row)}>x</button
                >
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
</div>
