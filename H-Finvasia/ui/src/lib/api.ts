import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Trend = "up" | "down" | "flat";
export type SourceKind = "idle" | "replay" | "live";

export interface PriceDto {
  text: string;
  e6: number;
}

export interface QuoteDto {
  key: string;
  trading_symbol: string | null;
  ltp: PriceDto | null;
  change: PriceDto | null;
  pct: string | null;
  open: PriceDto | null;
  high: PriceDto | null;
  low: PriceDto | null;
  prev_close: PriceDto | null;
  volume: number | null;
  oi: number | null;
  prev_oi: number | null;
  bid: PriceDto | null;
  ask: PriceDto | null;
  bid_qty: number | null;
  ask_qty: number | null;
  trend: Trend;
  seq: number;
  updated_at_ms: number;
  /** True when this key is an index (drives the spot-rate strip). */
  is_index: boolean;
  underlying: string | null;
  has_fno: boolean;
}

export interface InstrumentDto {
  key: string;
  trading_symbol: string;
  name: string | null;
  underlying: string | null;
  expiry: string | null;
  strike: string;
  strike_type: string;
  lot_size: number;
  instrument_type: string;
  has_fno: boolean;
  is_index: boolean;
}

export interface FundsDto {
  profile: string | null;
  cash: string | null;
  payin: string | null;
  payout: string | null;
  uncleared: string | null;
  blocked: string | null;
  turnover: string | null;
  turnover_limit: string | null;
  pending_order_value: string | null;
  brokerage: string | null;
  peak_margin: string | null;
  premium: string | null;
}

export interface PositionDto {
  exchange: string;
  symbol: string;
  token: string;
  product: string;
  product_alias: string;
  instrument: string;
  lot_size: number;
  net_qty: number;
  net_avg: string | null;
  ltp: string | null;
  mtm: string | null;
  pnl: string | null;
  day_buy_qty: number;
  day_sell_qty: number;
  buy_value: string | null;
  sell_value: string | null;
}

export interface PositionTotals {
  buy_qty: number;
  sell_qty: number;
  buy_value: string;
  sell_value: string;
  net_value: string;
  mtm: string;
  rpnl: string;
}

export interface PositionsView {
  funds: FundsDto | null;
  positions: PositionDto[];
  totals: PositionTotals;
}

export interface AccountSummaryDto {
  session: SessionInfo;
  funds: FundsDto | null;
  totals: PositionTotals;
  positions: number;
  connected: boolean;
}

export interface OrderDto {
  order_no: string;
  status: string;
  side: string;
  price_type: string;
  retention: string;
  quantity: number;
  price: string | null;
  average_price: string | null;
  filled: number;
  pending: number;
  rejection: string | null;
  rejected_by: string | null;
  exchange: string;
  symbol: string;
  exchange_order_id: string | null;
  remarks: string | null;
  order_time: string | null;
}

export interface GroupSummary {
  name: string;
  count: number;
  color: string | null;
}

export interface IndexRowDto {
  key: string;
  symbol: string;
  name: string | null;
  exchange: string;
  live: boolean;
  underlying: string | null;
  has_fno: boolean;
  quote: QuoteDto | null;
}

export interface ChainRowDto {
  key: string;
  exchange: string;
  symbol: string;
  expiry: string | null;
  strike: string;
  strike_type: string;
  lot_size: number;
  quote: QuoteDto | null;
  live: boolean;
}

export interface StockRowDto {
  key: string;
  symbol: string;
  name: string | null;
  exchange: string;
  live: boolean;
  underlying: string | null;
  has_fno: boolean;
  quote: QuoteDto | null;
}

export interface ContainersView {
  active: string | null;
  groups: GroupSummary[];
}

/** One instrument inside a group card; may be a stock, option, future or index. */
export interface ContainerRowDto {
  key: string;
  symbol: string;
  name: string | null;
  exchange: string;
  instrument_type: string;
  underlying: string | null;
  expiry: string | null;
  strike: string;
  strike_type: string;
  lot_size: number;
  is_index: boolean;
  live: boolean;
  quote: QuoteDto | null;
}

export interface ContainerCardDto {
  name: string;
  color: string | null;
  /** Stored member count, which can exceed `rows.length` before the master loads. */
  count: number;
  rows: ContainerRowDto[];
  /** Manual instrument order for the Custom sort; may be empty. */
  custom_order: string[];
}

