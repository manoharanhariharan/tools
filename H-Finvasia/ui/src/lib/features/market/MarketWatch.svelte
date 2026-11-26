<script lang="ts">
  import type { QuoteRow } from "../../state";
  import EmptyState from "../../components/EmptyState.svelte";
  import ContextMenu from "../../components/ContextMenu.svelte";

  interface Props {
    rows: QuoteRow[];
    selected: string | null;
    onselect: (key: string) => void;
    onremove: (key: string) => void;
    onchain: (underlying: string) => void;
    onactive: (key: string, label: string) => void;
  }
  let { rows, selected, onselect, onremove, onchain, onactive }: Props = $props();

  let menu = $state<{ x: number; y: number; row: QuoteRow } | null>(null);

  // Watched indices become the spot-rate strip (no extra subscriptions).
  const indexRows = $derived(rows.filter((row) => row.isIndex));
</script>

<section class="panel">
  <header class="panel-head">
    <h2>Market watch</h2>
    <span class="muted">{rows.length} instrument{rows.length === 1 ? "" : "s"}</span>
  </header>

  {#if rows.length === 0}
    <EmptyState
      title="Nothing watched yet"
      hint="Add instruments from the Instruments page, or from a Stocks group card."
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
