<script lang="ts">
  import { onMount } from "svelte";
  import { api, events, type ChainRowDto, type GroupSummary, type QuoteDto } from "../../api";
  import { formatVolume } from "../../state";
  import Badge from "../../components/Badge.svelte";
  import ContextMenu from "../../components/ContextMenu.svelte";
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

  let underlying = $state("");
  let committedUnderlying = $state("");
  let underlyings = $state<string[]>([]);
  let showUnderlyings = $state(false);
  let expiries = $state<string[]>([]);
  let expiry = $state("");
  let strikes = $state(10);
  let atm = $state("");
  let quick = $state("");
  let rows = $state<ChainRowDto[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let loaded = $state(false);
  let menu = $state<{ x: number; y: number; key: string; label: string } | null>(null);
  let groups = $state<GroupSummary[]>([]);
  let notice = $state<string | null>(null);

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

  const suggestions = $derived.by(() => {
    const needle = underlying.trim().toUpperCase();
    if (needle === "" || !showUnderlyings) return [];
    return underlyings
      .filter((name) => name.includes(needle))
      .slice(0, 12);
  });

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

  async function loadGroups() {
    try {
      groups = (await api.containersList()).groups;
    } catch (e) {
      error = String(e);
    }
  }

  function openAddMenu(event: MouseEvent, leg: ChainRowDto | null | undefined) {
    if (!leg) return;
    event.preventDefault();
    event.stopPropagation();
    menu = { x: event.clientX, y: event.clientY, key: leg.key, label: leg.symbol };
    void loadGroups();
  }

  async function addToGroup(group: string, key: string, label: string) {
    error = null;
    try {
      await api.containerAdd(group, key);
      notice = `Added ${label} to "${group}".`;
    } catch (e) {
      error = String(e);
    }
  }

  async function createCardAndAdd(name: string, key: string, label: string) {
    error = null;
    try {
      await api.containerCreateEmpty(name);
      await api.containerAdd(name, key);
      notice = `Created "${name}" and added ${label}.`;
      await loadGroups();
    } catch (e) {
      error = String(e);
    }
  }

  function resetChain() {
    expiries = [];
    expiry = "";
    rows = [];
    loaded = false;
    centreE6 = null;
    spot = null;
    spotKey = null;
    quick = "";
  }

  function onUnderlyingInput(event: Event) {
    const value = (event.currentTarget as HTMLInputElement).value;
    underlying = value;
    showUnderlyings = true;
    if (value.trim().toUpperCase() !== committedUnderlying) {
      committedUnderlying = "";
      resetChain();
    }
  }

  async function selectUnderlying(name: string) {
    underlying = name;
    committedUnderlying = name;
    showUnderlyings = false;
    resetChain();
    error = null;
    await loadExpiries();
  }

  function onUnderlyingKey(event: KeyboardEvent) {
    if (event.key === "Escape") {
      showUnderlyings = false;
      return;
    }
    if (event.key === "Enter") {
      const exact = underlyings.find(
        (name) => name === underlying.trim().toUpperCase(),
      );
      const pick = suggestions[0] ?? exact;
      if (pick) {
        event.preventDefault();
        void selectUnderlying(pick);
      }
    }
  }

  async function loadExpiries() {
    const name = committedUnderlying.trim();
    if (name === "") {
      expiries = [];
      expiry = "";
      return;
    }
    error = null;
    try {
      const list = await api.chainExpiries(name);
      expiries = list;
      if (list.length === 0) {
        error = `No expiries found for "${name}".`;
        expiry = "";
      } else if (!list.includes(expiry)) {
        expiry = list[0];
      }
    } catch (e) {
      error = String(e);
    }
  }

  async function refreshSpot() {
    const wanted = committedUnderlying.trim().toUpperCase();
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
    const name = committedUnderlying.trim();
    if (!name || !expiry) return;
    loading = true;
    error = null;
    try {
      // Prefer a live index/stock quote as the ATM when one is watched; the
      // backend otherwise centres on the middle of the full chain, which is
      // rarely where the market is.
      await refreshSpot();
      const centre = atm.trim() !== "" ? atm.trim() : (spot?.ltp?.text ?? null);
      const data = await api.chainLoad(
        name,
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

        const onMaster = await events.onMasterReady(() => {
          if (disposed) return;
          void api.chainUnderlyings("", 500).then((list) => {
            if (!disposed) underlyings = list;
          });
        });
        if (disposed) onMaster();
        else unlisteners.push(onMaster);

        underlyings = await api.chainUnderlyings("", 500).catch(() => []);

        // Direct navigation starts blank: the user types, picks an underlying,
        // then the expiry/strike controls appear. Opening from a row context
        // menu is the only path that pre-fills and auto-loads.
        if (pending) {
          underlying = pending.underlying;
          committedUnderlying = pending.underlying;
          showUnderlyings = false;
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
    <div
      class="field chain-underlying-field"
      onfocusout={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
          showUnderlyings = false;
        }
      }}
    >
      <label for="chain-underlying">Underlying</label>
      <input
        id="chain-underlying"
        value={underlying}
        placeholder="Type e.g. NIFTY, RELIANCE"
        autocomplete="off"
        spellcheck="false"
        oninput={onUnderlyingInput}
        onfocus={() => {
          if (underlying.trim() !== "") showUnderlyings = true;
        }}
        onkeydown={onUnderlyingKey}
      />
      {#if showUnderlyings && suggestions.length > 0}
        <div class="underlying-list" role="listbox" aria-label="Matching underlyings">
          {#each suggestions as name (name)}
            <button type="button" role="option" aria-selected={name === committedUnderlying} onclick={() => selectUnderlying(name)}>
              {name}
            </button>
          {/each}
        </div>
      {:else if showUnderlyings && underlying.trim() !== ""}
        <div class="underlying-empty">No derivative underlying matches.</div>
      {/if}
    </div>

    {#if committedUnderlying}
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
    {/if}
  </div>

  {#if error}
    <div class="banner"><span>{error}</span></div>
  {/if}
  {#if notice}
    <div class="banner ok-banner">
      <span>{notice}</span>
      <button class="ghost" onclick={() => (notice = null)}>Dismiss</button>
    </div>
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
            <tr class:atm-row={atmE6 !== null && row.strikeE6 === atmE6}>
              <td
                class="trade-cell"
                onmouseenter={() => {
                  if (row.ce) onactive(row.ce.key, row.ce.symbol);
                }}
                onclick={() => {
                  if (row.ce) onactive(row.ce.key, row.ce.symbol);
                }}
                oncontextmenu={(event) => openAddMenu(event, row.ce)}
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
                <td
                  class={cell.cls}
                  onmouseenter={() => {
                    if (row.ce) onactive(row.ce.key, row.ce.symbol);
                  }}
                  onclick={() => {
                    if (row.ce) onactive(row.ce.key, row.ce.symbol);
                  }}
                  oncontextmenu={(event) => openAddMenu(event, row.ce)}
                >
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
                <td
                  class={cell.cls}
                  onmouseenter={() => {
                    if (row.pe) onactive(row.pe.key, row.pe.symbol);
                  }}
                  onclick={() => {
                    if (row.pe) onactive(row.pe.key, row.pe.symbol);
                  }}
                  oncontextmenu={(event) => openAddMenu(event, row.pe)}
                >
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
                onclick={() => {
                  if (row.pe) onactive(row.pe.key, row.pe.symbol);
                }}
                oncontextmenu={(event) => openAddMenu(event, row.pe)}
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

  {#if menu}
    <ContextMenu
      x={menu.x}
      y={menu.y}
      items={[
        {
          label: `Add ${menu.label} to card`,
          children: groups.map((group) => ({
            label: `${group.name} (${group.count})`,
            action: () => {
              if (menu) void addToGroup(group.name, menu.key, menu.label);
            },
          })),
        },
      ]}
      creator={{
        label: "Create",
        placeholder: "New card name",
        action: (name) => {
          if (menu) void createCardAndAdd(name, menu.key, menu.label);
        },
      }}
      onclose={() => (menu = null)}
    />
  {/if}
</section>
