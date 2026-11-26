<script lang="ts">
  import { onMount } from "svelte";
  import { api, events, type ChainRowDto } from "../../api";
  import { formatVolume } from "../../state";
  import Badge from "../../components/Badge.svelte";

  interface Props {
    watched: string[];
    onadd: (key: string) => void;
    pending?: { underlying: string; nonce: number } | null;
    onconsumed?: () => void;
  }
  let { watched, onadd, pending = null, onconsumed }: Props = $props();

  let underlying = $state("NIFTY");
  let expiries = $state<string[]>([]);
  let expiry = $state("");
  let side = $state<"ALL" | "CE" | "PE">("ALL");
  let strikes = $state(10);
  let atm = $state("");
  let quick = $state("");
  let rows = $state<ChainRowDto[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let loaded = $state(false);

  // key -> index, so a delta updates one row without scanning the chain.
  let index = new Map<string, number>();

  const liveCount = $derived(rows.filter((row) => row.live).length);

  async function loadExpiries() {
    error = null;
    try {
      const list = await api.chainExpiries(underlying.trim());
      expiries = list;
      if (list.length === 0) {
        error = `No expiries found for "${underlying}".`;
        expiry = "";
      } else if (!list.includes(expiry)) {
        expiry = list[0];
      }
    } catch (e) {
      error = String(e);
    }
  }

  async function loadChain() {
    if (!underlying.trim() || !expiry) return;
    loading = true;
    error = null;
    try {
      // Subscribes only a window around the money; the rest is metadata only.
      const data = await api.chainLoad(
        underlying.trim(),
        expiry,
        Math.max(1, Math.min(20, Math.round(strikes))),
        atm.trim() === "" ? null : atm.trim(),
      );
      rows = data;
      index = new Map(data.map((row, i) => [row.key, i]));
      loaded = true;
    } catch (e) {
      error = String(e);
      rows = [];
      loaded = false;
    } finally {
      loading = false;
    }
  }

  const visible = $derived.by(() => {
    const needle = quick.trim().toUpperCase();
    return rows.filter((row) => {
      if (side !== "ALL" && row.strike_type !== side) return false;
      if (needle.length === 0) return true;
      return (
        row.symbol.toUpperCase().includes(needle) ||
        row.strike.includes(needle) ||
        row.strike_type.includes(needle)
      );
    });
  });

  const bid = (row: ChainRowDto) => row.quote?.bid?.text ?? "-";
  const ask = (row: ChainRowDto) => row.quote?.ask?.text ?? "-";
  const ltp = (row: ChainRowDto) => row.quote?.ltp?.text ?? "-";
  const trend = (row: ChainRowDto) => row.quote?.trend ?? "flat";

  onMount(() => {
    let disposed = false;
    const unlisteners: Array<() => void> = [];

    (async () => {
      try {
        const unlisten = await events.onDelta((delta) => {
          if (disposed) return;
          for (const quote of delta.quotes) {
            const at = index.get(quote.key);
            if (at !== undefined) rows[at] = { ...rows[at], quote };
          }
        });
        if (disposed) unlisten();
        else unlisteners.push(unlisten);

        // Expiries are local. The chain is NOT loaded automatically: subscribing
        // is an explicit action so the broker is only touched on request.
        await loadExpiries();

        // Opened from a row context menu: load that underlying straight away.
        if (pending) {
          underlying = pending.underlying;
          await loadExpiries();
          await loadChain();
          onconsumed?.();
        }
      } catch (e) {
        if (!disposed) error = String(e);
      }
    })();

    return () => {
      disposed = true;
      for (const unlisten of unlisteners) unlisten();
      // Release the chain subscriptions so the socket stops streaming legs.
      void api.chainClose().catch(() => undefined);
    };
  });
</script>

<section class="panel">
  <header class="panel-head">
    <h2>Option chain</h2>
    {#if loaded}
      <Badge label={`${rows.length} legs`} />
      <Badge label={`${liveCount} live`} tone={liveCount > 0 ? "ok" : "warn"} dot />
    {/if}
    <span class="spacer"></span>
  </header>

  <div class="chain-toolbar">
    <div class="field" style="width: 150px">
      <label for="chain-underlying">Underlying</label>
      <input
        id="chain-underlying"
        bind:value={underlying}
        onkeydown={(e) => {
          if (e.key === "Enter") void loadExpiries().then(loadChain);
        }}
      />
    </div>

    <div class="field" style="width: 165px">
      <label for="chain-expiry">Expiry</label>
      <select id="chain-expiry" bind:value={expiry}>
        {#each expiries as date (date)}
          <option value={date}>{date}</option>
        {/each}
      </select>
    </div>

    <div class="field" style="width: 105px">
      <label for="chain-strikes">Strikes +/-</label>
      <input id="chain-strikes" type="number" min="1" max="20" bind:value={strikes} />
    </div>

    <div class="field" style="width: 130px">
      <label for="chain-atm">ATM (optional)</label>
      <input id="chain-atm" bind:value={atm} placeholder="e.g. 25800" />
    </div>

    <div class="field" style="width: 105px">
      <label for="chain-side">Type</label>
      <select id="chain-side" bind:value={side}>
        <option value="ALL">ALL</option>
        <option value="CE">CE</option>
        <option value="PE">PE</option>
      </select>
    </div>

    <div class="field" style="flex: 1; min-width: 130px">
      <label for="chain-quick">Quick search</label>
      <input id="chain-quick" bind:value={quick} placeholder="strike or symbol" />
    </div>

    <button class="primary" disabled={loading || !expiry} onclick={loadChain}>
      {loading ? "Loading..." : "Load and subscribe"}
    </button>
  </div>

  {#if error}
    <div class="banner"><span>{error}</span></div>
  {/if}

  {#if visible.length === 0}
    <div class="empty">
      <h3>{loaded ? "No contracts match" : "Nothing loaded"}</h3>
      <p class="hint">
        {loaded
          ? "Adjust the type filter or quick search."
          : "Pick an underlying and expiry, then press Load and subscribe. Only the strikes window is subscribed (100 instruments max across the whole app); the full chain is browsable as metadata."}
      </p>
    </div>
  {:else}
    <div class="grid-wrap">
      <table class="grid">
        <thead>
          <tr>
            <th class="l">Exch</th>
            <th class="l">Symbol</th>
            <th class="l">ExpDate</th>
            <th>Strike</th>
            <th class="l">Type</th>
            <th>Bid</th>
            <th>BidQty</th>
            <th>LTP</th>
            <th>Ask</th>
            <th>AskQty</th>
            <th>NetChg</th>
            <th>%Chg</th>
            <th>Vol</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each visible as row (row.key)}
            <tr class:dim-row={!row.live}>
              <td class="l dim">{row.exchange}</td>
              <td class="l mono">{row.symbol}</td>
              <td class="l mono dim">{row.expiry ?? "-"}</td>
              <td class="num mono">{row.strike}</td>
              <td
                class="l"
                style="color: {row.strike_type === 'CE' ? 'var(--up)' : 'var(--down)'}"
              >
                {row.strike_type}
              </td>
              <td class="num mono dim">{bid(row)}</td>
              <td class="num mono dim">{row.quote?.bid_qty ?? "-"}</td>
              <td class="num mono {trend(row)}">{ltp(row)}</td>
              <td class="num mono dim">{ask(row)}</td>
              <td class="num mono dim">{row.quote?.ask_qty ?? "-"}</td>
              <td class="num mono {trend(row)}">{row.quote?.change?.text ?? "-"}</td>
              <td class="num mono {trend(row)}">{row.quote?.pct ?? "-"}</td>
              <td class="num mono dim">{formatVolume(row.quote?.volume ?? null)}</td>
              <td class="num">
                {#if watched.includes(row.key)}
                  <span class="dim" style="font-size: 11px">watching</span>
                {:else}
                  <button title="Add to market watch" onclick={() => onadd(row.key)}>Add</button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>
