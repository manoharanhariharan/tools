<script lang="ts">
  import { untrack } from "svelte";
  import {
    api,
    type MarginDto,
    type OrderDefaults,
    type OrderResult,
    type PlaceOrderRequest,
  } from "../../api";
  import { formatIndianAmount } from "../../format";
  import Modal from "../../components/Modal.svelte";
  import { buildOrderRequest, type OrderTicketState } from "../../order";

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
  let trigger = $state(
    untrack(() =>
      defaults.price_type === "SL-L" || defaults.price_type === "TSL" ? ticket.price : "",
    ),
  );
  let trail = $state("0.05");
  let busy = $state(false);
  let error = $state<string | null>(null);

  let margin = $state<MarginDto | null>(null);
  let marginLoading = $state(false);
  let marginError = $state<string | null>(null);

  const lotSize = $derived(ticket.target.lot_size > 0 ? ticket.target.lot_size : 1);
  const qty = $derived(Math.max(1, Math.round(lots)) * lotSize);
  const sideLabel = $derived(ticket.side === "B" ? "BUY" : "SELL");
  const needsTrigger = $derived(priceType === "SL-L" || priceType === "TSL");
  const needsTrail = $derived(priceType === "TSL");
  const valid = $derived(
    live &&
      lots >= 1 &&
      qty > 0 &&
      price.trim() !== "" &&
      (!needsTrigger || trigger.trim() !== "") &&
      (!needsTrail || Number(trail) > 0),
  );
  const subtitle = $derived(
    ticket.target.expiry
      ? `${ticket.target.exchange} - ${ticket.target.expiry} - ${ticket.target.strike_type} ${ticket.target.strike}`
      : `${ticket.target.exchange} - ${
          ticket.target.instrument_type || (ticket.target.is_index ? "INDEX" : "EQ")
        }`,
  );

  function orderRequest(): PlaceOrderRequest {
    return buildOrderRequest({
      key: ticket.target.key,
      side: ticket.side,
      lots,
      product,
      priceType,
      price,
      trigger,
      trail,
      validity,
    });
  }

  function onPriceTypeChange() {
    if (!needsTrigger) return;
    if (needsTrail) {
      if (!(Number(trail) > 0)) trail = "0.05";
      const reference = Number(price);
      const distance = Number(trail);
      if (Number.isFinite(reference) && reference > 0 && Number.isFinite(distance)) {
        const next = ticket.side === "S" ? reference - distance : reference + distance;
        trigger = Math.max(0.05, next).toFixed(2);
        price = trigger;
      }
    } else if (trigger.trim() === "") {
      trigger = price;
    }
  }

  untrack(() => {
    if (priceType === "TSL") onPriceTypeChange();
  });

  async function submit() {
    if (!valid || busy) return;
    busy = true;
    error = null;
    try {
      const result = await api.placeOrder(orderRequest());
      onsubmitted(result);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  let marginSeq = 0;

  // Margin is a broker call, so debounce edits and never run it offline.
  $effect(() => {
    const request = orderRequest();
    const ready =
      live &&
      request.price !== null &&
      (!needsTrigger || (request.trigger_price ?? "") !== "") &&
      (!needsTrail || Number(request.trail ?? 0) > 0);
    if (!ready) {
      margin = null;
      marginLoading = false;
      marginError = null;
      return;
    }
    marginLoading = true;
    marginError = null;
    const seq = ++marginSeq;
    const timer = setTimeout(async () => {
      try {
        const value = await api.marginRequired(request);
        if (seq === marginSeq) margin = value;
      } catch (e) {
        if (seq === marginSeq) {
          margin = null;
          marginError = String(e);
        }
      } finally {
        if (seq === marginSeq) marginLoading = false;
      }
    }, 350);
    return () => clearTimeout(timer);
  });
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
      <select
        id="ticket-price-type"
        bind:value={priceType}
        onchange={onPriceTypeChange}
      >
        <option value="LMT">LMT - Limit</option>
        <option value="MKT">MKT - Market (protected)</option>
        <option value="SL-L">SL-L - Stop loss limit</option>
        <option value="TSL">TSL - Trailing stop</option>
      </select>
    </div>
    <div class="field">
      <label for="ticket-price">
        {priceType === "MKT" ? "Market reference (LTP at load)" : "Limit price"}
      </label>
      <input id="ticket-price" bind:value={price} />
      <span class="field-hint">LTP when this ticket opened: {ticket.price || "-"}</span>
    </div>
    {#if needsTrigger}
      <div class="field">
        <label for="ticket-trigger">Trigger price</label>
        <input id="ticket-trigger" bind:value={trigger} />
      </div>
    {/if}
    {#if needsTrail}
      <div class="field">
        <label for="ticket-trail">Trail by</label>
        <input id="ticket-trail" type="number" min="0.05" step="0.05" bind:value={trail} />
      </div>
    {/if}
    <div class="field">
      <label for="ticket-validity">Validity</label>
      <select id="ticket-validity" bind:value={validity}>
        <option value="DAY">DAY</option>
        <option value="IOC">IOC</option>
      </select>
    </div>
  </div>

  {#if live}
    <div class="ticket-margin">
      <span class="ticket-margin-label">Margin required</span>
      {#if marginLoading}
        <span class="muted">calculating...</span>
      {:else if margin}
        <strong class="mono">₹{formatIndianAmount(margin.required)}</strong>
        {#if margin.after_trade && margin.after_trade !== margin.required}
          <span class="dim">after trade ₹{formatIndianAmount(margin.after_trade)}</span>
        {/if}
      {:else}
        <span class="dim">{marginError ?? "Margin unavailable"}</span>
      {/if}
    </div>
  {/if}

  <p class="hint">
    Lot size {lotSize}. MKT is sent as a protected marketable limit because Shoonya rejects raw
    market orders on the API. TSL places an SL-L and trails its trigger from the live LTP while the
    app is running. This sends a live order when you press Confirm.
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
