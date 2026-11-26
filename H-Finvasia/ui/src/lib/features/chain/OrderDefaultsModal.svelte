<script lang="ts">
  import { untrack } from "svelte";
  import { api, type OrderDefaults } from "../../api";
  import Modal from "../../components/Modal.svelte";

  interface Props {
    defaults: OrderDefaults;
    onclose: () => void;
    onsaved: (defaults: OrderDefaults) => void;
  }
  let { defaults, onclose, onsaved }: Props = $props();

  let lots = $state(untrack(() => defaults.lots));
  let product = $state(untrack(() => defaults.product));
  let priceType = $state(untrack(() => defaults.price_type));
  let validity = $state(untrack(() => defaults.validity));
  let busy = $state(false);
  let error = $state<string | null>(null);

  async function save() {
    busy = true;
    error = null;
    try {
      const saved = await api.orderDefaultsSet({
        lots: Math.max(1, Math.round(lots)),
        product,
        price_type: priceType,
        validity,
      });
      onsaved(saved);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Modal
  title="Order defaults"
  subtitle="Applied to every new ticket. The ticket still needs an explicit confirm before anything is sent."
  onclose={onclose}
>
  {#if error}
    <div class="banner"><span>{error}</span></div>
  {/if}

  <div class="form-grid">
    <div class="field">
      <label for="default-lots">Quantity (lots)</label>
      <input id="default-lots" type="number" min="1" max="10000" bind:value={lots} />
    </div>
    <div class="field">
      <label for="default-product">Product</label>
      <select id="default-product" bind:value={product}>
        <option value="I">I - Intraday</option>
        <option value="C">C - Delivery</option>
        <option value="M">M - Margin</option>
      </select>
    </div>
    <div class="field">
      <label for="default-price-type">Price type</label>
      <select id="default-price-type" bind:value={priceType}>
        <option value="LMT">LMT - Limit</option>
        <option value="MKT">MKT - Market (protected)</option>
        <option value="SL-L">SL-L - Stop loss limit</option>
        <option value="TSL">TSL - Trailing stop</option>
      </select>
    </div>
    <div class="field">
      <label for="default-validity">Validity</label>
      <select id="default-validity" bind:value={validity}>
        <option value="DAY">DAY</option>
        <option value="IOC">IOC</option>
      </select>
    </div>
  </div>

  <div class="modal-actions">
    <button class="ghost" disabled={busy} onclick={onclose}>Cancel</button>
    <button class="primary" disabled={busy} onclick={save}>{busy ? "Saving..." : "Save defaults"}</button>
  </div>
</Modal>
