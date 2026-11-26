<script lang="ts">
  import { onMount } from "svelte";
  import {
    api,
    events,
    type AccountSummaryDto,
    type AppInfo,
    type InstrumentDto,
    type QuoteDto,
    type SessionInfo,
    type StatusDto,
  } from "./lib/api";
  import { QuoteTable, type QuoteRow } from "./lib/state";
  import StatusBar from "./lib/components/StatusBar.svelte";
  import BottomDock from "./lib/components/BottomDock.svelte";
  import AccountStrip from "./lib/features/account/AccountStrip.svelte";
  import MarketWatch from "./lib/features/market/MarketWatch.svelte";
  import SnapQuote from "./lib/features/market/SnapQuote.svelte";
  import OptionChain from "./lib/features/chain/OptionChain.svelte";
  import IndexWatch from "./lib/features/indices/IndexWatch.svelte";
  import Stocks from "./lib/features/stocks/Stocks.svelte";
  import InstrumentSearch from "./lib/features/instruments/InstrumentSearch.svelte";
  import Groups from "./lib/features/groups/Groups.svelte";
  import Diagnostics from "./lib/features/diagnostics/Diagnostics.svelte";

  type Page =
    | "index"
    | "stocks"
    | "chain"
    | "instruments"
    | "market"
    | "groups"
    | "diagnostics";

  const table = new QuoteTable();

  let page = $state<Page>("index");
  let rows = $state<QuoteRow[]>([]);
  let quoteMap = $state<Map<string, QuoteDto>>(new Map());
  let status = $state<StatusDto | null>(null);
  let info = $state<AppInfo | null>(null);
  let session = $state<SessionInfo | null>(null);
  let summary = $state<AccountSummaryDto | null>(null);
  let summaryLoading = $state(false);
  let selected = $state<string | null>(null);
  let instrumentMeta = $state<InstrumentDto | null>(null);
  let notice = $state<string | null>(null);
  let busy = $state(false);
  let moreOpen = $state(false);
  /// Set by the row context menu; OptionChain consumes it and loads.
  let chainRequest = $state<{ underlying: string; nonce: number } | null>(null);
  let chainNonce = 0;

  const selectedQuote = $derived(selected ? (quoteMap.get(selected) ?? null) : null);
  const watchedKeys = $derived(rows.map((row) => row.key));
  const connected = $derived(status?.connected ?? false);

  function syncRows() {
    rows = table.rows();
    if (!selected && rows.length > 0) selected = rows[0].key;
  }

  function rememberQuotes(list: QuoteDto[]) {
    if (list.length === 0) return;
    const next = new Map(quoteMap);
    for (const quote of list) next.set(quote.key, quote);
    const keep = new Set(table.keys());
    for (const key of [...next.keys()]) if (!keep.has(key)) next.delete(key);
    quoteMap = next;
  }

  async function hydrate() {
    const [keys, quotes, current] = await Promise.all([
      api.watchList(),
      api.watchQuotes(),
      api.status(),
    ]);
    table.setKeys(keys, quotes);
    syncRows();
    rememberQuotes(quotes);
    status = current;
  }

  async function refreshSummary() {
    summaryLoading = true;
    try {
      summary = await api.accountSummary();
    } catch (error) {
      summary = null;
      console.warn("account summary failed", error);
    } finally {
      summaryLoading = false;
    }
  }

  async function startDemo() {
    busy = true;
    notice = null;
    moreOpen = false;
    try {
      await api.startReplay(120);
      await hydrate();
    } catch (error) {
      notice = `Could not start the demo feed: ${String(error)}`;
    } finally {
      busy = false;
    }
  }

  async function stopFeed() {
    notice = null;
    moreOpen = false;
    try {
      await api.stopFeed();
    } catch (error) {
      notice = String(error);
    }
  }

  async function startLive() {
    busy = true;
    notice = null;
    try {
      await api.startLive();
      session = await api.sessionInfo();
      await hydrate();
      await refreshSummary();
    } catch (error) {
      notice = String(error);
    } finally {
      busy = false;
    }
  }

  async function addInstrument(key: string) {
    notice = null;
    try {
      const plan = await api.watchAdd(key);
      if (plan.truncated) {
        notice = `Watch limit reached - ${plan.rejected.length} instrument(s) not added (max 100).`;
      }
      await hydrate();
      selected = key;
    } catch (error) {
      notice = String(error);
    }
  }

  async function removeInstrument(key: string) {
    notice = null;
    try {
      await api.watchRemove(key);
      if (selected === key) selected = null;
      await hydrate();
    } catch (error) {
      notice = String(error);
    }
  }

  function go(next: Page) {
    page = next;
    moreOpen = false;
  }

  /// Open the option chain for an instrument whose underlying has derivatives.
  function openChain(underlying: string) {
    chainNonce += 1;
    chainRequest = { underlying, nonce: chainNonce };
    page = "chain";
    moreOpen = false;
  }

  // Metadata for the selected scrip; cancelled if the selection changes.
  $effect(() => {
    const key = selected;
    if (!key) {
      instrumentMeta = null;
      return;
    }
    let cancelled = false;
    api
      .instrument(key)
      .then((meta) => {
        if (!cancelled) instrumentMeta = meta;
      })
      .catch(() => {
        if (!cancelled) instrumentMeta = null;
      });
    return () => {
      cancelled = true;
    };
  });

  // Refresh the account strip when the feed first becomes connected, but never
  // on a timer: the broker is polled only when something actually changed.
  let wasConnected = false;
  $effect(() => {
    const nowConnected = status?.connected ?? false;
    if (nowConnected && !wasConnected) {
      wasConnected = true;
      void refreshSummary();
    } else if (!nowConnected) {
      wasConnected = false;
    }
  });

  onMount(() => {
    const unlisteners: Array<() => void> = [];
    let disposed = false;

    function track(unlisten: () => void) {
      if (disposed) unlisten();
      else unlisteners.push(unlisten);
    }

    async function subscribeAll() {
      const subscriptions: Array<[string, () => Promise<() => void>]> = [
        [
          "quotes:delta",
          () =>
            events.onDelta((delta) => {
              table.upsert(delta.quotes);
              syncRows();
              rememberQuotes(delta.quotes);
            }),
        ],
        [
          "watch:changed",
          () =>
            events.onWatch((payload) => {
              table.setKeys(payload.keys, payload.quotes);
              syncRows();
              rememberQuotes(payload.quotes);
            }),
        ],
        [
          "status:changed",
          () =>
            events.onStatus((next) => {
              status = next;
            }),
        ],
        [
          "master:ready",
          () =>
            events.onMasterReady(() => {
              if (disposed) return;
              void api.status().then((next) => {
                if (!disposed) status = next;
              });
              // Rows restored before the master finished have no metadata
              // (underlying / F&O / index). Re-hydrate so right-click actions
              // and the spot-rate strip work before the first tick.
              void hydrate();
            }),
        ],
        [
          "feed:changed",
          () =>
            events.onFeed(() => {
              if (!disposed) notice = null;
            }),
        ],
      ];

      for (const [name, subscribe] of subscriptions) {
        try {
          track(await subscribe());
        } catch (error) {
          console.warn(`event listener '${name}' failed`, error);
        }
      }
    }

    (async () => {
      try {
        await subscribeAll();
        const appInfo = await api.appInfo();
        if (!disposed) info = appInfo;
        const sessionInfo = await api.sessionInfo();
        if (!disposed) session = sessionInfo;
        if (!disposed) await hydrate();
        if (!disposed) await refreshSummary();
      } catch (error) {
        if (!disposed) notice = `Backend unavailable: ${String(error)}`;
      }
    })();

    return () => {
      disposed = true;
      for (const unlisten of unlisteners) unlisten();
    };
  });
