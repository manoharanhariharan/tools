//! Data-transfer objects handed to the UI.
//!
//! Money is never sent as `f64`: every price travels as
//! * a formatted display string (`"1287.30"`), and
//! * an exact `i64` in 1e-6 units for client-side sorting.
//!
//! That keeps the "no float money" rule intact even across the IPC boundary.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Trend {
    Up,
    Down,
    Flat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PriceDto {
    /// Display text with the instrument's precision.
    pub text: String,
    /// Exact value in 1e-6 units, for sorting only.
    pub e6: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuoteDto {
    pub key: String,
    pub trading_symbol: Option<String>,

    pub ltp: Option<PriceDto>,
    pub change: Option<PriceDto>,
    pub pct: Option<String>,
    pub open: Option<PriceDto>,
    pub high: Option<PriceDto>,
    pub low: Option<PriceDto>,
    pub prev_close: Option<PriceDto>,

    pub volume: Option<i64>,
    pub bid: Option<PriceDto>,
    pub ask: Option<PriceDto>,
    pub bid_qty: Option<i64>,
    pub ask_qty: Option<i64>,

    pub trend: Trend,
    pub seq: u64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstrumentDto {
    pub key: String,
    pub trading_symbol: String,
    pub name: Option<String>,
    pub underlying: Option<String>,
    pub expiry: Option<String>,
    pub strike: String,
    pub strike_type: String,
    pub lot_size: i64,
    pub instrument_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeltaDto {
    /// Monotonic batch counter; the UI uses it to detect gaps.
    pub seq: u64,
    pub timestamp_ms: i64,
    pub quotes: Vec<QuoteDto>,
    /// Instruments still queued after this batch.
    pub pending: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanDto {
    pub subscribe: Vec<String>,
    pub unsubscribe: Vec<String>,
    pub rejected: Vec<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChainRowDto {
    pub key: String,
    pub exchange: String,
    pub symbol: String,
    pub expiry: Option<String>,
    pub strike: String,
    pub strike_type: String,
    pub lot_size: i64,
    /// Live/merged quote; `None` until the first tick for this leg.
    pub quote: Option<QuoteDto>,
    /// Whether this leg is currently subscribed (and therefore live).
    pub live: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Idle,
    Replay,
    Live,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StatusDto {
    pub source: SourceKind,
    pub connected: bool,
    /// `"mongo"` or `"in-memory"`.
    pub backend: String,
    pub master_loaded: bool,
    pub instruments: usize,
    pub watched: usize,
    pub watch_limit: usize,
    /// Unique instruments subscribed across every group (watch + chain).
    pub subscribed: usize,
    /// Hard ceiling on [`subscribed`].
    pub subscription_limit: usize,
    pub frames_total: u64,
    pub frames_error: u64,
    pub frames_per_sec: u64,
    pub last_error: Option<String>,
    pub build: String,
}
