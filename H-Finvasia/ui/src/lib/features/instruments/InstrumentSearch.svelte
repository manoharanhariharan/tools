<script lang="ts">
  import { api, type InstrumentDto } from "../../api";
  import EmptyState from "../../components/EmptyState.svelte";

  interface Props {
    watched: string[];
    onadd: (key: string) => void;
  }
  let { watched, onadd }: Props = $props();

  let query = $state("");
  let results = $state<InstrumentDto[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);

  // Debounced search with explicit timer cleanup on every re-run.
  $effect(() => {
    const text = query.trim();
    if (text.length < 1) {
      results = [];
      loading = false;
      error = null;
      return;
    }

    loading = true;
    const timer = setTimeout(async () => {
      try {
        results = await api.search(text, 40);
        error = null;
      } catch (err) {
        error = String(err);
        results = [];
      } finally {
        loading = false;
      }
    }, 180);

    return () => clearTimeout(timer);
  });

  function isWatched(key: string): boolean {
    return watched.includes(key);
  }
</script>

<section class="panel">
  <header class="panel-head">
    <h2>Instruments</h2>
    <span class="muted">
      {#if loading}searching…{:else if query.trim()} {results.length} result{results.length === 1 ? "" : "s"}{:else}search the instrument master{/if}
    </span>
  </header>

  <div style="padding: 12px 14px; border-bottom: 1px solid var(--border)">
    <div class="field">
      <label for="scrip-search">Symbol, underlying or name</label>
      <input
        id="scrip-search"
        type="search"
        placeholder="e.g. RELIANCE, NIFTY, ZYDUSLIFE"
        bind:value={query}
        autocomplete="off"
        spellcheck="false"
      />
    </div>
    {#if error}
      <p class="hint down" style="margin-top: 8px">Search failed: {error}</p>
    {/if}
  </div>

  {#if query.trim().length === 0}
    <EmptyState
      title="Search the master"
      hint="Backed by your local MongoDB FINVASIA_SYMBOL collection (182k+ instruments), or a bundled sample when Mongo is unavailable."
    />
  {:else if !loading && results.length === 0 && !error}
    <EmptyState title="No matches" hint="Try a shorter prefix, an underlying name, or the index symbol." />
  {:else}
    <div class="grid-wrap">
      <table class="grid">
        <thead>
          <tr>
            <th class="l">Symbol</th>
            <th class="l">Underlying</th>
            <th class="l">Expiry</th>
            <th>Strike</th>
            <th>Lot</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each results as item (item.key)}
            <tr>
              <td class="l mono">{item.trading_symbol}</td>
              <td class="l">{item.underlying ?? "—"}</td>
              <td class="l mono dim">{item.expiry ?? "—"}</td>
              <td class="num mono">{item.strike}</td>
              <td class="num mono dim">{item.lot_size}</td>
              <td class="num">
                {#if isWatched(item.key)}
                  <span class="dim" style="font-size: 11px">watching</span>
                {:else}
                  <button onclick={() => onadd(item.key)}>Add</button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>
