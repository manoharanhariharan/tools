<script lang="ts">
  import { api, type InstrumentDto } from "../../api";
  import type { QuoteRow } from "../../state";
  import EmptyState from "../../components/EmptyState.svelte";
  import ContextMenu from "../../components/ContextMenu.svelte";

  interface Props {
    rows: QuoteRow[];
    selected: string | null;
    watched: string[];
    onadd: (key: string) => void;
    onselect: (key: string) => void;
    onremove: (key: string) => void;
    onchain: (underlying: string) => void;
    onactive: (key: string, label: string) => void;
  }
  let { rows, selected, watched, onadd, onselect, onremove, onchain, onactive }: Props =
    $props();

  let menu = $state<{ x: number; y: number; row: QuoteRow } | null>(null);
  let query = $state("");
  let results = $state<InstrumentDto[]>([]);
  let searching = $state(false);
  let searchOpen = $state(false);
  let searchError = $state<string | null>(null);
  let searchSeq = 0;
  const watchedSet = $derived(new Set(watched));

  $effect(() => {
    const text = query.trim();
    if (text === "") {
      results = [];
      searching = false;
      searchOpen = false;
      searchError = null;
      return;
    }
    const seq = ++searchSeq;
    searching = true;
    searchOpen = true;
    const timer = setTimeout(async () => {
      try {
        const list = await api.search(text, 30);
        if (seq === searchSeq) results = list;
      } catch (e) {
        if (seq === searchSeq) {
          results = [];
          searchError = String(e);
        }
      } finally {
        if (seq === searchSeq) searching = false;
      }
    }, 170);
    return () => clearTimeout(timer);
  });

  function addInstrument(item: InstrumentDto) {
    if (watchedSet.has(item.key)) return;
    onadd(item.key);
  }

  // Watched indices become the spot-rate strip (no extra subscriptions).
  const indexRows = $derived(rows.filter((row) => row.isIndex));
</script>

<section class="panel">
  <header class="panel-head">
    <h2>Market watch</h2>
    <span class="muted">{rows.length} instrument{rows.length === 1 ? "" : "s"}</span>
  </header>

  <div class="stock-search-wrap">
    <div class="field">
      <label for="market-search">Search instruments to add to the watch list</label>
      <input
        id="market-search"
        type="search"
        value={query}
        placeholder="Symbol, underlying or company name"
        autocomplete="off"
        spellcheck="false"
        oninput={(event) => {
          query = event.currentTarget.value;
          searchOpen = true;
        }}
        onfocus={() => {
          if (query.trim() !== "") searchOpen = true;
        }}
        onkeydown={(event) => {
          if (event.key === "Escape") {
            query = "";
            results = [];
            searchOpen = false;
          }
        }}
      />
    </div>

    {#if searchOpen && query.trim() !== ""}
      <div class="search-flyout" role="listbox">
        {#if searching && results.length === 0}
          <div class="search-empty">Searching...</div>
        {:else if searchError}
          <div class="search-empty down">{searchError}</div>
        {:else if results.length === 0}
          <div class="search-empty">No matching instruments.</div>
        {:else}
          {#each results as item (item.key)}
            <div class="search-hit" role="option" aria-selected={watchedSet.has(item.key)}>
              <span class="symbol">{item.trading_symbol}</span>
              <span class="name">
                {item.underlying ?? item.name ?? item.instrument_type ?? "instrument"}
                {#if item.strike_type === "CE" || item.strike_type === "PE"}
                  &middot; {item.strike_type} {item.strike}
                {:else if item.expiry}
                  &middot; {item.expiry}
                {/if}
              </span>
              {#if item.is_index}<span class="badge">IDX</span>{/if}
              {#if watchedSet.has(item.key)}
                <button class="tiny" disabled>Added</button>
              {:else}
                <button class="tiny" onclick={() => addInstrument(item)}>Add</button>
              {/if}
            </div>
          {/each}
        {/if}
      </div>
    {/if}
  </div>

  {#if rows.length === 0}
    <EmptyState
      title="Nothing watched yet"
      hint="Use the search above to add an instrument, or add one from a Stocks group card."
    />
  {:else}
    {#if indexRows.length > 0}
      <div class="spot-strip">
        {#each indexRows as row (row.key)}
          <span class="spot-item">
            {row.symbol}
            <strong class={row.trend}>{row.ltp}</strong>
            <span class={row.trend}>{row.pct}</span>
          </span>
        {/each}
      </div>
    {/if}
    <div class="grid-wrap">
      <table class="grid">
        <thead>
          <tr>
            <th class="l">Symbol</th>
            <th>LTP</th>
            <th>Chg</th>
            <th>Chg%</th>
            <th>Bid</th>
            <th>Ask</th>
            <th>Vol</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each rows as row (row.key)}
            <tr
              class:selected={selected === row.key}
              tabindex="0"
              role="button"
              aria-label={`Select ${row.symbol}`}
              onmouseenter={() => onactive(row.key, row.symbol)}
              onclick={() => onselect(row.key)}
              oncontextmenu={(event) => {
                event.preventDefault();
                event.stopPropagation();
                menu = { x: event.clientX, y: event.clientY, row };
              }}
              onkeydown={(event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  onselect(row.key);
                }
              }}
            >
              <td class="l mono">{row.symbol}</td>
              <td class="num mono {row.trend}">{row.ltp}</td>
              <td class="num mono {row.trend}">{row.change}</td>
              <td class="num mono {row.trend}">{row.pct}</td>
              <td class="num mono dim">{row.bid}</td>
              <td class="num mono dim">{row.ask}</td>
              <td class="num mono dim">{row.volume}</td>
              <td class="num">
                <button
                  class="icon"
                  title="Remove from watch"
                  aria-label={`Remove ${row.symbol} from watch`}
                  onclick={(event) => {
                    event.stopPropagation();
                    onremove(row.key);
                  }}>x</button
                >
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
          label:
            menu.row.hasFno && menu.row.underlying
              ? `Open option chain (${menu.row.underlying})`
              : "No F&O for this instrument",
          disabled: !(menu.row.hasFno && menu.row.underlying),
          action: () => {
            if (menu?.row.underlying) onchain(menu.row.underlying);
          },
        },
        { label: "Remove from watch", action: () => onremove(menu!.row.key) },
      ]}
      onclose={() => (menu = null)}
    />
  {/if}
</section>
