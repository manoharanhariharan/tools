<script lang="ts">
  import { onMount } from "svelte";
  import { api, events, type ChainRowDto, type QuoteDto } from "../../api";
  import { formatVolume } from "../../state";
  import Badge from "../../components/Badge.svelte";
  import {
    deltaOi,
    fillPercent,
    groupStrikes,
    pickAtm,
    sideMoneyness,
    windowAround,
    type StrikeLadderRow,
  } from "./chain";

  interface Props {
    pending?: { underlying: string; nonce: number } | null;
    onconsumed?: () => void;
    onorder: (key: string, side: "B" | "S") => void;
    ondefaults: () => void;
    onactive: (key: string, label: string) => void;
  }
  let {
    pending = null,
    onconsumed,
    onorder,
    ondefaults,
    onactive,
  }: Props = $props();

  let underlying = $state("NIFTY");
  let expiries = $state<string[]>([]);
  let expiry = $state("");
  let strikes = $state(10);
  let atm = $state("");
  let quick = $state("");
  let rows = $state<ChainRowDto[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let loaded = $state(false);

  // The underlying spot is only known when it is on the watch list. Without it
  // the ATM strike is used as the ITM/OTM reference, which is the same boundary.
  let spot = $state<QuoteDto | null>(null);
  let spotKey = $state<string | null>(null);
  // Frozen when the chain loads, so ticks do not shift the ladder under the cursor.
  let centreE6 = $state<number | null>(null);

  let index = new Map<string, number>();

  const ladder = $derived(groupStrikes(rows));
  const atmE6 = $derived(
    centreE6 ?? pickAtm(ladder, atm.trim() !== '' ? atm : (spot?.ltp?.text ?? null)),
  );
  const visibleRows = $derived.by(() => {
    const window = windowAround(ladder, atmE6, Number(strikes));
    const needle = quick.trim().toUpperCase();
    if (needle === "") return window;
    return window.filter((row) => row.strike.toUpperCase().includes(needle));
  });
  const referenceE6 = $derived(spot?.ltp?.e6 ?? atmE6);

  const extremes = $derived.by(() => {
    let oi = 0;
    let volume = 0;
    for (const row of visibleRows) {
      for (const leg of [row.ce, row.pe]) {
        if (leg?.quote?.oi) oi = Math.max(oi, leg.quote.oi);
        if (leg?.quote?.volume) volume = Math.max(volume, leg.quote.volume);
      }
    }
    return { oi, volume };
  });

  const liveCount = $derived(rows.filter((row) => row.live).length);

  interface ChainCell {
    text: string;
    cls: string;
    fill: number;
    fillClass: string;
  }

  function compact(value: number | null | undefined): string {
    return value === null || value === undefined ? "-" : formatVolume(value);
  }

  function signed(value: number | null): string {
    if (value === null) return "-";
    if (value === 0) return "0";
    return value > 0 ? `+${formatVolume(value)}` : `-${formatVolume(Math.abs(value))}`;
  }

  function fmtStrike(value: number | null): string {
    if (value === null) return "-";
    return (value / 1_000_000).toLocaleString("en-IN", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    });
  }

  function cells(row: StrikeLadderRow, side: "CE" | "PE"): ChainCell[] {
    const leg = side === "CE" ? row.ce : row.pe;
    const q = leg?.quote ?? null;
    const trend = q?.trend ?? "flat";
    const shade = sideMoneyness(side, row.strikeE6, referenceE6 ?? row.strikeE6);
    const base = `num mono ${shade}`;
    const anchor = side === "CE" ? "fill-right" : "fill-left";
    const change = deltaOi(q);
    const changeTone = change === null || change === 0 ? "flat" : change > 0 ? "up" : "down";

    const column: Record<string, ChainCell> = {
      oi: {
        text: compact(q?.oi),
        cls: `${base} fillable`,
        fill: fillPercent(q?.oi ?? null, extremes.oi),
        fillClass: `${anchor} oi-fill`,
      },
      doi: {
        text: signed(change),
        cls: `${base} ${changeTone}`,
        fill: 0,
        fillClass: "",
      },
      vol: {
        text: compact(q?.volume),
        cls: `${base} dim fillable`,
        fill: fillPercent(q?.volume ?? null, extremes.volume),
        fillClass: `${anchor} vol-fill`,
      },
      ltp: { text: q?.ltp?.text ?? "-", cls: `${base} ${trend}`, fill: 0, fillClass: "" },
      chg: { text: q?.change?.text ?? "-", cls: `${base} ${trend}`, fill: 0, fillClass: "" },
      bq: { text: q?.bid_qty === null || q?.bid_qty === undefined ? "-" : String(q.bid_qty), cls: `${base} dim`, fill: 0, fillClass: "" },
      bid: { text: q?.bid?.text ?? "-", cls: `${base} dim`, fill: 0, fillClass: "" },
      ask: { text: q?.ask?.text ?? "-", cls: `${base} dim`, fill: 0, fillClass: "" },
      aq: { text: q?.ask_qty === null || q?.ask_qty === undefined ? "-" : String(q.ask_qty), cls: `${base} dim`, fill: 0, fillClass: "" },
    };

    const order =
      side === "CE"
        ? ["oi", "doi", "vol", "ltp", "chg", "bq", "bid", "ask", "aq"]
        : ["bq", "bid", "ask", "aq", "ltp", "chg", "vol", "doi", "oi"];
    return order.map((key) => column[key]);
  }

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

  async function refreshSpot() {
    const wanted = underlying.trim().toUpperCase();
    if (!wanted) {
      spot = null;
      spotKey = null;
      return;
    }
    try {
      const quotes = await api.watchQuotes();
      const candidates = quotes.filter(
        (quote) => (quote.underlying ?? "").toUpperCase() === wanted,
      );
      const best =
        candidates.find((quote) => quote.is_index) ??
        candidates.find((quote) => {
          const symbol = (quote.trading_symbol ?? "").toUpperCase();
          return symbol === wanted || symbol === `${wanted}-EQ`;
        }) ??
        candidates[0] ??
        null;
      spot = best;
      spotKey = best?.key ?? null;
    } catch {
      spot = null;
      spotKey = null;
    }
  }

  async function loadChain() {
    if (!underlying.trim() || !expiry) return;
    loading = true;
    error = null;
    try {
      // Prefer a live index/stock quote as the ATM when one is watched; the
      // backend otherwise centres on the middle of the full chain, which is
      // rarely where the market is.
      await refreshSpot();
      const centre = atm.trim() !== "" ? atm.trim() : (spot?.ltp?.text ?? null);
      const data = await api.chainLoad(
        underlying.trim(),
        expiry,
        Math.max(1, Math.min(20, Math.round(Number(strikes) || 10))),
        centre,
      );
      rows = data;
      index = new Map(data.map((row, i) => [row.key, i]));
      centreE6 = pickAtm(groupStrikes(data), centre);
      loaded = true;
    } catch (e) {
      error = String(e);
      rows = [];
      centreE6 = null;
      loaded = false;
    } finally {
      loading = false;
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
            if (spotKey && quote.key === spotKey) spot = quote;
          }
        });
        if (disposed) onDelta();
        else unlisteners.push(onDelta);

        const onWatch = await events.onWatch(() => {
          if (!disposed) void refreshSpot();
        });
        if (disposed) onWatch();
        else unlisteners.push(onWatch);

        // Expiries are local. The chain is NOT loaded automatically: subscribing
        // is an explicit action so the broker is only touched on request.
        await loadExpiries();

        // Opened from a row context menu: load that underlying straight away.
        if (pending) {
          underlying = pending.underlying;
          atm = "";
          quick = "";
          centreE6 = null;
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

    <div class="field" style="width: 125px">
      <label for="chain-strikes">Far strikes</label>
      <input id="chain-strikes" type="number" min="1" max="20" bind:value={strikes} />
    </div>

    <div class="field" style="width: 130px">
      <label for="chain-atm">ATM (optional)</label>
      <input id="chain-atm" bind:value={atm} placeholder="e.g. 25800" />
    </div>

    <div class="field" style="width: 130px">
      <label for="chain-quick">Filter strike</label>
      <input id="chain-quick" bind:value={quick} placeholder="258" />
    </div>

    <button onclick={ondefaults}>Defaults</button>

    <button class="primary" disabled={loading || !expiry} onclick={loadChain}>
      {loading ? "Loading..." : "Load and subscribe"}
    </button>
  </div>

  {#if error}
    <div class="banner"><span>{error}</span></div>
  {/if}

  {#if loaded}
    <div class="chain-summary">
      <span class="spot-item">Spot <strong>{spot?.ltp?.text ?? "-"}</strong></span>
      <span class="spot-item">ATM <strong>{fmtStrike(atmE6)}</strong></span>
      <span class="spot-item">Expiry <strong>{expiry || "-"}</strong></span>
      <span class="spacer"></span>
      <span class="chain-legend">
        <i class="swatch itm"></i>ITM
        <i class="swatch atm"></i>ATM
        <i class="swatch otm"></i>OTM
      </span>
    </div>
  {/if}

  {#if !loaded}
    <div class="empty">
      <h3>Nothing loaded</h3>
      <p class="hint">
        Pick an underlying and expiry, then press Load and subscribe. Only the strikes window is
        subscribed (100 instruments max across the whole app); other strikes stay in the local
        master as metadata and are not streamed.
      </p>
    </div>
  {:else if visibleRows.length === 0}
    <div class="empty">
      <h3>No contracts match</h3>
      <p class="hint">Widen the strike range or clear the strike filter.</p>
    </div>
  {:else}
    <div class="grid-wrap chain-wrap">
      <table class="grid chain-grid">
        <thead>
          <tr>
            <th colspan="10" class="calls-band">CALLS</th>
            <th rowspan="2" class="strike-band">Strike price</th>
            <th colspan="10" class="puts-band">PUTS</th>
          </tr>
          <tr>
            <th class="trade-head">Trade</th>
            <th>OI</th>
            <th>d OI</th>
            <th>Volume</th>
            <th>LTP</th>
            <th>Chg</th>
            <th>Bid Qty</th>
            <th>Bid</th>
            <th>Ask</th>
            <th>Ask Qty</th>

            <th>Bid Qty</th>
            <th>Bid</th>
            <th>Ask</th>
            <th>Ask Qty</th>
            <th>LTP</th>
            <th>Chg</th>
            <th>Volume</th>
            <th>d OI</th>
            <th>OI</th>
            <th class="trade-head">Trade</th>
          </tr>
        </thead>
        <tbody>
          {#each visibleRows as row (row.strike)}
            <tr
              class:atm-row={atmE6 !== null && row.strikeE6 === atmE6}
              onmouseenter={() => {
                if (row.ce) onactive(row.ce.key, row.ce.symbol);
              }}
            >
              <td
                class="trade-cell"
                onmouseenter={() => {
                  if (row.ce) onactive(row.ce.key, row.ce.symbol);
                }}
              >
                {#if row.ce}
                  <button
                    class="trade-buy"
                    title={`Buy ${row.ce.symbol}`}
                    onclick={() => onorder(row.ce!.key, "B")}>B</button
                  >
                  <button
                    class="trade-sell"
                    title={`Sell ${row.ce.symbol}`}
                    onclick={() => onorder(row.ce!.key, "S")}>S</button
                  >
                {/if}
              </td>

              {#each cells(row, "CE") as cell}
                <td class={cell.cls}>
                  {#if cell.fill > 0}
                    <span class="chain-fill {cell.fillClass}" style="width: {cell.fill}%"></span>
                  {/if}
                  <span class="cell-text">{cell.text}</span>
                </td>
              {/each}

              <td class="num mono strike-cell" class:strike-atm={atmE6 !== null && row.strikeE6 === atmE6}>
                {row.strike}
              </td>

              {#each cells(row, "PE") as cell}
                <td class={cell.cls}>
                  {#if cell.fill > 0}
                    <span class="chain-fill {cell.fillClass}" style="width: {cell.fill}%"></span>
                  {/if}
                  <span class="cell-text">{cell.text}</span>
                </td>
              {/each}

              <td
                class="trade-cell"
                onmouseenter={() => {
                  if (row.pe) onactive(row.pe.key, row.pe.symbol);
                }}
              >
                {#if row.pe}
                  <button
                    class="trade-buy"
                    title={`Buy ${row.pe.symbol}`}
                    onclick={() => onorder(row.pe!.key, "B")}>B</button
                  >
                  <button
                    class="trade-sell"
                    title={`Sell ${row.pe.symbol}`}
                    onclick={() => onorder(row.pe!.key, "S")}>S</button
                  >
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>
