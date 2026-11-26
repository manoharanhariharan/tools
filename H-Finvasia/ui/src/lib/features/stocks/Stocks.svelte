<script lang="ts">
  import { onMount } from "svelte";
  import {
    api,
    events,
    type ContainerCardDto,
    type ContainersView,
    type StockRowDto,
  } from "../../api";
  import { formatVolume } from "../../state";
  import {
    loadSortMode,
    loadStocksView,
    saveSortMode,
    saveStocksView,
    type SortMode,
    type StocksView,
  } from "../../layoutPrefs";
  import EmptyState from "../../components/EmptyState.svelte";
  import ContextMenu from "../../components/ContextMenu.svelte";
  import GroupCards from "./GroupCards.svelte";
  import { sortInstrumentRows } from "./cards";

  interface Props {
    watched: string[];
    onadd: (key: string) => void;
    onremove: (key: string) => void;
    onchain: (underlying: string) => void;
    onactive: (key: string, label: string) => void;
  }
  let { watched, onadd, onremove, onchain, onactive }: Props = $props();

  let rows = $state<StockRowDto[]>([]);
  let containers = $state<ContainersView>({ active: null, groups: [] });
  let cards = $state<ContainerCardDto[]>([]);
  let view = $state<StocksView>(loadStocksView());
  let sortMode = $state<SortMode>(loadSortMode());
  let query = $state("");
  let results = $state<StockRowDto[]>([]);
  let searching = $state(false);
  let searchOpen = $state(false);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let menu = $state<{ x: number; y: number; row: StockRowDto } | null>(null);
  let editing = $state<string | null>(null);
  let editingDraft = $state("");
  let creating = $state(false);
  let createDraft = $state("");
  let busy = $state(false);
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let searchSeq = 0;
  let rowsSeq = 0;
  let index = new Map<string, number>();

  const watchedSet = $derived(new Set(watched));
  const active = $derived(containers.active ?? "Watchlist");
  const activeCount = $derived(
    containers.groups.find((group) => group.name === containers.active)?.count ?? rows.length,
  );
  const cardTotal = $derived(cards.reduce((total, card) => total + card.count, 0));
  const activeCard = $derived(cards.find((card) => card.name === containers.active) ?? null);
  const listRows = $derived(
    sortInstrumentRows(
      rows,
      sortMode,
      activeCard?.rows.map((row) => row.key) ?? rows.map((row) => row.key),
      activeCard?.custom_order ?? [],
    ),
  );

  function focusOnMount(node: HTMLInputElement) {
    queueMicrotask(() => node.focus());
    return {};
  }

  async function loadRows() {
    const seq = ++rowsSeq;
    try {
      const list = await api.stocksWatched();
      if (seq !== rowsSeq) return;
      rows = list;
      index = new Map(list.map((row, i) => [row.key, i]));
    } catch (e) {
      if (seq === rowsSeq) error = String(e);
    }
  }

  async function loadContainers() {
    try {
      containers = await api.containersList();
    } catch (e) {
      error = String(e);
    }
  }

  async function loadCards() {
    try {
      cards = await api.containerCards();
    } catch (e) {
      error = String(e);
    }
  }

  async function refresh() {
    if (loading) return;
    loading = true;
    error = null;
    try {
      await Promise.all([loadRows(), loadContainers(), loadCards()]);
    } finally {
      loading = false;
    }
  }

  function setView(next: StocksView) {
    if (view === next) return;
    view = next;
    saveStocksView(next);
    if (next === "cards") void loadCards();
  }

  function setSort(next: SortMode) {
    sortMode = next;
    saveSortMode(next);
  }

  function scheduleSearch() {
    if (searchTimer !== undefined) clearTimeout(searchTimer);
    const text = query.trim();
    if (text === "") {
      results = [];
      searching = false;
      searchOpen = false;
      return;
    }
    searching = true;
    searchOpen = true;
    const seq = ++searchSeq;
    searchTimer = setTimeout(async () => {
      try {
        const list = await api.stocksSearch(text, 20);
        if (seq === searchSeq) results = list;
      } catch (e) {
        if (seq === searchSeq) error = String(e);
      } finally {
        if (seq === searchSeq) searching = false;
      }
    }, 160);
  }

  function onSearchInput(event: Event) {
    query = (event.currentTarget as HTMLInputElement).value;
    scheduleSearch();
  }

  function clearSearch() {
    query = "";
    results = [];
    searching = false;
    searchOpen = false;
    if (searchTimer !== undefined) clearTimeout(searchTimer);
  }

  async function activate(name: string) {
    if (busy || name === containers.active) return;
    busy = true;
    error = null;
    try {
      await api.containerActivate(name);
      await Promise.all([loadContainers(), loadRows(), loadCards()]);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function cardAdd(group: string, key: string) {
    if (busy) return;
    busy = true;
    error = null;
    try {
      containers = await api.containerAdd(group, key);
      await Promise.all([loadCards(), loadRows()]);
    } catch (e) {
      error = String(e);
      throw e;
    } finally {
      busy = false;
    }
  }

  async function cardRemove(group: string, key: string) {
    if (busy) return;
    busy = true;
    error = null;
    try {
      containers = await api.containerRemove(group, key);
      await Promise.all([loadCards(), loadRows()]);
    } catch (e) {
      error = String(e);
      throw e;
    } finally {
      busy = false;
    }
  }

  async function cardReorder(next: string[]) {
    if (busy) return;
    busy = true;
    error = null;
    try {
      containers = await api.containerReorder(next);
      await loadCards();
    } catch (e) {
      error = String(e);
      throw e;
    } finally {
      busy = false;
    }
  }

  async function cardRename(oldName: string, newName: string) {
    if (busy) return;
    busy = true;
    error = null;
    try {
      containers = await api.containerRename(oldName, newName);
      await Promise.all([loadCards(), loadRows()]);
      notice = `Renamed "${oldName}" to "${newName}".`;
    } catch (e) {
      error = String(e);
      throw e;
    } finally {
      busy = false;
    }
  }

  async function cardCreate(name: string) {
    if (busy) return;
    busy = true;
    error = null;
    try {
      containers = await api.containerCreateEmpty(name);
      await Promise.all([loadCards(), loadContainers()]);
      notice = `Created "${name}". Use its + button to add instruments.`;
    } catch (e) {
      error = String(e);
      throw e;
    } finally {
      busy = false;
    }
  }

  async function cardDelete(name: string) {
    if (busy) return;
    busy = true;
    error = null;
    try {
      containers = await api.containerDelete(name);
      await Promise.all([loadCards(), loadRows(), loadContainers()]);
      notice = `Deleted card "${name}".`;
    } catch (e) {
      error = String(e);
      throw e;
    } finally {
      busy = false;
    }
  }

  async function cardReorderKeys(group: string, keys: string[]) {
    if (busy) return;
    busy = true;
    error = null;
    try {
      containers = await api.containerReorderKeys(group, keys);
      await loadCards();
    } catch (e) {
      error = String(e);
      throw e;
    } finally {
      busy = false;
    }
  }

  async function createContainer() {
    const name = createDraft.trim();
    if (name === "" || busy) return;
    busy = true;
    error = null;
    try {
      containers = await api.containerCreate(name);
      creating = false;
      createDraft = "";
      notice = `Created "${name}" from the current watch list.`;
      await Promise.all([loadContainers(), loadRows()]);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function commitRename() {
    if (busy) return;
    const old = editing;
    const next = editingDraft.trim();
    if (!old) return;
    if (next === "" || next === old) {
      editing = null;
      return;
    }
    busy = true;
    error = null;
    try {
      containers = await api.containerRename(old, next);
      editing = null;
      editingDraft = "";
      notice = `Renamed "${old}" to "${next}".`;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function setGroupColor(name: string, color: string) {
    if (busy) return;
    busy = true;
    error = null;
    try {
      containers = await api.containerColor(name, color);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function deleteContainer(name: string) {
    if (busy) return;
    if (!window.confirm(`Delete group "${name}"?`)) return;
    busy = true;
    error = null;
    try {
      containers = await api.containerDelete(name);
      notice = `Deleted "${name}".`;
      await Promise.all([loadContainers(), loadRows()]);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    let disposed = false;
    const unlisteners: Array<() => void> = [];

    (async () => {
      try {
        const onDelta = await events.onDelta((delta) => {
          if (disposed) return;
          for (const quote of delta.quotes) {
            const at = index.get(quote.key);
            if (at !== undefined) rows[at] = { ...rows[at], quote };
          }
        });
        if (disposed) onDelta();
        else unlisteners.push(onDelta);

        const onWatch = await events.onWatch(() => {
          if (!disposed) {
            void loadRows();
            void loadContainers();
            void loadCards();
          }
        });
        if (disposed) onWatch();
        else unlisteners.push(onWatch);

        const onMaster = await events.onMasterReady(() => {
          if (!disposed) void refresh();
        });
        if (disposed) onMaster();
        else unlisteners.push(onMaster);

        await refresh();
      } catch (e) {
        if (!disposed) error = String(e);
      }
    })();

    return () => {
      disposed = true;
      if (searchTimer !== undefined) clearTimeout(searchTimer);
      for (const unlisten of unlisteners) unlisten();
    };
  });
</script>

<section class="panel">
  <header class="panel-head">
    <h2>Stocks</h2>
    {#if view === "cards"}
      <span class="muted">{containers.groups.length} group{containers.groups.length === 1 ? "" : "s"}</span>
      <span class="muted">{cardTotal} instrument{cardTotal === 1 ? "" : "s"}</span>
    {:else}
      <span class="muted">{rows.length} stocks</span>
      {#if activeCount > rows.length}
        <span class="muted">({activeCount} instruments in {active})</span>
      {/if}
    {/if}
    <span class="spacer"></span>
    <label class="sort-picker">
      <span>Sort</span>
      <select
        aria-label="Sort instruments"
        value={sortMode}
        onchange={(event) => setSort(event.currentTarget.value as SortMode)}
      >
        <option value="name">Name (A-Z)</option>
        <option value="price">Price (high first)</option>
        <option value="added">Added</option>
        <option value="custom">Custom</option>
      </select>
    </label>
    <div class="view-switch" role="group" aria-label="Stocks layout">
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
      {cards}
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
    />
  {:else}
  <div class="stocks-body">
    <aside class="containers">
      <div class="containers-head">
        <span>Groups</span>
        <span class="spacer"></span>
        <button class="tiny" title="New group (copies the current watch)" disabled={busy} onclick={() => (creating = true)}>
          +
        </button>
      </div>

      {#if creating}
        <div class="container-create">
          <input
            use:focusOnMount
            bind:value={createDraft}
            placeholder="Group name"
            onkeydown={(event) => {
              if (event.key === "Enter") void createContainer();
              if (event.key === "Escape") {
                creating = false;
                createDraft = "";
              }
            }}
          />
          <button class="tiny" disabled={busy || createDraft.trim() === ""} onclick={createContainer}>
            Add
          </button>
        </div>
      {/if}

      <div class="container-list">
        {#each containers.groups as group (group.name)}
          <div
            class="container-row"
            class:active={group.name === containers.active}
            style="--group-color: {group.color ?? '#4B9BD8'}"
            role="button"
            tabindex="0"
            title={group.name === containers.active ? "Active group" : "Switch to this group"}
            onclick={() => activate(group.name)}
            onkeydown={(event) => {
              if (event.key === "Enter" || event.key === " ") void activate(group.name);
            }}
          >
            {#if editing === group.name}
              <input
                class="container-edit"
                use:focusOnMount
                bind:value={editingDraft}
                onclick={(event) => event.stopPropagation()}
                onkeydown={(event) => {
                  if (event.key === "Enter") void commitRename();
                  if (event.key === "Escape") {
                    editing = null;
                    editingDraft = "";
                  }
                }}
                onblur={commitRename}
              />
            {:else}
              <button
                class="container-name"
                type="button"
                onclick={(event) => {
                  event.stopPropagation();
                  void activate(group.name);
                }}
                ondblclick={(event) => {
                  event.stopPropagation();
                  editing = group.name;
                  editingDraft = group.name;
                }}>{group.name}</button
              >
            {/if}
            <input
              class="group-color"
              type="color"
              value={group.color ?? "#4B9BD8"}
              title={`Color for ${group.name}`}
              onclick={(event) => event.stopPropagation()}
              onchange={(event) => void setGroupColor(group.name, event.currentTarget.value)}
            />
            <span class="container-count">{group.count}</span>
            <button
              class="container-del"
              title={`Delete ${group.name}`}
              disabled={busy}
              onclick={(event) => {
                event.stopPropagation();
                void deleteContainer(group.name);
              }}>x</button
            >
          </div>
        {/each}
      </div>
    </aside>

    <div class="stocks-main">
      <div class="stock-search-wrap">
        <div class="field">
          <label for="stock-search">Search stocks to add to {active}</label>
          <input
            id="stock-search"
            value={query}
            oninput={onSearchInput}
            onfocus={() => {
              if (query.trim() !== "") searchOpen = true;
            }}
            onkeydown={(event) => {
              if (event.key === "Escape") clearSearch();
            }}
            placeholder="Symbol or company name"
          />
        </div>

        {#if searchOpen && query.trim() !== ""}
          <div class="search-flyout" role="listbox" tabindex="-1">
            {#if searching && results.length === 0}
              <div class="search-empty">Searching...</div>
            {:else if results.length === 0}
              <div class="search-empty">No matching NSE stocks.</div>
            {:else}
              {#each results as row (row.key)}
                <div class="search-hit" role="option" aria-selected={watchedSet.has(row.key)}>
                  <span class="symbol">{row.symbol}</span>
                  <span class="name">{row.name && row.name !== row.symbol ? row.name : row.exchange}</span>
                  {#if row.has_fno}<span class="badge">F&amp;O</span>{/if}
                  {#if watchedSet.has(row.key)}
                    <button class="tiny" disabled>Added</button>
                  {:else}
                    <button
                      class="tiny"
                      disabled={busy}
                      onclick={() => onadd(row.key)}>Add</button
                    >
                  {/if}
                </div>
              {/each}
            {/if}
          </div>
        {/if}
      </div>

      {#if rows.length === 0}
        <EmptyState
          title={`No watched stocks in ${active}`}
          hint="Search above to add a stock to this group. Switching groups replaces the watch list."
        />
      {:else}
        <div class="stock-head" role="row">
          <span role="columnheader">Symbol</span>
          <span role="columnheader" class="num">LTP</span>
          <span role="columnheader" class="num">Chg</span>
          <span role="columnheader" class="num">%Chg</span>
          <span role="columnheader" class="num">Volume</span>
          <span role="columnheader" class="num">Watch</span>
        </div>

        <div class="stock-scroll" role="rowgroup">
          {#each listRows as row (row.key)}
            <div
              class="stock-row watched"
              role="row"
              tabindex="-1"
              onmouseenter={() => onactive(row.key, row.symbol)}
              onclick={() => onactive(row.key, row.symbol)}
              onkeydown={(event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  onactive(row.key, row.symbol);
                }
              }}
              title={row.has_fno
                ? "F&O available - right-click for the option chain"
                : (row.name ?? undefined)}
              oncontextmenu={(event) => {
                event.preventDefault();
                event.stopPropagation();
                menu = { x: event.clientX, y: event.clientY, row };
              }}
            >
              <span class="mono symbol" role="cell">{row.symbol}</span>
              <span class="num mono {row.quote?.trend ?? 'flat'}" role="cell">
                {row.quote?.ltp?.text ?? "-"}
              </span>
              <span class="num mono {row.quote?.trend ?? 'flat'}" role="cell">
                {row.quote?.change?.text ?? "-"}
              </span>
              <span class="num mono {row.quote?.trend ?? 'flat'}" role="cell">
                {row.quote?.pct ?? "-"}
              </span>
              <span class="num mono dim" role="cell">
                {formatVolume(row.quote?.volume ?? null)}
              </span>
              <span class="num" role="cell">
                <button
                  class="ghost tiny"
                  title="Remove from group"
                  onclick={(event) => {
                    event.stopPropagation();
                    onremove(row.key);
                  }}>watching</button
                >
              </span>
            </div>
          {/each}
        </div>
      {/if}

      <p class="hint" style="padding: 8px 14px; border-top: 1px solid var(--border)">
        Showing watched stocks only. Double-click a group name to rename it. Changes to the active
        group are saved locally; no broker call is made until a live subscription needs it.
      </p>
    </div>
  </div>
  {/if}

  {#if menu}
    <ContextMenu
      x={menu.x}
      y={menu.y}
      items={[
        {
          label: menu.row.underlying
            ? `Open option chain (${menu.row.underlying})`
            : "Open option chain",
          disabled: !menu.row.has_fno,
          action: () => {
            if (menu?.row.underlying) onchain(menu.row.underlying);
          },
        },
        {
          label: "Remove from group",
          action: () => {
            if (menu) onremove(menu.row.key);
          },
        },
      ]}
      onclose={() => (menu = null)}
    />
  {/if}
</section>
