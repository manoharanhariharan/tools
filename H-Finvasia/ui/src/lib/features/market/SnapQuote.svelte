<script lang="ts">
  import type { InstrumentDto, QuoteDto } from "../../api";
  import { formatVolume } from "../../state";

  interface Props {
    quote: QuoteDto | null;
    instrument: InstrumentDto | null;
    onremove: (key: string) => void;
  }
  let { quote, instrument, onremove }: Props = $props();

  const title = $derived(instrument?.trading_symbol ?? quote?.trading_symbol ?? quote?.key ?? "—");
  const trend = $derived(quote?.trend ?? "flat");
</script>

<aside class="panel snap">
  <header class="panel-head">
    <h2>Snap quote</h2>
    <span class="spacer"></span>
    {#if quote}
      <button class="icon" title="Remove from watch" onclick={() => onremove(quote.key)}>×</button>
    {/if}
  </header>

  {#if !quote}
    <div class="empty">
      <p class="hint">Select an instrument in the market watch to see details.</p>
    </div>
  {:else}
    <div class="snap-body">
      <div>
        <div class="snap-price">
          <span class="big mono {trend}">{quote.ltp?.text ?? "—"}</span>
          <span class="mono {trend}">{quote.change?.text ?? "—"}</span>
          <span class="mono {trend}">{quote.pct ?? ""}</span>
        </div>
        <div class="muted mono" style="margin-top: 4px">{title}</div>
        <div class="dim mono" style="font-size: 11px">{quote.key}</div>
      </div>

      <dl class="kv">
        <dt>Open</dt>
        <dd>{quote.open?.text ?? "—"}</dd>
        <dt>High</dt>
        <dd>{quote.high?.text ?? "—"}</dd>
        <dt>Low</dt>
        <dd>{quote.low?.text ?? "—"}</dd>
        <dt>Prev close</dt>
        <dd>{quote.prev_close?.text ?? "—"}</dd>
        <dt>Best bid</dt>
        <dd>{quote.bid?.text ?? "—"}</dd>
        <dt>Best ask</dt>
        <dd>{quote.ask?.text ?? "—"}</dd>
        <dt>Volume</dt>
        <dd>{formatVolume(quote.volume)}</dd>
      </dl>

      {#if instrument}
        <dl class="kv">
          <dt>Exchange</dt>
          <dd>{instrument.key.split("|")[0]}</dd>
          <dt>Token</dt>
          <dd>{instrument.key.split("|")[1]}</dd>
          <dt>Type</dt>
          <dd>{instrument.instrument_type || "—"}</dd>
          <dt>Lot size</dt>
          <dd>{instrument.lot_size}</dd>
          <dt>Expiry</dt>
          <dd>{instrument.expiry ?? "—"}</dd>
          <dt>Strike</dt>
          <dd>{instrument.strike} {instrument.strike_type !== "XX" ? instrument.strike_type : ""}</dd>
          <dt>Underlying</dt>
          <dd>{instrument.underlying ?? "—"}</dd>
        </dl>
      {/if}

      <p class="hint">
        Order placement is not wired yet: it needs the REST client and OAuth session (Phase 2b).
        This panel is read-only.
      </p>
    </div>
  {/if}
</aside>
