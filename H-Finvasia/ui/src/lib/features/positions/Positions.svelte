<script lang="ts">
  import { onMount } from "svelte";
  import { api, type PositionDto, type PositionsView } from "../../api";
  import EmptyState from "../../components/EmptyState.svelte";
  import ContextMenu from "../../components/ContextMenu.svelte";

  interface Props {
    embedded?: boolean;
  }
  let { embedded = false }: Props = $props();

  let view = $state<PositionsView | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let busy = $state(false);
  let menu = $state<{ x: number; y: number; position: PositionDto } | null>(null);

  async function refresh() {
    loading = true;
    error = null;
    try {
      view = await api.positionsLoad();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function squareOff(position: PositionDto) {
    if (busy) return;
    if (position.net_qty === 0) return;
    const side = position.net_qty > 0 ? "S" : "B";
    if (
      !window.confirm(
        `Square off ${Math.abs(position.net_qty)} ${position.symbol} with a protected market order?`,
      )
    ) {
      return;
    }
    busy = true;
    error = null;
    notice = null;
    try {
      const result = await api.squareOff({
        key: `${position.exchange}|${position.token}`,
        side,
        quantity: Math.abs(position.net_qty),
        product: position.product || position.product_alias,
      });
      notice = `Square-off ${result.status}: ${result.order_no}`;
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    void refresh();
  });

  const funds = $derived(view?.funds ?? null);
</script>

<section class="panel">
  <header class="panel-head">
    <h2>Net position book</h2>
    <span class="muted">live - right-click to square off</span>
    <span class="spacer"></span>
    <button disabled={loading} onclick={refresh}>{loading ? "Loading?" : "Refresh"}</button>
  </header>

  {#if error}
    <div class="banner"><span>{error}</span></div>
  {/if}
  {#if notice}
    <div class="banner ok-banner">
      <span>{notice}</span>
      <button class="ghost" onclick={() => (notice = null)}>Dismiss</button>
    </div>
  {/if}

  {#if funds}
    <div class="spot-strip">
      <span class="spot-item">Cash <strong>{funds.cash ?? "-"}</strong></span>
      <span class="spot-item">Blocked <strong>{funds.blocked ?? "-"}</strong></span>
      <span class="spot-item">Peak margin <strong>{funds.peak_margin ?? "-"}</strong></span>
      <span class="spot-item">Turnover <strong>{funds.turnover ?? "-"}</strong></span>
      <span class="spot-item">Pending value <strong>{funds.pending_order_value ?? "-"}</strong></span>
      <span class="spot-item">Brokerage <strong>{funds.brokerage ?? "-"}</strong></span>
    </div>
  {/if}

  {#if view && view.positions.length === 0}
    <EmptyState title="No open positions" hint="Positions appear here once an order is executed." />
  {:else if view}
    <div class="grid-wrap">
      <table class="grid">
        <thead>
          <tr>
            <th class="l">Exch</th>
            <th class="l">Symbol</th>
            <th class="l">Product</th>
            <th class="l">Type</th>
            <th>Lot</th>
            <th>NetQty</th>
            <th>Avg</th>
            <th>LTP</th>
            <th>MToM</th>
            <th>P&amp;L</th>
            <th>BuyVal</th>
            <th>SellVal</th>
          </tr>
        </thead>
        <tbody>
          {#each view.positions as position (position.exchange + position.symbol + position.product)}
            <tr
              title="Right-click to square off"
              oncontextmenu={(event) => {
                event.preventDefault();
                event.stopPropagation();
                menu = { x: event.clientX, y: event.clientY, position };
              }}
            >
              <td class="l dim">{position.exchange}</td>
              <td class="l mono">{position.symbol}</td>
              <td class="l">{position.product_alias || position.product}</td>
              <td class="l dim">{position.instrument}</td>
              <td class="num mono dim">{position.lot_size}</td>
              <td class="num mono {position.net_qty >= 0 ? "up" : "down"}">{position.net_qty}</td>
              <td class="num mono">{position.net_avg ?? "-"}</td>
              <td class="num mono">{position.ltp ?? "-"}</td>
              <td class="num mono {Number(position.mtm ?? 0) >= 0 ? "up" : "down"}">{position.mtm ?? "-"}</td>
              <td class="num mono {Number(position.pnl ?? 0) >= 0 ? "up" : "down"}">{position.pnl ?? "-"}</td>
              <td class="num mono dim">{position.buy_value ?? "-"}</td>
              <td class="num mono dim">{position.sell_value ?? "-"}</td>
            </tr>
          {/each}
        </tbody>
        <tfoot>
          <tr>
            <td class="l" colspan="5">Totals</td>
            <td class="num mono">-</td>
            <td class="num mono">-</td>
            <td class="num mono">-</td>
            <td class="num mono {Number(view.totals.mtm) >= 0 ? "up" : "down"}">{view.totals.mtm}</td>
            <td class="num mono">-</td>
            <td class="num mono">{view.totals.buy_value}</td>
            <td class="num mono">{view.totals.sell_value}</td>
          </tr>
        </tfoot>
      </table>
    </div>
  {/if}

  {#if !embedded}
    <p class="hint" style="padding: 10px 14px; border-top: 1px solid var(--border)">
      Right-click a live position to square it off with a protected market order. Product conversion
      and margin updates are not wired yet.
    </p>
  {/if}

  {#if menu}
    <ContextMenu
      x={menu.x}
      y={menu.y}
      items={[
        {
          label:
            menu.position.net_qty === 0
              ? "Position already flat"
              : `Square off ${Math.abs(menu.position.net_qty)} ${menu.position.symbol} (protected market)`,
          disabled: busy || menu.position.net_qty === 0,
          action: () => {
            if (menu) void squareOff(menu.position);
          },
        },
      ]}
      onclose={() => (menu = null)}
    />
  {/if}
</section>
