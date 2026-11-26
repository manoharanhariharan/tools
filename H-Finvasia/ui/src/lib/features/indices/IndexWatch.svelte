<script lang="ts">
  import { onMount } from "svelte";
  import { api, events, type IndexRowDto } from "../../api";
  import EmptyState from "../../components/EmptyState.svelte";

  interface Props {
    onadd: (key: string) => void;
    refreshToken: number;
  }
  let { onadd, refreshToken }: Props = $props();

  let indices = $state<IndexRowDto[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let query = $state("");
  let index = new Map<string, number>();

  async function refresh() {
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

  // Refresh on mount and whenever the watch list changes (an Add flips a row
  // to "watching"). Deliberately not gated on a non-empty result: the first call
  // can legitimately return nothing while the master is still loading.
  $effect(() => {
    void refreshToken;
    void refresh();
  });
</script>

<section class="panel">
  <header class="panel-head">
    <h2>Index watch</h2>
    <span class="muted">{indices.length} indices</span>
    <span class="spacer"></span>
    <button disabled={loading} onclick={refresh}>{loading ? "Loading?" : "Refresh"}</button>
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
            <tr>
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
</section>
