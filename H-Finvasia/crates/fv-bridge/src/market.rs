//! UI-facing market state.
//!
//! Deliberately has **no Tauri dependency** so the whole thing is unit-testable
//! without a webview. `src-tauri` is a thin adapter over this type.
//!
//! Responsibilities:
//! * own the merged quote book and the watch set;
//! * absorb frames and count errors instead of panicking;
//! * emit bounded, deterministic delta batches for the UI;
//! * answer snapshot/search/status queries.

use fv_core::{Price, Quote, SymbolKey};
use fv_engine::{Coalescer, DeltaBatch};
use fv_feed::{Applied, Frame, QuoteBook, WatchSet};
use fv_master::{Instrument, MemoryMaster};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Instant;

use crate::dto::{
    DeltaDto, InstrumentDto, PlanDto, PriceDto, QuoteDto, SourceKind, StatusDto, Trend,
};

/// Default watch budget. The venue publishes no scrip cap; this is a UI-side
/// guard so a runaway option chain cannot allocate without bound.
pub const DEFAULT_WATCH_LIMIT: usize = 500;

/// Frames per coalesced UI batch.
pub const DEFAULT_MAX_PER_FRAME: usize = 600;

struct RateMeter {
    count: u64,
    last_rate: u64,
    window_start: Instant,
}

impl RateMeter {
    fn new(now: Instant) -> Self {
        RateMeter {
            count: 0,
            last_rate: 0,
            window_start: now,
        }
    }

    fn record(&mut self) {
        self.count = self.count.saturating_add(1);
    }

    fn per_sec(&mut self, now: Instant) -> u64 {
        let elapsed = now
            .saturating_duration_since(self.window_start)
            .as_secs_f64();
        if elapsed >= 1.0 {
            self.last_rate = (self.count as f64 / elapsed).round() as u64;
            self.count = 0;
            self.window_start = now;
        }
        self.last_rate
    }
}

pub struct MarketData {
    book: QuoteBook,
    coalescer: Coalescer,
    watch: WatchSet,
    master: Option<Arc<MemoryMaster>>,
    source: SourceKind,
    connected: bool,
    backend: &'static str,
    frames_total: u64,
    frames_error: u64,
    last_error: Option<String>,
    rate: RateMeter,
    build: String,
}

impl MarketData {
    pub fn new(build: impl Into<String>) -> Self {
        let now = Instant::now();
        MarketData {
            book: QuoteBook::new(),
            coalescer: Coalescer::new(30, DEFAULT_MAX_PER_FRAME),
            watch: WatchSet::new(DEFAULT_WATCH_LIMIT),
            master: None,
            source: SourceKind::Idle,
            connected: false,
            backend: "in-memory",
            frames_total: 0,
            frames_error: 0,
            last_error: None,
            rate: RateMeter::new(now),
            build: build.into(),
        }
    }

    pub fn with_watch_limit(mut self, limit: usize) -> Self {
        self.watch = WatchSet::new(limit);
        self
    }

    pub fn set_master(&mut self, master: Arc<MemoryMaster>) {
        self.master = Some(master);
    }

    /// Publish the backend that actually served the master, so the UI can show
    /// "mongo" vs the degraded "in-memory" state.
    pub fn set_backend(&mut self, backend: impl Into<String>) {
        let text = backend.into();
        self.backend = if text == "mongo" {
            "mongo"
        } else {
            "in-memory"
        };
    }