export interface OrderTargetDto {
  key: string;
  trading_symbol: string;
  exchange: string;
  expiry: string | null;
  strike: string;
  strike_type: string;
  lot_size: number;
  instrument_type: string;
  is_index: boolean;
  tradeable: boolean;
  underlying: string | null;
  has_fno: boolean;
  quote: QuoteDto | null;
}

export interface OrderDefaults {
  product: string;
  price_type: string;
  validity: string;
  lots: number;
}

export interface PlaceOrderRequest {
  key: string;
  side: string;
  lots: number;
  product: string;
  price_type: string;
  price: string | null;
  trigger_price?: string | null;
  trail?: string | null;
  validity: string;
}

export interface OrderResult {
  order_no: string;
  status: string;
}

export interface MarginDto {
  required: string;
  after_trade: string | null;
}

export interface SquareOffRequest {
  key: string;
  side: string;
  quantity: number;
  product: string;
  /** Last position LTP, used if the socket has not ticked this key yet. */
  price: string | null;
}

export interface DeltaDto {
  seq: number;
  timestamp_ms: number;
  quotes: QuoteDto[];
  pending: number;
}

export interface PlanDto {
  subscribe: string[];
  unsubscribe: string[];
  rejected: string[];
  truncated: boolean;
}

export interface StatusDto {
  source: SourceKind;
  connected: boolean;
  backend: string;
  master_loaded: boolean;
  instruments: number;
  watched: number;
  watch_limit: number;
  subscribed: number;
  subscription_limit: number;
  frames_total: number;
  frames_error: number;
  frames_per_sec: number;
  last_error: string | null;
  build: string;
}

export interface WatchChanged {
  keys: string[];
  quotes: QuoteDto[];
}

export interface AppInfo {
  version: string;
  backend: string;
  has_token: boolean;
}