</script>

<div class="shell">
  <AccountStrip
    {summary}
    loading={summaryLoading}
    onrefresh={refreshSummary}
    source={status?.source ?? "idle"}
  />

  <div class="workspace-tabs">
    <button class="ws-tab" class:active={page === "index"} onclick={() => go("index")}>Index</button>
    <button class="ws-tab" class:active={page === "stocks"} onclick={() => go("stocks")}>Stocks</button>
    <button class="ws-tab" class:active={page === "chain"} onclick={() => go("chain")}>
      Option chain
    </button>
    <button class="ws-tab" class:active={page === "instruments"} onclick={() => go("instruments")}>
      Instruments
    </button>

    <span class="spacer"></span>

    <button class="ghost" class:active={page === "market"} onclick={() => go("market")}>
      Market watch
    </button>

    <div class="menu-wrap">
      <button class="ghost" onclick={() => (moreOpen = !moreOpen)} aria-haspopup="menu">
        More
      </button>
      {#if moreOpen}
        <div class="menu" role="menu">
          <button role="menuitem" onclick={() => go("groups")}>Groups</button>
          <button role="menuitem" onclick={() => go("diagnostics")}>Diagnostics</button>
          <div class="menu-sep"></div>
          <button role="menuitem" disabled={busy} onclick={startDemo}>Demo feed</button>
          <button role="menuitem" disabled={!connected} onclick={stopFeed}>Stop feed</button>
        </div>
      {/if}
    </div>
  </div>

  {#if notice}
    <div class="banner">
      <span>{notice}</span>
      <button class="ghost" onclick={() => (notice = null)}>Dismiss</button>
    </div>
  {/if}

  <div class="workspace-body">
    {#if page === "index"}
      <IndexWatch
        onadd={addInstrument}
        onchain={openChain}
        refreshToken={watchedKeys.length}
      />
    {:else if page === "stocks"}
      <Stocks
        watched={watchedKeys}
        onadd={addInstrument}
        onremove={removeInstrument}
        onchain={openChain}
      />
    {:else if page === "chain"}
      <OptionChain
        source={status?.source ?? "idle"}
        pending={chainRequest}
        onconsumed={() => (chainRequest = null)}
      />
    {:else if page === "instruments"}
      <InstrumentSearch watched={watchedKeys} onadd={addInstrument} onchain={openChain} />
    {:else if page === "market"}
      <div class="ws-split">
        <MarketWatch
          {rows}
          {selected}
          {busy}
          onselect={(key) => (selected = key)}
          onremove={removeInstrument}
          onstart={startDemo}
          onchain={openChain}
        />
        <SnapQuote quote={selectedQuote} instrument={instrumentMeta} onremove={removeInstrument} />
      </div>
    {:else if page === "groups"}
      <Groups watchCount={rows.length} />
    {:else}
      <Diagnostics
        {status}
        {info}
        {session}
        {busy}
        {notice}
        ondemo={startDemo}
        onstop={stopFeed}
        onlive={startLive}
        onnotice={(message) => (notice = message)}
      />
    {/if}
  </div>

  <BottomDock />

  <StatusBar {status} />
</div>
