<script lang="ts">
  import type { QuoteRow } from "../../state";
  import EmptyState from "../../components/EmptyState.svelte";

  interface Props {
    rows: QuoteRow[];
    selected: string | null;
    busy: boolean;
    onselect: (key: string) => void;
    onremove: (key: string) => void;
    onstart: () => void;
  }
  let { rows, selected, busy, onselect, onremove, onstart }: Props = $props();
</script>

<section class="panel">
  <header class="panel-head">
    <h2>Market watch</h2>
    <span class="muted">{rows.length} instrument{rows.length === 1 ? "" : "s"}</span>
  </header>

  {#if rows.length === 0}
    <EmptyState
      title="Nothing watched yet"
      hint="Start the bundled demo feed to see live-updating quotes, or add instruments from the Instruments page."
      action="Start demo feed"
      {busy}
      onaction={onstart}
    />
  {:else}
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
              onclick={() => onselect(row.key)}
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
                  }}>×</button
                >
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>
