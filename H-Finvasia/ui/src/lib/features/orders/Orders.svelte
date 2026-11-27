<script lang="ts">
  import { onMount } from "svelte";
  import { api, events, type OrderDto } from "../../api";
  import EmptyState from "../../components/EmptyState.svelte";
  import { isClosedOrder } from "./orderStatus";

  interface Props {
    embedded?: boolean;
  }
  let { embedded = false }: Props = $props();

  let orders = $state<OrderDto[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let tab = $state<"pending" | "closed">("pending");

  async function refresh() {
    loading = true;
    error = null;
    try {
      orders = await api.ordersLoad();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    void refresh();
    let disposed = false;
    let unlisten: (() => void) | null = null;
    void events.onOrdersChanged(() => {
      if (!disposed) void refresh();
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      if (unlisten) unlisten();
    };
  });

  const pending = $derived(orders.filter((o) => !isClosedOrder(o.status)));
  const closed = $derived(orders.filter((o) => isClosedOrder(o.status)));
  const shown = $derived(tab === "pending" ? pending : closed);
</script>

<section class="panel">
  <header class="panel-head">
    <h2>Order book</h2>
    <span class="muted">read-only</span>
    <span class="spacer"></span>
    <button disabled={loading} onclick={refresh}>{loading ? "Loading?" : "Refresh"}</button>
  </header>

  <div class="chain-toolbar" style="padding: 8px 14px">
    <button class:primary={tab === "pending"} onclick={() => (tab = "pending")}>
      Pending ({pending.length})
    </button>
    <button class:primary={tab === "closed"} onclick={() => (tab = "closed")}>
      Closed ({closed.length})
    </button>
  </div>

  {#if error}
    <div class="banner"><span>{error}</span></div>
  {/if}

  {#if shown.length === 0}
    <EmptyState
      title={tab === "pending" ? "No pending orders" : "No closed orders"}
      hint="Orders placed through the broker appear here and refresh on websocket order updates."
    />
  {:else}
    <div class="grid-wrap">
      <table class="grid">
        <thead>
          <tr>
            <th class="l">Order</th>
            <th class="l">Exch</th>
            <th class="l">Symbol</th>
            <th>Side</th>
            <th class="l">Type</th>
            <th>Qty</th>
            <th>Filled</th>
            <th>Pending</th>
            <th>Price</th>
            <th>Avg</th>
            <th class="l">Status</th>
            <th class="l">Rejection</th>
          </tr>
        </thead>
        <tbody>
          {#each shown as order (order.order_no + order.status)}
            <tr>
              <td class="l mono dim">{order.order_no}</td>
              <td class="l dim">{order.exchange}</td>
              <td class="l mono">{order.symbol}</td>
              <td class="num mono {order.side === "B" ? "up" : "down"}">{order.side}</td>
              <td class="l dim">{order.price_type}</td>
              <td class="num mono">{order.quantity}</td>
              <td class="num mono">{order.filled}</td>
              <td class="num mono">{order.pending}</td>
              <td class="num mono">{order.price ?? "-"}</td>
              <td class="num mono">{order.average_price ?? "-"}</td>
              <td class="l {order.status === "REJECTED" ? "down" : ""}">{order.status}</td>
              <td class="l dim" title={order.rejection ?? ""}>
                {order.rejection ? order.rejection.slice(0, 60) : "-"}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}

  {#if !embedded}
    <p class="hint" style="padding: 10px 14px; border-top: 1px solid var(--border)">
      Modify, cancel and exit are intentionally absent: they are write operations and need the order
      path plus your explicit go-ahead.
    </p>
  {/if}
</section>
