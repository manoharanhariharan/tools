<script lang="ts">
  import { onMount } from "svelte";
  import { api, events, type OrderDto } from "../../api";
  import EmptyState from "../../components/EmptyState.svelte";
  import ContextMenu from "../../components/ContextMenu.svelte";
  import { columnResize } from "../../table/resize";
  import {
    loadColumnLayout,
    saveColumnLayout,
    type ColumnMeta,
  } from "../../table/columns";
  import { isClosedOrder } from "./orderStatus";

  const COLUMN_KEY = "fv.orders.columns";
  const COLUMNS: ColumnMeta[] = [
    { id: "order", label: "Order", width: 96, min: 70 },
    { id: "exch", label: "Exch", width: 48, min: 38 },
    { id: "symbol", label: "Symbol", width: 152, min: 90 },
    { id: "side", label: "Side", width: 46, min: 38 },
    { id: "type", label: "Type", width: 56, min: 44 },
    { id: "qty", label: "Qty", width: 52, min: 40 },
    { id: "filled", label: "Filled", width: 56, min: 44 },
    { id: "pending", label: "Pending", width: 62, min: 48 },
    { id: "price", label: "Price", width: 66, min: 50 },
    { id: "avg", label: "Avg", width: 66, min: 50 },
    { id: "status", label: "Status", width: 80, min: 58 },
    { id: "rejection", label: "Rejection", width: 176, min: 80 },
  ];
  const LEFT_COLUMNS = new Set(["order", "exch", "symbol", "type", "status", "rejection"]);

  interface Props {
    embedded?: boolean;
  }
  let { embedded = false }: Props = $props();

  let orders = $state<OrderDto[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let tab = $state<"pending" | "closed">("pending");
  let columnLayout = $state(loadColumnLayout(COLUMN_KEY, COLUMNS));
  let columnMenu = $state<{ x: number; y: number } | null>(null);

  const visibleColumns = $derived(
    COLUMNS.filter((column) => !columnLayout.hidden.includes(column.id)),
  );
  const totalWidth = $derived(
    visibleColumns.reduce(
      (total, column) => total + (columnLayout.widths[column.id] ?? column.width),
      0,
    ),
  );

  function columnWidth(id: string): number {
    return columnLayout.widths[id] ?? COLUMNS.find((column) => column.id === id)?.width ?? 80;
  }

  function setColumnWidth(id: string, width: number) {
    columnLayout = {
      ...columnLayout,
      widths: { ...columnLayout.widths, [id]: Math.round(width) },
    };
  }

  function persistColumns() {
    saveColumnLayout(COLUMN_KEY, columnLayout);
  }

  function toggleColumn(id: string) {
    const hidden = columnLayout.hidden.includes(id)
      ? columnLayout.hidden.filter((item) => item !== id)
      : [...columnLayout.hidden, id];
    if (hidden.length >= COLUMNS.length) return;
    columnLayout = { ...columnLayout, hidden };
    persistColumns();
  }

  function resetColumns() {
    columnLayout = {
      widths: Object.fromEntries(COLUMNS.map((column) => [column.id, column.width])),
      hidden: [],
    };
    persistColumns();
  }

  function orderCellClass(id: string, order: OrderDto): string {
    switch (id) {
      case "order":
        return "l mono dim";
      case "exch":
        return "l dim";
      case "symbol":
        return "l mono";
      case "side":
        return `num mono ${order.side === "B" ? "up" : "down"}`;
      case "type":
        return "l dim";
      case "status":
        return `l ${order.status === "REJECTED" ? "down" : ""}`;
      case "rejection":
        return "l dim";
      default:
        return "num mono";
    }
  }

  function orderCellText(id: string, order: OrderDto): string | number {
    switch (id) {
      case "order":
        return order.order_no;
      case "exch":
        return order.exchange;
      case "symbol":
        return order.symbol;
      case "side":
        return order.side;
      case "type":
        return order.price_type;
      case "qty":
        return order.quantity;
      case "filled":
        return order.filled;
      case "pending":
        return order.pending;
      case "price":
        return order.price ?? "-";
      case "avg":
        return order.average_price ?? "-";
      case "status":
        return order.status;
      case "rejection":
        return order.rejection ? order.rejection.slice(0, 60) : "-";
      default:
        return "-";
    }
  }

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
      <table class="grid column-table" style="width: {totalWidth}px">
        <colgroup>
          {#each visibleColumns as column (column.id)}
            <col style="width: {columnWidth(column.id)}px" />
          {/each}
        </colgroup>
        <thead>
          <tr>
            {#each visibleColumns as column (column.id)}
              <th
                class={LEFT_COLUMNS.has(column.id) ? "l" : ""}
                title="Drag the edge to resize; right-click to show or hide columns"
                oncontextmenu={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  columnMenu = { x: event.clientX, y: event.clientY };
                }}
              >
                {column.label}
                <span
                  class="col-resize"
                  use:columnResize={{
                    width: columnWidth(column.id),
                    min: column.min,
                    max: column.max,
                    onresize: (width) => setColumnWidth(column.id, width),
                    onend: persistColumns,
                  }}
                ></span>
              </th>
            {/each}
          </tr>
        </thead>
        <tbody>
          {#each shown as order (order.order_no + order.status)}
            <tr>
              {#each visibleColumns as column (column.id)}
                <td
                  class={orderCellClass(column.id, order)}
                  title={column.id === "rejection" ? (order.rejection ?? "") : undefined}
                >
                  {orderCellText(column.id, order)}
                </td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}

  {#if columnMenu}
    <ContextMenu
      x={columnMenu.x}
      y={columnMenu.y}
      items={[
        ...COLUMNS.map((column) => ({
          label: `${columnLayout.hidden.includes(column.id) ? "○" : "●"} ${column.label}`,
          action: () => toggleColumn(column.id),
        })),
        { label: "Reset columns", action: resetColumns },
      ]}
      onclose={() => (columnMenu = null)}
    />
  {/if}

  {#if !embedded}
    <p class="hint" style="padding: 10px 14px; border-top: 1px solid var(--border)">
      Modify, cancel and exit are intentionally absent: they are write operations and need the order
      path plus your explicit go-ahead.
    </p>
  {/if}
</section>
