<script lang="ts">
  import { onMount } from "svelte";
  import { api, events, type PositionDto, type PositionsView, type QuoteDto } from "../../api";
  import EmptyState from "../../components/EmptyState.svelte";
  import ContextMenu from "../../components/ContextMenu.svelte";
  import { livePosition, liveTotals, positionKey } from "./live";
  import { columnResize } from "../../table/resize";
  import {
    loadColumnLayout,
    saveColumnLayout,
    type ColumnMeta,
  } from "../../table/columns";

  const COLUMN_KEY = "fv.positions.columns";
  const COLUMNS: ColumnMeta[] = [
    { id: "exch", label: "Exch", width: 48, min: 38 },
    { id: "symbol", label: "Symbol", width: 152, min: 90 },
    { id: "product", label: "Product", width: 76, min: 56 },
    { id: "type", label: "Type", width: 72, min: 54 },
    { id: "lot", label: "Lot", width: 48, min: 38 },
    { id: "netqty", label: "NetQty", width: 66, min: 48 },
    { id: "avg", label: "Avg", width: 70, min: 52 },
    { id: "ltp", label: "LTP", width: 70, min: 52 },
    { id: "mtm", label: "MToM", width: 76, min: 56 },
    { id: "pnl", label: "P&L", width: 76, min: 56 },
    { id: "buyval", label: "BuyVal", width: 86, min: 62 },
    { id: "sellval", label: "SellVal", width: 86, min: 62 },
  ];

  interface Props {
    embedded?: boolean;
    onnotify?: (message: string, tone: "ok" | "error" | "warn") => void;
  }
  let { embedded = false, onnotify }: Props = $props();

  let view = $state<PositionsView | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let busy = $state(false);
  let menu = $state<{ x: number; y: number; position: PositionDto } | null>(null);
  let liveQuotes = $state<Map<string, QuoteDto>>(new Map());
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

  const LEFT_COLUMNS = new Set(["exch", "symbol", "product", "type"]);

  function headerClass(id: string): string {
    return LEFT_COLUMNS.has(id) ? "l" : "";
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

  function positionCellClass(id: string, position: PositionDto): string {
    switch (id) {
      case "exch":
        return "l dim";
      case "symbol":
        return "l mono";
      case "product":
        return "l";
      case "type":
        return "l dim";
      case "lot":
        return "num mono dim";
      case "netqty":
        return `num mono ${position.net_qty >= 0 ? "up" : "down"}`;
      case "mtm":
      case "pnl":
        return `num mono ${Number(position[id] ?? 0) >= 0 ? "up" : "down"}`;
      case "buyval":
      case "sellval":
        return "num mono dim";
      default:
        return "num mono";
    }
  }

  function positionCellText(id: string, position: PositionDto): string | number {
    switch (id) {
      case "exch":
        return position.exchange;
      case "symbol":
        return position.symbol;
      case "product":
        return position.product_alias || position.product;
      case "type":
        return position.instrument;
      case "lot":
        return position.lot_size;
      case "netqty":
        return position.net_qty;
      case "avg":
        return position.net_avg ?? "-";
      case "ltp":
        return position.ltp ?? "-";
      case "mtm":
        return position.mtm ?? "-";
      case "pnl":
        return position.pnl ?? "-";
      case "buyval":
        return position.buy_value ?? "-";
      case "sellval":
        return position.sell_value ?? "-";
      default:
        return "-";
    }
  }

  function positionFooterClass(id: string, first: boolean): string {
    if (first) return "l";
    if (id === "mtm") return `num mono ${Number(totals.mtm) >= 0 ? "up" : "down"}`;
    if (id === "pnl") return `num mono ${Number(totals.pnl) >= 0 ? "up" : "down"}`;
    return "num mono";
  }

  function positionFooterText(id: string, first: boolean): string {
    if (first) return "Totals";
    switch (id) {
      case "mtm":
        return totals.mtm;
      case "pnl":
        return totals.pnl;
      case "buyval":
        return view?.totals.buy_value ?? "-";
      case "sellval":
        return view?.totals.sell_value ?? "-";
      default:
        return "-";
    }
  }

  const displayPositions = $derived(
    (view?.positions ?? []).map((position) =>
      livePosition(position, liveQuotes.get(positionKey(position))),
    ),
  );
  const totals = $derived(liveTotals(view?.positions ?? [], liveQuotes));
  const positionKeys = $derived(new Set((view?.positions ?? []).map(positionKey)));

  async function refresh() {
    loading = true;
    error = null;
    try {
      const next = await api.positionsLoad();
      view = next;
      const keep = new Set(next.positions.map(positionKey));
      if ([...liveQuotes.keys()].some((key) => !keep.has(key))) {
        liveQuotes = new Map([...liveQuotes].filter(([key]) => keep.has(key)));
      }
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
        price: position.ltp,
      });
      notice = `Square-off ${result.status}: ${result.order_no}`;
      onnotify?.(`Square-off ${result.status}: ${result.order_no}`, "ok");
      await refresh();
    } catch (e) {
      error = String(e);
      onnotify?.(`Square-off failed: ${String(e)}`, "error");
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    void refresh();
    let disposed = false;
    const unlisteners: Array<() => void> = [];

    (async () => {
      try {
        const onDelta = await events.onDelta((delta) => {
          if (disposed) return;
          let changed = false;
          const next = new Map(liveQuotes);
          for (const quote of delta.quotes) {
            if (positionKeys.has(quote.key)) {
              next.set(quote.key, quote);
              changed = true;
            }
          }
          if (changed) liveQuotes = next;
        });
        if (disposed) onDelta();
        else unlisteners.push(onDelta);

        const onOrders = await events.onOrdersChanged(() => {
          if (!disposed) void refresh();
        });
        if (disposed) onOrders();
        else unlisteners.push(onOrders);
      } catch (e) {
        if (!disposed) error = String(e);
      }
    })();

    return () => {
      disposed = true;
      for (const unlisten of unlisteners) unlisten();
    };
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
                class={headerClass(column.id)}
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
          {#each displayPositions as position (position.exchange + position.symbol + position.product)}
            <tr
              title="Right-click to square off"
              oncontextmenu={(event) => {
                event.preventDefault();
                event.stopPropagation();
                menu = { x: event.clientX, y: event.clientY, position };
              }}
            >
              {#each visibleColumns as column (column.id)}
                <td class={positionCellClass(column.id, position)}>
                  {positionCellText(column.id, position)}
                </td>
              {/each}
            </tr>
          {/each}
        </tbody>
        <tfoot>
          <tr>
            {#each visibleColumns as column, positionIndex (column.id)}
              <td class={positionFooterClass(column.id, positionIndex === 0)}>
                {positionFooterText(column.id, positionIndex === 0)}
              </td>
            {/each}
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
