<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { api, events, type IndexRowDto } from "../../api";
  import EmptyState from "../../components/EmptyState.svelte";
  import ContextMenu from "../../components/ContextMenu.svelte";

  interface Props {
    onadd: (key: string) => void;
    refreshToken: number;
    onchain: (underlying: string) => void;
  }
  let { onadd, refreshToken, onchain }: Props = $props();

  let menu = $state<{ x: number; y: number; row: IndexRowDto } | null>(null);

  let indices = $state<IndexRowDto[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let query = $state("");
  let index = new Map<string, number>();

  async function refresh() {
    // Guard against overlapping runs: the button must not toggle while a
    // request is already in flight.
    if (loading) return;
    loading = true;
    error = null;
    try {
      const rows = await api.indicesList();
      indices = rows;
      index = new Map(rows.map((row, i) => [row.key, i]));
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  const visible = $derived.by(() => {
    const needle = query.trim().toUpperCase();
    if (needle === "") return indices;
    return indices.filter(
      (row) =>
        row.symbol.toUpperCase().includes(needle) ||
        (row.name ?? "").toUpperCase().includes(needle),
    );
  });

  onMount(() => {
    let disposed = false;
    const unlisteners: Array<() => void> = [];

    (async () => {
      try {
        const unlisten = await events.onDelta((delta) => {
          if (disposed) return;
          for (const quote of delta.quotes) {
            const at = index.get(quote.key);
            if (at !== undefined) {
              indices[at] = { ...indices[at], live: true, quote };
            }
          }
        });
        if (disposed) unlisten();
        else unlisteners.push(unlisten);

        // The master can finish loading after this page mounts; refresh when it
        // does, otherwise the list stays empty.
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

  // Refresh when the watch list actually changes. `untrack` keeps the async
  // refresh (which writes `indices`/`loading`) out of the dependency graph, so
  // the effect cannot re-trigger itself.
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
    <h2>Index watch</h2>
    <span class="muted">{indices.length} indices</span>
    <span class="spacer"></span>
    <button disabled={loading} onclick={refresh}>Refresh</button>
  </header>

  <div class="chain-toolbar" style="padding: 8px 14px">
    <div class="field" style="flex: 1; min-width: 160px">
      <label for="index-search">Filter</label>
      <input id="index-search" bind:value={query} placeholder="NIFTY, BANKNIFTY?" />
    </div>
  </div>

  {#if error}
    <div class="banner"><span>{error}</span></div>
  {/if}

  {#if visible.length === 0}
    <EmptyState
      title="No indices"
      hint="Index instruments come from the local master; nothing is subscribed until you add one to the watch list."
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
              oncontextmenu={(event) => {
                event.preventDefault();
                event.stopPropagation();
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
                {#if row.live}
                  <span class="dim" style="font-size: 11px">watching</span>
                {:else}
                  <button onclick={() => onadd(row.key)}>Add</button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
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
          label: menu.row.live ? "Remove from watch" : "Add to watch",
          action: () => {
            if (!menu) return;
            if (menu.row.live) return;
            onadd(menu.row.key);
          },
          disabled: menu.row.live,
        },
      ]}
      onclose={() => (menu = null)}
    />
  {/if}
</section>
