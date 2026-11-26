<script lang="ts">
  import { onMount } from "svelte";
  import {
    api,
    events,
    type AppInfo,
    type InstrumentDto,
    type QuoteDto,
    type SessionInfo,
    type StatusDto,
  } from "./lib/api";
  import { QuoteTable, type QuoteRow } from "./lib/state";
  import StatusBar from "./lib/components/StatusBar.svelte";
  import Badge from "./lib/components/Badge.svelte";
  import MarketWatch from "./lib/features/market/MarketWatch.svelte";
  import SnapQuote from "./lib/features/market/SnapQuote.svelte";
  import InstrumentSearch from "./lib/features/instruments/InstrumentSearch.svelte";
  import Diagnostics from "./lib/features/diagnostics/Diagnostics.svelte";
  import OptionChain from "./lib/features/chain/OptionChain.svelte";

  type Page = "market" | "chain" | "instruments" | "diagnostics";

  const table = new QuoteTable();

  let page = $state<Page>("market");
  let rows = $state<QuoteRow[]>([]);
  let quoteMap = $state<Map<string, QuoteDto>>(new Map());
  let status = $state<StatusDto | null>(null);
  let info = $state<AppInfo | null>(null);
  let session = $state<SessionInfo | null>(null);
  let selected = $state<string | null>(null);
  let instrumentMeta = $state<InstrumentDto | null>(null);
  let notice = $state<string | null>(null);
  let busy = $state(false);

  const selectedQuote = $derived(selected ? (quoteMap.get(selected) ?? null) : null);
  const watchedKeys = $derived(rows.map((row) => row.key));

  function syncRows() {
    rows = table.rows();
    if (!selected && rows.length > 0) selected = rows[0].key;
  }

  function rememberQuotes(list: QuoteDto[]) {
    if (list.length === 0) return;
    const next = new Map(quoteMap);
    for (const quote of list) next.set(quote.key, quote);
    // Bound the map to what is actually watched.
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

  async function startDemo() {
    busy = true;
    notice = null;
    try {
      await api.startReplay(120);
      // Belt and braces: the watch:changed event normally drives the grid, but
      // re-hydrating means a dropped event can never leave the UI stale.
      await hydrate();
    } catch (error) {
      notice = `Could not start the demo feed: ${String(error)}`;
    } finally {
      busy = false;
    }
  }

  async function stopFeed() {
    notice = null;
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
        notice = `Watch limit reached â€” ${plan.rejected.length} instrument(s) were not added.`;
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

  // Fetch static instrument metadata for the selected scrip. The cleanup flag
  // prevents a stale response from overwriting a newer selection.
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

  onMount(() => {
    const unlisteners: Array<() => void> = [];
    let disposed = false;

    function track(unlisten: () => void) {
      if (disposed) unlisten();
      else unlisteners.push(unlisten);
    }

    // Each listener is registered independently: one failure (for example a
    // missing capability) must not prevent hydration or blank the app.
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
              void api.status().then((next) => {
                if (!disposed) status = next;
              });
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
      } catch (error) {
        if (!disposed) notice = `Backend unavailable: ${String(error)}`;
      }
    })();

    // Every listener and pending fetch is released here.
    return () => {
      disposed = true;
      for (const unlisten of unlisteners) unlisten();
    };
  });
</script>

<div class="shell">
  <header class="topbar">
    <div class="brand">H-Finvasia <span>market terminal</span></div>

    <Badge
      label={status?.connected ? (status.source === "live" ? "live" : "replay") : "disconnected"}
      tone={status?.connected ? "ok" : "warn"}
      dot
    />
    <Badge label={status?.backend ?? "â€¦"} tone={status?.backend === "mongo" ? "ok" : "warn"} />
    {#if status?.master_loaded === false}
      <Badge label="master loading" tone="warn" />
    {/if}

    <span class="spacer"></span>

    <button disabled={busy} onclick={startDemo}>{busy ? "Startingâ€¦" : "Demo feed"}</button>
    <button disabled={!status?.connected} onclick={stopFeed}>Stop</button>
  </header>

  <div class="body">
    <nav class="sidebar" aria-label="Sections">
      <div class="nav-title">Terminal</div>
      <button
        class="nav-item"
        class:active={page === "market"}
        aria-current={page === "market" ? "page" : undefined}
        onclick={() => (page = "market")}>Market watch</button
      >
      <button
        class="nav-item"
        class:active={page === "chain"}
        aria-current={page === "chain" ? "page" : undefined}
        onclick={() => (page = "chain")}>Option chain</button
      >
      <button
        class="nav-item"
        class:active={page === "instruments"}
        aria-current={page === "instruments" ? "page" : undefined}
        onclick={() => (page = "instruments")}>Instruments</button
      >
      <button
        class="nav-item"
        class:active={page === "diagnostics"}
        aria-current={page === "diagnostics" ? "page" : undefined}
        onclick={() => (page = "diagnostics")}>Diagnostics</button
      >
    </nav>

    <div class="content">
      {#if notice}
        <div class="banner">
          <span>{notice}</span>
          <button class="ghost" onclick={() => (notice = null)}>Dismiss</button>
        </div>
      {/if}

      <main class="main" class:with-snap={page === "market"}>
        {#if page === "chain"}
          <OptionChain watched={watchedKeys} onadd={addInstrument} />
        {:else if page === "market"}
          <MarketWatch
            {rows}
            {selected}
            {busy}
            onselect={(key) => (selected = key)}
            onremove={removeInstrument}
            onstart={startDemo}
          />
          <SnapQuote quote={selectedQuote} instrument={instrumentMeta} onremove={removeInstrument} />
        {:else if page === "instruments"}
          <InstrumentSearch watched={watchedKeys} onadd={addInstrument} />
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
      </main>
    </div>
  </div>

  <StatusBar {status} />
</div>
