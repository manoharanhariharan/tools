<script lang="ts">
  import { onMount } from "svelte";
  import { api, events, type StockRowDto } from "../../api";
  import { formatVolume } from "../../state";
  import EmptyState from "../../components/EmptyState.svelte";
  import ContextMenu from "../../components/ContextMenu.svelte";
  import { matchesStock, sortStocks, visibleSlice } from "./stocks";

  interface Props {
    watched: string[];
    onadd: (key: string) => void;
    onremove: (key: string) => void;
    onchain: (underlying: string) => void;
  }
  let { watched, onadd, onremove, onchain }: Props = $props();

  const ROW_H = 30;

  let rows = $state<StockRowDto[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let query = $state("");
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  let scrollEl = $state<HTMLDivElement | null>(null);
  let menu = $state<{ x: number; y: number; row: StockRowDto } | null>(null);
  let index = new Map<string, number>();

  const watchedSet = $derived(new Set(watched));
  const watchedCount = $derived(rows.filter((row) => watchedSet.has(row.key)).length);

  const sorted = $derived(sortStocks(rows.filter((row) => matchesStock(row, query)), watched));
  const slice = $derived(visibleSlice(sorted, scrollTop, viewportHeight, ROW_H));
  const pageRows = $derived(sorted.slice(slice.start, slice.end));

  async function refresh() {
    if (loading) return;
    loading = true;
    error = null;
    try {
      const list = await api.stocksList();
      rows = list;
      index = new Map(list.map((row, i) => [row.key, i]));
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function resetScroll() {
    scrollTop = 0;
    if (scrollEl) scrollEl.scrollTop = 0;
  }

  function onFilter(event: Event) {
    query = (event.currentTarget as HTMLInputElement).value;
    resetScroll();
  }

  function trend(row: StockRowDto): string {
    return row.quote?.trend ?? "flat";
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

        // The master may finish loading after this tab mounts.
        const onMaster = await events.onMasterReady(() => {
          if (!disposed) void refresh();
        });
        if (disposed) onMaster();
        else unlisteners.push(onMaster);

        // Newly watched rows need their metadata/quote shape; the master read is
        // local, so this does not touch the broker.
        const onWatch = await events.onWatch(() => {
          if (!disposed) void refresh();
        });
        if (disposed) onWatch();
        else unlisteners.push(onWatch);

        await refresh();
      } catch (e) {
        if (!disposed) error = String(e);
      }
    })();

    return () => {
      disposed = true;
      for (const unlisten of unlisteners) unlisten();
    };
  });
</script>

<section class="panel">
  <header class="panel-head">
    <h2>Stocks</h2>
    <span class="muted">{rows.length} NSE equities</span>
    {#if watchedCount > 0}
      <span class="badge ok"><span class="dot"></span>{watchedCount} watching</span>
    {/if}
    <span class="spacer"></span>
    <button disabled={loading} onclick={refresh}>{loading ? "Loading..." : "Refresh"}</button>
  </header>

  <div class="chain-toolbar" style="padding: 8px 14px">
    <div class="field" style="flex: 1; min-width: 180px">
      <label for="stock-search">Filter</label>
      <input
        id="stock-search"
        value={query}
        oninput={onFilter}
        placeholder="Symbol or company name"
      />
    </div>
    <span class="muted" style="padding-bottom: 7px">
      {sorted.length} shown - watched first
    </span>
  </div>

  {#if error}
    <div class="banner"><span>{error}</span></div>
  {/if}

  {#if rows.length === 0}
    <EmptyState
      title={loading ? "Loading stocks" : "No stocks"}
      hint="The NSE cash-equity list comes from the local instrument master. Nothing is subscribed until you add a stock to the watch list."
    />
  {:else if sorted.length === 0}
    <EmptyState title="No match" hint="Try a different symbol or company name." />
  {:else}
    <div class="stock-head" role="row">
      <span role="columnheader">Symbol</span>
      <span role="columnheader" class="num">LTP</span>
      <span role="columnheader" class="num">Chg</span>
      <span role="columnheader" class="num">%Chg</span>
      <span role="columnheader" class="num">Volume</span>
      <span role="columnheader" class="num">Watch</span>
    </div>

    <div
      class="stock-scroll"
      bind:this={scrollEl}
      bind:clientHeight={viewportHeight}
      onscroll={(event) => (scrollTop = (event.currentTarget as HTMLDivElement).scrollTop)}
      role="rowgroup"
    >
      <div style="height: {slice.top}px"></div>
      {#each pageRows as row (row.key)}
        <div
          class="stock-row"
          class:watched={watchedSet.has(row.key)}
          role="row"
          tabindex="-1"
          title={row.has_fno
            ? row.name && row.name !== row.symbol
              ? `${row.name} - F&O available`
              : "F&O available - right-click for the option chain"
            : (row.name ?? undefined)}
          oncontextmenu={(event) => {
            event.preventDefault();
            event.stopPropagation();
            menu = { x: event.clientX, y: event.clientY, row };
          }}
        >
          <span class="mono symbol" role="cell">{row.symbol}</span>
          <span class="num mono {trend(row)}" role="cell">{row.quote?.ltp?.text ?? "-"}</span>
          <span class="num mono {trend(row)}" role="cell">{row.quote?.change?.text ?? "-"}</span>
          <span class="num mono {trend(row)}" role="cell">{row.quote?.pct ?? "-"}</span>
          <span class="num mono dim" role="cell">{formatVolume(row.quote?.volume ?? null)}</span>
          <span class="num" role="cell">
            {#if watchedSet.has(row.key)}
              <button class="ghost tiny" title="Remove from watch" onclick={() => onremove(row.key)}>
                watching
              </button>
            {:else}
              <button class="tiny" title="Add to watch" onclick={() => onadd(row.key)}>Add</button>
            {/if}
          </span>
        </div>
      {/each}
      <div style="height: {slice.bottom}px"></div>
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
          label: watchedSet.has(menu.row.key) ? "Remove from watch" : "Add to watch",
          action: () => {
            if (!menu) return;
            if (watchedSet.has(menu.row.key)) onremove(menu.row.key);
            else onadd(menu.row.key);
          },
        },
      ]}
      onclose={() => (menu = null)}
    />
  {/if}
</section>
