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
  bid: PriceDto | null;
  ask: PriceDto | null;
  trend: Trend;
  seq: number;
  updated_at_ms: number;
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
  startReplay: (hz = 120) => invoke<string[]>("start_replay", { path: null, hz }),
  startReplayFile: (path: string, hz = 120) =>
    invoke<string[]>("start_replay", { path, hz }),
  startLive: (uid: string, actid: string) => invoke<string[]>("start_live", { uid, actid }),
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
  onMasterReady: (handler: () => void) =>
    listen<null>("master:ready", () => handler()),
};
