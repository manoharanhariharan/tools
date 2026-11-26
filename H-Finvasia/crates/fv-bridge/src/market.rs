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
    ChainRowDto, DeltaDto, InstrumentDto, PlanDto, PriceDto, QuoteDto, SourceKind, StatusDto, Trend,
};

/// Default watch budget.
///
/// Deliberately conservative: the operator asked that we touch the broker with no
/// more than 100 instruments at a time. The venue publishes no cap, so this is a
/// self-imposed politeness limit, not a technical one.
pub const DEFAULT_WATCH_LIMIT: usize = 100;

/// Frames per coalesced UI batch.
pub const DEFAULT_MAX_PER_FRAME: usize = 600;

/// Maximum option-chain legs subscribed at once.
pub const DEFAULT_CHAIN_LIMIT: usize = 100;

/// Hard ceiling on simultaneous subscriptions across **every** group
/// (watch + option chain). Exceeding it is refused, not truncated silently.
pub const MAX_TOTAL_SUBSCRIPTIONS: usize = 100;

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
    /// Option-chain subscriptions, tracked separately from the watch list.
    chain: BTreeSet<SymbolKey>,
    chain_limit: usize,
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
            chain: BTreeSet::new(),
            chain_limit: DEFAULT_CHAIN_LIMIT,
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

    pub fn master(&self) -> Option<&MemoryMaster> {
        self.master.as_deref()
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

    /// The union of every subscribed instrument: the watch list plus the option
    /// chain. Both groups are subscribed, but only the watch list is the user's
    /// portfolio-style list.
    fn desired_keys(&self) -> BTreeSet<SymbolKey> {
        self.watch
            .active()
            .union(&self.chain)
            .cloned()
            .collect::<BTreeSet<_>>()
    }

    /// Diff the union before/after a group change so a key still needed by the
    /// other group is never unsubscribed.
    fn reconcile(&mut self, before: BTreeSet<SymbolKey>, rejected: Vec<SymbolKey>) -> PlanDto {
        let after = self.desired_keys();
        let subscribe: Vec<SymbolKey> = after.difference(&before).cloned().collect();
        let unsubscribe: Vec<SymbolKey> = before.difference(&after).cloned().collect();
        self.book.retain_keys(&after);
        PlanDto {
            subscribe: subscribe.iter().map(|k| k.as_wire()).collect(),
            unsubscribe: unsubscribe.iter().map(|k| k.as_wire()).collect(),
            rejected: rejected.iter().map(|k| k.as_wire()).collect(),
            truncated: !rejected.is_empty(),
        }
    }

    /// Replace the watch set. Emits the minimal subscribe/unsubscribe plan and
    /// drops quotes for instruments that are no longer needed by any group.
    pub fn set_watch(&mut self, keys: Vec<SymbolKey>) -> PlanDto {
        let before = self.desired_keys();
        let target: BTreeSet<SymbolKey> = keys.into_iter().collect();
        let (_accepted, mut rejected) = self.watch.replace(&target);
        // Growing the watch list can push the chain over the global budget.
        rejected.extend(self.clamp_chain());
        self.reconcile(before, rejected)
    }

    /// Trim the chain group so the union stays within
    /// [`MAX_TOTAL_SUBSCRIPTIONS`]. Returns the dropped keys.
    fn clamp_chain(&mut self) -> Vec<SymbolKey> {
        let available = MAX_TOTAL_SUBSCRIPTIONS.saturating_sub(self.watch.len());
        if self.chain.len() <= available {
            return Vec::new();
        }
        let keep: BTreeSet<SymbolKey> = self.chain.iter().take(available).cloned().collect();
        let dropped: Vec<SymbolKey> = self.chain.difference(&keep).cloned().collect();
        self.chain = keep;
        dropped
    }

    /// Replace the option-chain subscription group.
    ///
    /// Kept separate from the watch list because a single chain can exceed the
    /// watch budget, and legs should not appear in the user's portfolio list.
    pub fn set_chain(&mut self, keys: Vec<SymbolKey>) -> PlanDto {
        let before = self.desired_keys();
        let target: BTreeSet<SymbolKey> = keys.into_iter().collect();
        let available = self
            .chain_limit
            .min(MAX_TOTAL_SUBSCRIPTIONS.saturating_sub(self.watch.len()));
        let accepted: BTreeSet<SymbolKey> = target.iter().take(available).cloned().collect();
        let rejected: Vec<SymbolKey> = target.iter().skip(available).cloned().collect();
        self.chain = accepted;
        if !rejected.is_empty() {
            tracing::warn!(
                requested = target.len(),
                accepted = available,
                dropped = rejected.len(),
                "option chain request clamped to the subscription budget"
            );
        }
        self.reconcile(before, rejected)
    }

    /// Drop every chain subscription (leaving the watch list untouched).
    pub fn clear_chain(&mut self) -> PlanDto {
        let before = self.desired_keys();
        self.chain.clear();
        self.reconcile(before, Vec::new())
    }

    pub fn chain_len(&self) -> usize {
        self.chain.len()
    }

    /// Total unique instruments currently subscribed (watch ∪ chain).
    pub fn subscribed_len(&self) -> usize {
        self.desired_keys().len()
    }

    /// One option-chain row: static instrument metadata plus the live quote.
    pub fn chain_row(&self, instrument: &Instrument) -> ChainRowDto {
        ChainRowDto {
            key: instrument.key.as_wire(),
            exchange: instrument.key.exchange().to_string(),
            symbol: instrument.trading_symbol.clone(),
            expiry: instrument.expiry.clone(),
            strike: instrument.strike.format(2),
            strike_type: instrument.strike_type.as_str().to_string(),
            lot_size: instrument.lot_size,
            quote: self.book.get(&instrument.key).map(quote_dto),
            live: self.chain.contains(&instrument.key),
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
            subscribed: self.subscribed_len(),
            subscription_limit: MAX_TOTAL_SUBSCRIPTIONS,
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

/// Pick a narrow window of strikes around `atm`.
///
/// A full expiry can be hundreds of legs; subscribing all of them is both
/// unnecessary and impolite to the venue. This returns the keys for at most
/// `strikes_each_side * 2 + 1` strikes (CE and PE), centred on the strike
/// nearest `atm` (or the middle of the chain when no ATM is known).
pub fn chain_window(
    legs: &[Instrument],
    strikes_each_side: usize,
    atm: Option<Price>,
) -> Vec<SymbolKey> {
    if legs.is_empty() {
        return Vec::new();
    }

    let mut strikes: Vec<Price> = legs.iter().map(|leg| leg.strike).collect();
    strikes.sort();
    strikes.dedup();
    if strikes.is_empty() {
        return Vec::new();
    }

    let centre = match atm {
        Some(target) => strikes
            .iter()
            .enumerate()
            .min_by_key(|(_, strike)| {
                let a = strike.scaled() as i128;
                let b = target.scaled() as i128;
                (a - b).unsigned_abs()
            })
            .map(|(index, _)| index)
            .unwrap_or(strikes.len() / 2),
        None => strikes.len() / 2,
    };

    let start = centre.saturating_sub(strikes_each_side);
    let end = (centre + strikes_each_side).min(strikes.len() - 1);
    let selected: std::collections::BTreeSet<Price> =
        strikes[start..=end].iter().copied().collect();

    legs.iter()
        .filter(|leg| selected.contains(&leg.strike))
        .map(|leg| leg.key.clone())
        .collect()
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
        bid_qty: quote.best_bid_qty,
        ask_qty: quote.best_ask_qty,
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