    pub fn set_source(&mut self, source: SourceKind) {
        self.source = source;
        self.connected = source != SourceKind::Idle;
    }

    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.last_error = Some(message.into());
    }

    pub fn clear_error(&mut self) {
        self.last_error = None;
    }

    pub fn master_loaded(&self) -> bool {
        self.master.is_some()
    }

    /// Absorb one decoded frame.
    pub fn apply(&mut self, frame: &Frame, now_ms: i64) -> Applied {
        self.frames_total = self.frames_total.saturating_add(1);
        self.rate.record();
        let applied = self.book.apply(frame, now_ms);
        if let Applied::Snapshot(key) | Applied::Update(key) = &applied {
            self.coalescer.mark(key.clone());
        }
        applied
    }

    /// Absorb one raw frame, counting decode failures instead of panicking.
    pub fn apply_raw(&mut self, raw: &[u8], now_ms: i64) -> Result<Applied, fv_feed::FeedError> {
        match fv_feed::parse(raw) {
            Ok(frame) => Ok(self.apply(&frame, now_ms)),
            Err(err) => {
                self.frames_error = self.frames_error.saturating_add(1);
                self.last_error = Some(err.to_string());
                Err(err)
            }
        }
    }

    pub fn record_error(&mut self, message: impl Into<String>) {
        self.frames_error = self.frames_error.saturating_add(1);
        self.last_error = Some(message.into());
    }

    /// Emit a bounded delta batch if the coalescing interval has elapsed.
    pub fn flush(&mut self, now: Instant, now_ms: i64) -> Option<DeltaDto> {
        self.coalescer
            .flush_due(now, now_ms, &self.book)
            .map(|batch| self.to_delta(batch))
    }

    /// Emit regardless of interval (window focus, watch changes).
    pub fn flush_now(&mut self, now: Instant, now_ms: i64) -> Option<DeltaDto> {
        self.coalescer
            .flush_now(now, now_ms, &self.book)
            .map(|batch| self.to_delta(batch))
    }

    fn to_delta(&self, batch: DeltaBatch) -> DeltaDto {
        DeltaDto {
            seq: batch.seq,
            timestamp_ms: batch.timestamp_ms,
            quotes: batch.quotes.iter().map(quote_dto).collect(),
            pending: batch.pending,
        }
    }

    pub fn snapshot(&self, wire: &str) -> Option<QuoteDto> {
        let key = SymbolKey::parse(wire).ok()?;
        self.book.get(&key).map(quote_dto)
    }

    /// Current quotes for everything in the watch list, in stable order. Used to
    /// hydrate the grid immediately after a watch change.
    pub fn watch_quotes(&self) -> Vec<QuoteDto> {
        self.watch
            .active()
            .iter()
            .filter_map(|k| self.book.get(k))
            .map(quote_dto)
            .collect()
    }

    pub fn watched(&self) -> Vec<String> {
        self.watch.active().iter().map(|k| k.as_wire()).collect()
    }

    /// Replace the watch set. Emits the minimal subscribe/unsubscribe plan,
    /// reports overflow, and drops quotes for instruments no longer watched so
    /// memory tracks the watch list instead of the whole session.
    pub fn set_watch(&mut self, keys: Vec<SymbolKey>) -> PlanDto {
        let target: BTreeSet<SymbolKey> = keys.into_iter().collect();
        let plan = self.watch.reconcile(&target);
        self.book.retain_keys(self.watch.active());
        PlanDto {
            subscribe: plan.subscribe.iter().map(|k| k.as_wire()).collect(),
            unsubscribe: plan.unsubscribe.iter().map(|k| k.as_wire()).collect(),
            rejected: plan.rejected.iter().map(|k| k.as_wire()).collect(),
            truncated: plan.truncated,
        }
    }

    /// Add one instrument to the watch list, returning the resulting plan.
    pub fn add_to_watch(&mut self, key: SymbolKey) -> PlanDto {
        let mut target: BTreeSet<SymbolKey> = self.watch.active().clone();
        target.insert(key);
        self.set_watch(target.into_iter().collect())
    }

    pub fn remove_from_watch(&mut self, wire: &str) -> PlanDto {
        let mut target: BTreeSet<SymbolKey> = self.watch.active().clone();
        if let Ok(key) = SymbolKey::parse(wire) {
            target.remove(&key);
        }
        self.set_watch(target.into_iter().collect())
    }

    pub fn status(&mut self, now: Instant) -> StatusDto {
        StatusDto {
            source: self.source,
            connected: self.connected,
            backend: self.backend.to_string(),
            master_loaded: self.master.is_some(),
            instruments: self.master.as_ref().map(|m| m.len()).unwrap_or(0),
            watched: self.watch.len(),
            watch_limit: self.watch.max(),
            frames_total: self.frames_total,
            frames_error: self.frames_error,
            frames_per_sec: self.rate.per_sec(now),
            last_error: self.last_error.clone(),
            build: self.build.clone(),
        }
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<InstrumentDto> {
        match self.master.as_ref() {
            Some(master) => master
                .search(query, limit)
                .into_iter()
                .map(|(instrument, _)| instrument_dto(instrument))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Instrument metadata for a symbol already in the master (used by Snap Quote).
    pub fn instrument(&self, wire: &str) -> Option<InstrumentDto> {
        let key = SymbolKey::parse(wire).ok()?;
        self.master
            .as_ref()
            .and_then(|m| m.get(&key))
            .map(instrument_dto)
    }
}

fn price_dto(price: Price, decimals: u8) -> PriceDto {
    PriceDto {
        text: price.format(decimals),
        e6: price.scaled(),
    }
}

fn decimals_for(quote: &Quote) -> u8 {
    quote.price_precision.unwrap_or(2).min(6)
}

fn quote_dto(quote: &Quote) -> QuoteDto {
    let d = decimals_for(quote);
    let change = quote.change();
    let trend = match change {
        Some(c) if c.scaled() > 0 => Trend::Up,
        Some(c) if c.scaled() < 0 => Trend::Down,
        Some(_) => Trend::Flat,
        None => Trend::Flat,
    };
    QuoteDto {
        key: quote.key.as_wire(),
        trading_symbol: quote.trading_symbol.clone(),
        ltp: quote.ltp.map(|p| price_dto(p, d)),
        change: change.map(|p| price_dto(p, d)),
        pct: quote.pct_change.map(|p| format!("{}%", p.format(2))),
        open: quote.open.map(|p| price_dto(p, d)),
        high: quote.high.map(|p| price_dto(p, d)),
        low: quote.low.map(|p| price_dto(p, d)),
        prev_close: quote.prev_close.map(|p| price_dto(p, d)),
        volume: quote.volume,
        bid: quote.best_bid.map(|p| price_dto(p, d)),
        ask: quote.best_ask.map(|p| price_dto(p, d)),
        trend,
        seq: quote.seq,
        updated_at_ms: quote.updated_at_ms,
    }
}

fn instrument_dto(instrument: &Instrument) -> InstrumentDto {
    InstrumentDto {
        key: instrument.key.as_wire(),
        trading_symbol: instrument.trading_symbol.clone(),
        name: instrument.name.clone(),
        underlying: instrument.underlying.clone(),
        expiry: instrument.expiry.clone(),
        strike: instrument.strike.format(2),
        strike_type: instrument.strike_type.as_str().to_string(),
        lot_size: instrument.lot_size,
        instrument_type: instrument.instrument_type.clone(),
    }
}

/// Compile-time-ish build label for the status bar.
pub fn build_label(version: &str, date: &str) -> String {
    format!("{version} ({date})")
}