export interface SessionInfo {
  uid: string;
  actid: string;
  source: string;
  has_credentials: boolean;
  has_stored_token: boolean;
  stored_date: string | null;
}

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  status: () => invoke<StatusDto>("status"),
  search: (query: string, limit = 25) => invoke<InstrumentDto[]>("search", { query, limit }),
  instrument: (key: string) => invoke<InstrumentDto | null>("instrument", { key }),
  watchList: () => invoke<string[]>("watch_list"),
  watchQuotes: () => invoke<QuoteDto[]>("watch_quotes"),
  snapshot: (key: string) => invoke<QuoteDto | null>("snapshot", { key }),
  watchAdd: (key: string) => invoke<PlanDto>("watch_add", { key }),
  watchRemove: (key: string) => invoke<PlanDto>("watch_remove", { key }),
  removeEverywhere: (key: string) => invoke<PlanDto>("remove_everywhere", { key }),
  startReplay: (hz = 120) => invoke<string[]>("start_replay", { path: null, hz }),
  startReplayFile: (path: string, hz = 120) =>
    invoke<string[]>("start_replay", { path, hz }),
  startLive: () => invoke<string[]>("start_live"),
  sessionInfo: () => invoke<SessionInfo>("session_info"),
  chainUnderlyings: (query = "", limit = 40) =>
    invoke<string[]>("chain_underlyings", { query, limit }),
  chainExpiries: (underlying: string) => invoke<string[]>("chain_expiries", { underlying }),
  chainLoad: (
    chainId: string,
    underlying: string,
    expiry: string,
    strikes: number,
    atm: string | null,
  ) =>
    invoke<ChainRowDto[]>("chain_load", {
      chainId,
      underlying,
      expiry,
      strikes,
      atm,
    }),
  chainClose: (chainId: string) => invoke<void>("chain_close", { chainId }),
  positionsLoad: () => invoke<PositionsView>("positions_load"),
  accountSummary: () => invoke<AccountSummaryDto>("account_summary"),
  ordersLoad: () => invoke<OrderDto[]>("orders_load"),
  groupsList: () => invoke<GroupSummary[]>("groups_list"),
  groupSave: (name: string) => invoke<void>("group_save", { name }),
  groupLoad: (name: string) => invoke<PlanDto>("group_load", { name }),
  groupDelete: (name: string) => invoke<void>("group_delete", { name }),
  indicesList: () => invoke<IndexRowDto[]>("indices_list"),
  stocksList: () => invoke<StockRowDto[]>("stocks_list"),
  stocksWatched: () => invoke<StockRowDto[]>("stocks_watched"),
  stocksSearch: (query: string, limit = 20) =>
    invoke<StockRowDto[]>("stocks_search", { query, limit }),
  containersList: () => invoke<ContainersView>("containers_list"),
  containerCreate: (name: string) => invoke<ContainersView>("container_create", { name }),
  containerCreateEmpty: (name: string) =>
    invoke<ContainersView>("container_create_empty", { name }),
  containerActivate: (name: string) => invoke<PlanDto>("container_activate", { name }),
  containerRename: (oldName: string, newName: string) =>
    invoke<ContainersView>("container_rename", { old: oldName, new: newName }),
  containerDelete: (name: string) => invoke<ContainersView>("container_delete", { name }),
  containerColor: (name: string, color: string) =>
    invoke<ContainersView>("container_color", { name, color }),
  containerCards: () => invoke<ContainerCardDto[]>("container_cards"),
  containerAdd: (name: string, key: string) =>
    invoke<ContainersView>("container_add", { name, key }),
  containerRemove: (name: string, key: string) =>
    invoke<ContainersView>("container_remove", { name, key }),
  containerReorder: (order: string[]) =>
    invoke<ContainersView>("container_reorder", { order }),
  containerReorderKeys: (name: string, keys: string[]) =>
    invoke<ContainersView>("container_reorder_keys", { name, keys }),
  indexContainersList: () => invoke<ContainersView>("index_containers_list"),
  indexContainerCreateEmpty: (name: string) =>
    invoke<ContainersView>("index_container_create_empty", { name }),
  indexContainerActivate: (name: string) => invoke<PlanDto>("index_container_activate", { name }),
  indexContainerRename: (oldName: string, newName: string) =>
    invoke<ContainersView>("index_container_rename", { old: oldName, new: newName }),
  indexContainerDelete: (name: string) =>
    invoke<ContainersView>("index_container_delete", { name }),
  indexContainerColor: (name: string, color: string) =>
    invoke<ContainersView>("index_container_color", { name, color }),
  indexContainerCards: () => invoke<ContainerCardDto[]>("index_container_cards"),
  indexContainerAdd: (name: string, key: string) =>
    invoke<ContainersView>("index_container_add", { name, key }),
  indexContainerRemove: (name: string, key: string) =>
    invoke<ContainersView>("index_container_remove", { name, key }),
  indexContainerReorder: (order: string[]) =>
    invoke<ContainersView>("index_container_reorder", { order }),
  indexContainerReorderKeys: (name: string, keys: string[]) =>
    invoke<ContainersView>("index_container_reorder_keys", { name, keys }),
  orderTarget: (key: string) => invoke<OrderTargetDto | null>("order_target", { key }),
  orderDefaultsGet: () => invoke<OrderDefaults>("order_defaults_get"),
  orderDefaultsSet: (defaults: OrderDefaults) =>
    invoke<OrderDefaults>("order_defaults_set", { defaults }),
  marginRequired: (request: PlaceOrderRequest) =>
    invoke<MarginDto>("margin_required", { request }),
  placeOrder: (request: PlaceOrderRequest) =>
    invoke<OrderResult>("place_order", { request }),
  squareOff: (request: SquareOffRequest) =>
    invoke<OrderResult>("square_off", { request }),
  stopFeed: () => invoke<void>("stop_feed"),
};

/**
 * Event subscriptions. Every call returns its own unlisten function so callers
 * can release listeners on teardown (no leaked handlers across navigation).
 */
export const events = {
  onDelta: (handler: (delta: DeltaDto) => void) =>
    listen<DeltaDto>("quotes:delta", (event) => handler(event.payload)),
  onStatus: (handler: (status: StatusDto) => void) =>
    listen<StatusDto>("status:changed", (event) => handler(event.payload)),
  onWatch: (handler: (payload: WatchChanged) => void) =>
    listen<WatchChanged>("watch:changed", (event) => handler(event.payload)),
  onFeed: (handler: (state: string) => void) =>
    listen<string>("feed:changed", (event) => handler(event.payload)),
  onFeedError: (handler: (message: string) => void) =>
    listen<string>("feed:error", (event) => handler(event.payload)),
  onMasterReady: (handler: () => void) =>
    listen<null>("master:ready", () => handler()),
  onOrdersChanged: (handler: (orderNo: string) => void) =>
    listen<string>("orders:changed", (event) => handler(event.payload)),
};
