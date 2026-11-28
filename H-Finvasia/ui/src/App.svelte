<script lang="ts">
  import { onMount } from "svelte";
  import {
    api,
    events,
    type AccountSummaryDto,
    type AppInfo,
    type InstrumentDto,
    type OrderDefaults,
    type OrderDto,
    type OrderResult,
    type PositionDto,
    type QuoteDto,
    type SessionInfo,
    type StatusDto,
  } from "./lib/api";
  import { QuoteTable, type QuoteRow } from "./lib/state";
  import { defaultOrderPrice, shortcutSide, type OrderTicketState } from "./lib/order";
  import StatusBar from "./lib/components/StatusBar.svelte";
  import BottomDock from "./lib/components/BottomDock.svelte";
  import OrderTicket from "./lib/features/chain/OrderTicket.svelte";
  import OrderDefaultsModal from "./lib/features/chain/OrderDefaultsModal.svelte";
  import ThemePicker from "./lib/components/ThemePicker.svelte";
  import { loadTheme } from "./lib/theme";
  import AccountStrip from "./lib/features/account/AccountStrip.svelte";
  import MarketWatch from "./lib/features/market/MarketWatch.svelte";
  import SnapQuote from "./lib/features/market/SnapQuote.svelte";
  import OptionChain from "./lib/features/chain/OptionChain.svelte";
  import IndexWatch from "./lib/features/indices/IndexWatch.svelte";
  import Stocks from "./lib/features/stocks/Stocks.svelte";
  import Settings from "./lib/features/settings/Settings.svelte";
  import { liveTotals } from "./lib/features/positions/live";
  import { orderNotification } from "./lib/features/orders/notify";
  import { normaliseBlankChainTabs } from "./lib/features/chain/chainTabs";

  type Page = "index" | "stocks" | "chain" | "market" | "settings";

  const table = new QuoteTable();

  let page = $state<Page>("index");
  let rows = $state<QuoteRow[]>([]);
  let quoteMap = $state<Map<string, QuoteDto>>(new Map());
  let status = $state<StatusDto | null>(null);
  let info = $state<AppInfo | null>(null);
  let session = $state<SessionInfo | null>(null);
  let summary = $state<AccountSummaryDto | null>(null);
  let summaryLoading = $state(false);
  let positions = $state<PositionDto[]>([]);
  let liveQuotes = $state<Map<string, QuoteDto>>(new Map());
  let selected = $state<string | null>(null);
  let instrumentMeta = $state<InstrumentDto | null>(null);
  let notice = $state<string | null>(null);
  let busy = $state(false);
  let orderToast = $state<{ text: string; tone: "ok" | "error" | "warn" } | null>(null);
  let orderToastTimer: ReturnType<typeof setTimeout> | undefined;
  let ordersSeeded = false;
  const ordersSeen = new Set<string>();

  function showOrderToast(text: string, tone: "ok" | "error" | "warn") {
    if (orderToastTimer !== undefined) clearTimeout(orderToastTimer);
    orderToast = { text, tone };
    orderToastTimer = setTimeout(() => {
      orderToast = null;
      orderToastTimer = undefined;
    }, 9000);
  }

  function orderSeenKey(order: OrderDto): string {
    return `${order.order_no}|${order.status}|${order.filled}`;
  }

  async function syncOrderNotices() {
    try {
      const list = await api.ordersLoad();
      if (!ordersSeeded) {
        for (const order of list) ordersSeen.add(orderSeenKey(order));
        ordersSeeded = true;
        return;
      }
      for (const order of list) {
        const key = orderSeenKey(order);
        if (ordersSeen.has(key)) continue;
        ordersSeen.add(key);
        const notification = orderNotification(order);
        if (notification) showOrderToast(notification.text, notification.tone);
      }
    } catch (error) {
      console.warn("order notification refresh failed", error);
    }
  }

  interface ChainTab {
    id: string;
    title: string;
    pinned: boolean;
    loaded: boolean;
    pending: { underlying: string; nonce: number } | null;
  }

  let chainSeq = 0;
  let chainNonce = 0;
  function createChainTab(
    pending: { underlying: string; nonce: number } | null = null,
    title = "Option chain",
  ): ChainTab {
    chainSeq += 1;
    return { id: `chain-${chainSeq}`, title, pinned: false, loaded: false, pending };
  }

  const firstChain = createChainTab();
  let chainTabs = $state<ChainTab[]>([firstChain]);
  let activeChainId = $state(firstChain.id);
  /// Last row/leg under the pointer; F1/F2 act on it.
  let activeKey = $state<string | null>(null);
  let orderDefaults = $state<OrderDefaults>({
    product: "I",
    price_type: "LMT",
    validity: "DAY",
    lots: 1,
  });
  let showOrderDefaults = $state(false);
  let orderTicket = $state<OrderTicketState | null>(null);
  const initialTheme = loadTheme();
  let theme = $state(initialTheme.theme);
  let accent = $state(initialTheme.accent);
  let showTheme = $state(false);

  const selectedQuote = $derived(selected ? (quoteMap.get(selected) ?? null) : null);
  const watchedKeys = $derived(rows.map((row) => row.key));
  const liveAccount = $derived(liveTotals(positions, liveQuotes));
  const displaySummary = $derived(
    summary && positions.length > 0
      ? {
          ...summary,
          totals: {
            ...summary.totals,
            mtm: liveAccount.mtm,
            rpnl: liveAccount.rpnl,
          },
        }
      : summary,
  );

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

  async function refreshPositions() {
    try {
      const view = await api.positionsLoad();
      positions = view.positions;
    } catch (error) {
      console.warn("positions unavailable for live totals", error);
    }
  }

  async function startLive() {
    activeKey = null;
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
      if (activeKey === key) {
        activeKey = null;
      }
      await hydrate();
    } catch (error) {
      notice = String(error);
    }
  }

  async function removeInstrumentEverywhere(key: string) {
    notice = null;
    try {
      await api.removeEverywhere(key);
      if (selected === key) selected = null;
      if (activeKey === key) {
        activeKey = null;
      }
      await hydrate();
    } catch (error) {
      notice = String(error);
    }
  }

  function go(next: Page) {
    page = next;
    activeKey = null;
  }

  /** IST trading date; the broker rolls positions/RPNL at this boundary. */
  function tradingDay(): string {
    return new Intl.DateTimeFormat("en-CA", {
      timeZone: "Asia/Kolkata",
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    }).format(new Date());
  }

  function rememberActive(key: string, _label: string) {
    activeKey = key;
    // Picking a row is the action the warning asked for; clear it immediately.
    if (notice?.includes("instrument row first")) notice = null;
  }

  async function openOrder(key: string | null, side: "B" | "S") {
    if (!key) return;
    try {
      const target = await api.orderTarget(key);
      if (!target) {
        notice = `No instrument metadata for ${key}.`;
        return;
      }
      if (!target.tradeable) {
        notice = `${target.trading_symbol} is not tradeable.`;
        return;
      }
      orderTicket = { target, side, price: defaultOrderPrice(target, side) };
    } catch (error) {
      notice = String(error);
    }
  }

  function onSubmitted(result: OrderResult) {
    orderTicket = null;
    showOrderToast(`Order ${result.status}: ${result.order_no}`, "ok");
  }

  function handleShortcut(event: KeyboardEvent) {
    const side = shortcutSide(event.key);
    if (!side) return;
    const target = event.target as HTMLElement | null;
    const tag = target?.tagName ?? "";
    if (
      tag === "INPUT" ||
      tag === "TEXTAREA" ||
      tag === "SELECT" ||
      target?.isContentEditable ||
      orderTicket !== null ||
      showOrderDefaults ||
      showTheme
    ) {
      return;
    }
    if (!activeKey) {
      event.preventDefault();
      notice = "Hover or click an instrument row first; F1 buys, F2 sells.";
      return;
    }
    event.preventDefault();
    notice = null;
    void openOrder(activeKey, side);
  }

  function updateChainTab(id: string, patch: Partial<ChainTab>) {
    chainTabs = chainTabs.map((tab) => (tab.id === id ? { ...tab, ...patch } : tab));
  }

  /// Open the option chain for an instrument in a fresh/reusable chain tab.
  function openChain(underlying: string) {
    activeKey = null;
    chainNonce += 1;
    let target = chainTabs.find((tab) => !tab.loaded && !tab.pinned && !tab.pending);
    if (!target) {
      target = createChainTab();
      chainTabs = [...chainTabs, target];
    }
    updateChainTab(target.id, {
      title: underlying,
      pending: { underlying, nonce: chainNonce },
    });
    activeChainId = target.id;
    page = "chain";
  }

  function pinChain(id: string, info: { underlying: string; expiry: string }) {
    updateChainTab(id, {
      pinned: true,
      loaded: true,
      title: `${info.underlying} ${info.expiry}`,
      pending: null,
    });
    // Keep exactly one spare blank tab, but leave focus on the pinned chain.
    chainTabs = normaliseBlankChainTabs(chainTabs, createChainTab);
  }

  function closeChainTab(id: string) {
    void api.chainClose(id).catch(() => undefined);
    if (chainTabs.length === 1) {
      const replacement = createChainTab();
      chainTabs = [replacement];
      activeChainId = replacement.id;
      return;
    }
    const index = chainTabs.findIndex((tab) => tab.id === id);
    const next = chainTabs[index + 1] ?? chainTabs[index - 1] ?? chainTabs[0];
    chainTabs = chainTabs.filter((tab) => tab.id !== id);
    if (activeChainId === id) activeChainId = next.id;
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
              if (delta.quotes.length > 0) {
                const next = new Map(liveQuotes);
                for (const quote of delta.quotes) next.set(quote.key, quote);
                liveQuotes = next;
              }
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
        [
          "feed:error",
          () =>
            events.onFeedError((message) => {
              if (disposed) return;
              if (message === "") {
                if (notice?.startsWith("Live feed:")) notice = null;
              } else {
                notice = `Live feed: ${message}`;
                if (/session rejected|handshake rejected|NOT_OK/i.test(message)) {
                  // Do not keep yesterday's RPNL/cash on screen when the token
                  // is no longer accepted.
                  summary = null;
                  positions = [];
                  liveQuotes = new Map();
                }
              }
            }),
        ],
        [
          "orders:changed",
          () =>
            events.onOrdersChanged(() => {
              if (disposed) return;
              void refreshSummary();
              void refreshPositions();
              void syncOrderNotices();
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

    let currentDay = tradingDay();
    const dayTimer = setInterval(() => {
      const next = tradingDay();
      if (next === currentDay) return;
      currentDay = next;
      // The broker resets the day book at midnight IST. Drop the previous
      // day's RPNL/positions immediately, then reload from the new session.
      summary = null;
      positions = [];
      liveQuotes = new Map();
      ordersSeen.clear();
      ordersSeeded = false;
      void hydrate();
      void refreshSummary();
      void refreshPositions();
      void syncOrderNotices();
    }, 30_000);

    (async () => {
      try {
        await subscribeAll();
        const appInfo = await api.appInfo();
        if (!disposed) info = appInfo;
        const defaults = await api.orderDefaultsGet();
        if (!disposed) orderDefaults = defaults;
        const sessionInfo = await api.sessionInfo();
        if (!disposed) session = sessionInfo;
        if (!disposed) await hydrate();
        if (!disposed) await refreshSummary();
        if (!disposed) await refreshPositions();
        if (!disposed) await syncOrderNotices();
      } catch (error) {
        if (!disposed) notice = `Backend unavailable: ${String(error)}`;
      }
    })();

    return () => {
      disposed = true;
      clearInterval(dayTimer);
      if (orderToastTimer !== undefined) clearTimeout(orderToastTimer);
      for (const unlisten of unlisteners) unlisten();
    };
  });
</script>

<svelte:window onkeydown={handleShortcut} />

<div class="shell">
  <AccountStrip
    summary={displaySummary}
    loading={summaryLoading}
    onrefresh={refreshSummary}
    source={status?.source ?? "idle"}
  />

  <div class="workspace-tabs" role="tablist" aria-label="Workspace">
    <button
      class="ws-tab intent-index"
      class:active={page === "index"}
      aria-selected={page === "index"}
      role="tab"
      onclick={() => go("index")}
    >
      <span class="tab-icon" aria-hidden="true">▦</span>Index
    </button>
    <button
      class="ws-tab intent-stocks"
      class:active={page === "stocks"}
      aria-selected={page === "stocks"}
      role="tab"
      onclick={() => go("stocks")}
    >
      <span class="tab-icon" aria-hidden="true">▤</span>Stocks
    </button>
    {#each chainTabs as tab (tab.id)}
      <div class="ws-chain-group" class:active={page === "chain" && activeChainId === tab.id}>
        <button
          class="ws-tab intent-chain"
          aria-selected={page === "chain" && activeChainId === tab.id}
          role="tab"
          onclick={() => {
            activeChainId = tab.id;
            page = "chain";
          }}
        >
          <span class="tab-icon" aria-hidden="true">⇄</span>
          {#if tab.pinned}<span class="chain-pin" aria-hidden="true">•</span>{/if}
          {tab.loaded || tab.pinned ? tab.title : "Option chain"}
        </button>
        {#if tab.pinned}
          <button
            class="ws-chain-close"
            type="button"
            title="Close pinned chain"
            aria-label={`Close ${tab.title}`}
            onclick={() => closeChainTab(tab.id)}>x</button
          >
        {/if}
      </div>
    {/each}
    <button
      class="ws-tab intent-market"
      class:active={page === "market"}
      aria-selected={page === "market"}
      role="tab"
      onclick={() => go("market")}
    >
      <span class="tab-icon" aria-hidden="true">◉</span>Market watch
    </button>
    <button
      class="ws-tab intent-settings"
      class:active={page === "settings"}
      aria-selected={page === "settings"}
      role="tab"
      onclick={() => go("settings")}
    >
      <span class="tab-icon" aria-hidden="true">⚙</span>Settings
    </button>
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
        onchain={openChain}
        onbuy={(key) => void openOrder(key, "B")}
        onsell={(key) => void openOrder(key, "S")}
        onactive={rememberActive}
        refreshToken={watchedKeys.length}
      />
    {:else if page === "stocks"}
      <Stocks
        watched={watchedKeys}
        onadd={addInstrument}
        onremove={removeInstrument}
        onchain={openChain}
        onbuy={(key) => void openOrder(key, "B")}
        onsell={(key) => void openOrder(key, "S")}
        onactive={rememberActive}
      />
    {:else if page === "market"}
      <div class="ws-split">
        <MarketWatch
          {rows}
          {selected}
          watched={watchedKeys}
          onadd={addInstrument}
          onselect={(key) => (selected = key)}
          onremove={removeInstrumentEverywhere}
          onchain={openChain}
          onbuy={(key) => void openOrder(key, "B")}
          onsell={(key) => void openOrder(key, "S")}
          onactive={rememberActive}
        />
        <SnapQuote quote={selectedQuote} instrument={instrumentMeta} onremove={removeInstrument} />
      </div>
    {:else if page === "settings"}
      <Settings
        {status}
        {info}
        {session}
        {busy}
        onlive={startLive}
        onorderdefaults={() => (showOrderDefaults = true)}
        onappearance={() => (showTheme = true)}
      />
    {/if}

    <div class="chain-workspace" class:hidden={page !== "chain"}>
      <div class="chain-views">
        {#each chainTabs as tab (tab.id)}
          <div class="chain-view" class:hidden={tab.id !== activeChainId}>
            <OptionChain
              chainId={tab.id}
              pinned={tab.pinned}
              pending={tab.pending}
              onconsumed={() => updateChainTab(tab.id, { pending: null })}
              onloaded={(info) =>
                updateChainTab(tab.id, {
                  loaded: true,
                  title: tab.pinned ? tab.title : `${info.underlying} ${info.expiry}`,
                })}
              onpin={(info) => pinChain(tab.id, info)}
              onorder={openOrder}
              ondefaults={() => (showOrderDefaults = true)}
              onactive={rememberActive}
            />
          </div>
        {/each}
      </div>
    </div>
  </div>

  <BottomDock onnotify={showOrderToast} />

  <StatusBar {status} />

  {#if orderToast}
    <div class="order-toast {orderToast.tone}" role="status" aria-live="polite">
      <span class="order-toast-text">{orderToast.text}</span>
      <button class="ghost" onclick={() => (orderToast = null)}>Dismiss</button>
    </div>
  {/if}

  {#if orderTicket}
    <OrderTicket
      ticket={orderTicket}
      defaults={orderDefaults}
      live={status?.source === "live"}
      onclose={() => (orderTicket = null)}
      onsubmitted={onSubmitted}
      onerror={(message) => showOrderToast(`Order failed: ${message}`, "error")}
    />
  {/if}

  {#if showTheme}
    <ThemePicker {theme} {accent} onclose={() => (showTheme = false)} />
  {/if}

  {#if showOrderDefaults}
    <OrderDefaultsModal
      defaults={orderDefaults}
      onclose={() => (showOrderDefaults = false)}
      onsaved={(saved) => {
        orderDefaults = saved;
        showOrderDefaults = false;
      }}
    />
  {/if}
</div>
