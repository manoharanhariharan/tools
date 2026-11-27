<script lang="ts">
  import { onMount, untrack } from "svelte";
  import {
    api,
    events,
    type ContainerCardDto,
    type ContainersView,
    type IndexRowDto,
    type InstrumentDto,
  } from "../../api";
  import EmptyState from "../../components/EmptyState.svelte";
  import InstrumentMenu from "../../components/InstrumentMenu.svelte";
  import GroupCards from "../stocks/GroupCards.svelte";
  import { sortInstrumentRows } from "../stocks/cards";
  import {
    loadSortMode,
    loadStocksView,
    saveSortMode,
    saveStocksView,
    type SortMode,
    type StocksView,
  } from "../../layoutPrefs";

  interface Props {
    onadd: (key: string) => void;
    onremove: (key: string) => void;
    onbuy: (key: string) => void;
    onsell: (key: string) => void;
    onchain: (underlying: string) => void;
    onactive: (key: string, label: string) => void;
    refreshToken: number;
  }
  let { onadd, onremove, onbuy, onsell, onchain, onactive, refreshToken }: Props = $props();

  let indices = $state<IndexRowDto[]>([]);
  let containers = $state<ContainersView>({ active: null, groups: [] });
  let cards = $state<ContainerCardDto[]>([]);
  let view = $state<StocksView>(loadStocksView());
  let sortMode = $state<SortMode>(loadSortMode());
  let query = $state("");
  let addQuery = $state("");
  let loading = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let menu = $state<{ x: number; y: number; row: IndexRowDto } | null>(null);

  const indexCards = $derived(
    cards.map((card) => {
      const rows = card.rows.filter((row) => row.is_index);
      return { ...card, rows, count: rows.length };
    }),
  );
  const activeCard = $derived(indexCards.find((card) => card.name === containers.active) ?? null);
  const activeKeys = $derived(
    new Set((activeCard?.rows ?? []).map((row) => row.key)),
  );
  const listSource = $derived(
    activeCard ? indices.filter((row) => activeKeys.has(row.key)) : indices,
  );
  const listRows = $derived.by(() => {
    const order = activeCard?.rows.map((row) => row.key) ?? listSource.map((row) => row.key);
    const custom = activeCard?.custom_order ?? [];
    return sortInstrumentRows(listSource, sortMode, order, custom);
  });
  const visible = $derived.by(() => {
    const needle = query.trim().toUpperCase();
    if (needle === "") return listRows;
    return listRows.filter(
      (row) =>
        row.symbol.toUpperCase().includes(needle) ||
        (row.name ?? "").toUpperCase().includes(needle),
    );
  });
  const addResults = $derived.by(() => {
    const needle = addQuery.trim().toUpperCase();
    if (needle === "") return [];
    return indices
      .filter(
        (row) =>
          !activeKeys.has(row.key) &&
          (row.symbol.toUpperCase().includes(needle) ||
            (row.name ?? "").toUpperCase().includes(needle)),
      )
      .slice(0, 8);
  });
  const cardTotal = $derived(indexCards.reduce((total, card) => total + card.count, 0));

  async function loadRows() {
    const rows = await api.indicesList();
    indices = rows;
  }

  async function loadContainers() {
    containers = await api.containersList();
  }

  async function loadCards() {
    cards = await api.containerCards();
  }

  async function refresh() {
    if (loading) return;
    loading = true;
    error = null;
    try {
      await Promise.all([loadRows(), loadContainers(), loadCards()]);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function setView(next: StocksView) {
    if (view === next) return;
    view = next;
    saveStocksView(next);
  }

  function setSort(next: SortMode) {
    sortMode = next;
    saveSortMode(next);
  }

  async function activate(name: string) {
    if (busy || name === containers.active) return;
    busy = true;
    error = null;
    try {
      await api.containerActivate(name);
      await Promise.all([loadContainers(), loadCards()]);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function cardAdd(group: string, key: string) {
    await api.containerAdd(group, key);
    await Promise.all([loadCards(), loadContainers()]);
  }

  async function cardRemove(group: string, key: string) {
    await api.containerRemove(group, key);
    await Promise.all([loadCards(), loadContainers()]);
  }

  async function cardReorder(order: string[]) {
    await api.containerReorder(order);
    await loadCards();
  }

  async function cardRename(oldName: string, newName: string) {
    await api.containerRename(oldName, newName);
    await Promise.all([loadCards(), loadContainers()]);
    notice = `Renamed "${oldName}" to "${newName}".`;
  }

  async function cardCreate(name: string) {
    await api.containerCreateEmpty(name);
    await Promise.all([loadCards(), loadContainers()]);
    notice = `Created "${name}". Use its + button to add indices.`;
  }

  async function cardDelete(name: string) {
    await api.containerDelete(name);
    await Promise.all([loadCards(), loadContainers()]);
    notice = `Deleted card "${name}".`;
  }

  async function cardReorderKeys(group: string, keys: string[]) {
    await api.containerReorderKeys(group, keys);
    await loadCards();
  }

  async function addIndex(key: string) {
    if (!containers.active) return;
    try {
      await cardAdd(containers.active, key);
      addQuery = "";
    } catch (e) {
      error = String(e);
    }
  }

  function toggleIndex(key: string) {
    if (activeKeys.has(key)) onremove(key);
    else onadd(key);
  }

  function indexToInstrument(row: IndexRowDto): InstrumentDto {
    return {
      key: row.key,
      trading_symbol: row.symbol,
      name: row.name,
      underlying: row.underlying,
      expiry: null,
      strike: "0",
      strike_type: "XX",
      lot_size: 0,
      instrument_type: "INDEX",
      has_fno: row.has_fno,
      is_index: true,
    };
  }

  async function searchIndices(text: string, limit: number): Promise<InstrumentDto[]> {
    const needle = text.trim().toUpperCase();
    if (needle === "") return [];
    const rows = indices.length > 0 ? indices : await api.indicesList();
    return rows
      .filter(
        (row) =>
          row.symbol.toUpperCase().includes(needle) ||
          (row.name ?? "").toUpperCase().includes(needle),
      )
      .slice(0, limit)
      .map(indexToInstrument);
  }

  onMount(() => {
    let disposed = false;
    const unlisteners: Array<() => void> = [];

    (async () => {
      try {
        const unlisten = await events.onDelta((delta) => {
          if (disposed) return;
          const updates = new Map(delta.quotes.map((quote) => [quote.key, quote]));
          if (!indices.some((row) => updates.has(row.key))) return;
          indices = indices.map((row) => {
            const quote = updates.get(row.key);
            return quote ? { ...row, live: true, quote } : row;
          });
        });
        if (disposed) unlisten();
        else unlisteners.push(unlisten);

        const onMaster = await events.onMasterReady(() => {
          if (!disposed) void refresh();
        });
        if (disposed) onMaster();
        else unlisteners.push(onMaster);
      } catch (e) {
        if (!disposed) error = String(e);
      }
    })();

    return () => {
      disposed = true;
      for (const unlisten of unlisteners) unlisten();
    };
  });

  let lastToken = -1;
  $effect(() => {
    const token = refreshToken;
    if (token === lastToken) return;
    lastToken = token;
    untrack(() => {
      void refresh();
    });
  });
</script>

<section class="panel">
  <header class="panel-head">
    <h2>Index</h2>
    {#if view === "cards"}
      <span class="muted">{containers.groups.length} group{containers.groups.length === 1 ? "" : "s"}</span>
      <span class="muted">{cardTotal} index{cardTotal === 1 ? "" : "es"}</span>
    {:else}
      <span class="muted">{visible.length} index{visible.length === 1 ? "" : "es"}</span>
    {/if}
    <span class="spacer"></span>
    <label class="sort-picker">
      <span>Sort</span>
      <select
        aria-label="Sort indices"
        value={sortMode}
        onchange={(event) => setSort(event.currentTarget.value as SortMode)}
      >
        <option value="name">Name (A-Z)</option>
        <option value="price">Price (high first)</option>
        <option value="added">Added</option>
        <option value="custom">Custom</option>
      </select>
    </label>
    <div class="view-switch" role="group" aria-label="Index layout">
      <button
        type="button"
        class:active={view === "list"}
        aria-pressed={view === "list"}
        onclick={() => setView("list")}>List</button
      >
      <button
        type="button"
        class:active={view === "cards"}
        aria-pressed={view === "cards"}
        onclick={() => setView("cards")}>Cards</button
      >
    </div>
    <button disabled={loading || busy} onclick={refresh}>{loading ? "Loading..." : "Refresh"}</button>
  </header>

  {#if error}
    <div class="banner"><span>{error}</span></div>
  {/if}
  {#if notice}
    <div class="banner ok-banner">
      <span>{notice}</span>
      <button class="ghost" onclick={() => (notice = null)}>Dismiss</button>
    </div>
  {/if}

  {#if view === "cards"}
    <GroupCards
      cards={indexCards}
      {sortMode}
      active={containers.active}
      onactivate={activate}
      onadd={cardAdd}
      onremove={cardRemove}
      onreorder={cardReorder}
      onrename={cardRename}
      oncreate={cardCreate}
      ondelete={cardDelete}
      onreorderkeys={cardReorderKeys}
      {onactive}
      {onbuy}
      {onsell}
      {onchain}
      search={searchIndices}
    />
  {:else}
    <div class="index-toolbar">
      <label class="sort-picker">
        <span>Group</span>
        <select
          aria-label="Index group"
          value={containers.active ?? ""}
          disabled={busy}
          onchange={(event) => activate(event.currentTarget.value)}
        >
          {#each containers.groups as group (group.name)}
            <option value={group.name}>{group.name} ({group.count})</option>
          {/each}
        </select>
      </label>
      <div class="field index-filter">
        <label for="index-search">Filter</label>
        <input id="index-search" bind:value={query} placeholder="NIFTY, BANKNIFTY?" />
      </div>
      <div class="index-add">
        <label class="index-add-label" for="index-add-search">Add index</label>
        <input
          id="index-add-search"
          bind:value={addQuery}
          placeholder="Type an index name"
          autocomplete="off"
        />
        {#if addResults.length > 0}
          <div class="index-add-results" role="listbox">
            {#each addResults as row (row.key)}
              <button type="button" onclick={() => void addIndex(row.key)}>
                <span>{row.symbol}</span>
                <small>{row.name ?? row.exchange}</small>
                <strong>+</strong>
              </button>
            {/each}
          </div>
        {:else if addQuery.trim() !== ""}
          <p class="hint index-add-empty">No matching index outside this group.</p>
        {/if}
      </div>
    </div>

    {#if visible.length === 0}
      <EmptyState
        title={query.trim() ? "No matching indices" : "No indices in this group"}
        hint="Switch group, clear the filter, or add an index above. Cards view can also add instruments."
      />
    {:else}
      <div class="grid-wrap">
        <table class="grid">
          <thead>
            <tr>
              <th class="l">Exch</th>
              <th class="l">Symbol</th>
              <th class="l">Name</th>
              <th>LTP</th>
              <th>Chg</th>
              <th>%Chg</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each visible as row (row.key)}
              <tr
                onmouseenter={() => onactive(row.key, row.symbol)}
                onclick={() => onactive(row.key, row.symbol)}
                oncontextmenu={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  onactive(row.key, row.symbol);
                  menu = { x: event.clientX, y: event.clientY, row };
                }}
              >
                <td class="l dim">{row.exchange}</td>
                <td class="l mono">{row.symbol}</td>
                <td class="l dim">{row.name ?? "-"}</td>
                <td class="num mono {row.quote?.trend ?? 'flat'}">{row.quote?.ltp?.text ?? "-"}</td>
                <td class="num mono {row.quote?.trend ?? 'flat'}">{row.quote?.change?.text ?? "-"}</td>
                <td class="num mono {row.quote?.trend ?? 'flat'}">{row.quote?.pct ?? "-"}</td>
                <td class="num">
                  {#if activeKeys.has(row.key)}
                    <button
                      class="tiny ghost"
                      title={`Remove ${row.symbol} from ${containers.active ?? "group"}`}
                      onclick={(event) => {
                        event.stopPropagation();
                        toggleIndex(row.key);
                      }}>Remove</button
                    >
                  {:else}
                    <button
                      class="tiny"
                      onclick={(event) => {
                        event.stopPropagation();
                        toggleIndex(row.key);
                      }}>Add</button
                    >
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/if}

  {#if menu}
    <InstrumentMenu
      x={menu.x}
      y={menu.y}
      instrumentKey={menu.row.key}
      symbol={menu.row.symbol}
      {onbuy}
      {onsell}
      {onchain}
      extras={[
        {
          label: activeKeys.has(menu.row.key) ? "Remove from group" : "Add to group",
          action: () => toggleIndex(menu!.row.key),
        },
      ]}
      onclose={() => (menu = null)}
    />
  {/if}
</section>
