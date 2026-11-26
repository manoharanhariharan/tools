<script lang="ts">
  import { untrack } from "svelte";
  import { api, type OrderDefaults, type OrderResult } from "../../api";
  import Modal from "../../components/Modal.svelte";
  import type { OrderTicketState } from "../../order";

  interface Props {
    ticket: OrderTicketState;
    defaults: OrderDefaults;
    live: boolean;
    onclose: () => void;
    onsubmitted: (result: OrderResult) => void;
  }
  let { ticket, defaults, live, onclose, onsubmitted }: Props = $props();

  let lots = $state(untrack(() => defaults.lots));
  let product = $state(untrack(() => defaults.product));
  let priceType = $state(untrack(() => defaults.price_type));
  let validity = $state(untrack(() => defaults.validity));
  let price = $state(untrack(() => ticket.price));
  let busy = $state(false);
  let error = $state<string | null>(null);

  const lotSize = $derived(ticket.target.lot_size > 0 ? ticket.target.lot_size : 1);
  const qty = $derived(Math.max(1, Math.round(lots)) * lotSize);
  const sideLabel = $derived(ticket.side === "B" ? "BUY" : "SELL");
  const valid = $derived(live && lots >= 1 && qty > 0 && price.trim() !== "");
  const subtitle = $derived(
    ticket.target.expiry
      ? `${ticket.target.exchange} - ${ticket.target.expiry} - ${ticket.target.strike_type} ${ticket.target.strike}`
      : `${ticket.target.exchange} - ${
          ticket.target.instrument_type || (ticket.target.is_index ? "INDEX" : "EQ")
        }`,
  );

  async function submit() {
    if (!valid || busy) return;
    busy = true;
    error = null;
    try {
      const result = await api.placeOrder({
        key: ticket.target.key,
        side: ticket.side,
        lots: Math.max(1, Math.round(lots)),
        product,
        price_type: priceType,
        price: price.trim(),
        validity,
      });
      onsubmitted(result);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Modal title={`${sideLabel} ${ticket.target.trading_symbol}`} {subtitle} onclose={onclose} wide>
  {#if !live}
    <div class="banner">
      <span>Live feed required. This is a replay/offline session; confirmation is disabled.</span>
    </div>
  {/if}
  {#if error}
    <div class="banner"><span>{error}</span></div>
  {/if}

  <div class="ticket-side {ticket.side === 'B' ? 'buy' : 'sell'}">
    <strong>{sideLabel}</strong>
    <span>{ticket.target.trading_symbol}</span>
  </div>

  <div class="form-grid">
    <div class="field">
      <label for="ticket-lots">Quantity (lots)</label>
      <input id="ticket-lots" type="number" min="1" max="10000" bind:value={lots} />
    </div>
    <div class="field">
      <label for="ticket-qty">Order quantity</label>
      <input id="ticket-qty" value={qty} disabled />
    </div>
    <div class="field">
      <label for="ticket-product">Product</label>
      <select id="ticket-product" bind:value={product}>
        <option value="I">I - Intraday</option>
        <option value="C">C - Delivery</option>
        <option value="M">M - Margin</option>
      </select>
    </div>
    <div class="field">
      <label for="ticket-price-type">Price type</label>
      <select id="ticket-price-type" bind:value={priceType}>
        <option value="LMT">LMT - Limit</option>
      </select>
    </div>
    <div class="field">
      <label for="ticket-price">Limit price</label>
      <input id="ticket-price" bind:value={price} />
    </div>
    <div class="field">
      <label for="ticket-validity">Validity</label>
      <select id="ticket-validity" bind:value={validity}>
        <option value="DAY">DAY</option>
        <option value="IOC">IOC</option>
      </select>
    </div>
  </div>

  <p class="hint">
    Lot size {lotSize}. Shoonya accepts LMT only; MKT orders are rejected. This sends a live order
    when you press Confirm. Check the symbol, side and quantity first.
  </p>

  <div class="modal-actions">
    <button class="ghost" disabled={busy} onclick={onclose}>Cancel</button>
    <button
      class:primary={ticket.side === "B"}
      class:down={ticket.side === "S"}
      disabled={busy || !valid}
      onclick={submit}
    >
      {busy ? "Sending..." : `Confirm ${sideLabel} ${qty}`}
    </button>
  </div>
</Modal>
