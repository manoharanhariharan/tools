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
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

use crate::dto::{
    ChainRowDto, ContainerCardDto, ContainerRowDto, DeltaDto, IndexRowDto, InstrumentDto, PlanDto,
    PriceDto, QuoteDto, SourceKind, StatusDto, StockRowDto, Trend,
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
    /// Every instrument stored in any Stocks group. Subscribed in addition to
    /// the active watch list so all group cards stream, not just the active one.
    groups: BTreeSet<SymbolKey>,
    /// Instruments in the broker position book, subscribed so live LTP can
    /// recompute MTM/P&L without another REST round-trip.
    positions: BTreeSet<SymbolKey>,
    /// Option-chain subscriptions keyed by chain tab id, tracked separately
    /// from the watch list. Multiple pinned chains can coexist.
    chains: BTreeMap<String, BTreeSet<SymbolKey>>,
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
            groups: BTreeSet::new(),
            positions: BTreeSet::new(),
            chains: BTreeMap::new(),
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
            quotes: batch
                .quotes
                .iter()
                .map(|q| quote_dto(q, self.master.as_deref()))
                .collect(),
            pending: batch.pending,
        }
    }

    pub fn snapshot(&self, wire: &str) -> Option<QuoteDto> {
        let key = SymbolKey::parse(wire).ok()?;
        self.book
            .get(&key)
            .map(|q| quote_dto(q, self.master.as_deref()))
    }

    /// Current quotes for everything in the watch list, in stable order. Used to
    /// hydrate the grid immediately after a watch change.
    pub fn watch_quotes(&self) -> Vec<QuoteDto> {
        let master = self.master.as_deref();
        self.watch
            .active()
            .iter()
            .filter_map(|key| match self.book.get(key) {
                Some(quote) => Some(quote_dto(quote, master)),
                // No tick yet: still return the instrument metadata so the row
                // knows its underlying, index flag and F&O availability.
                None => master
                    .and_then(|m| m.get(key))
                    .map(|instrument| placeholder_dto(instrument, master)),
            })
            .collect()
    }

    /// Index rows for Index Watch: local metadata plus a live quote where
    /// subscribed. Browsing costs no broker interaction.
    pub fn index_rows(&self, limit: usize) -> Vec<IndexRowDto> {
        let Some(master) = self.master.as_deref() else {
            return Vec::new();
        };
        master
            .indices()
            .into_iter()
            .take(limit)
            .map(|instrument| IndexRowDto {
                key: instrument.key.as_wire(),
                symbol: instrument.trading_symbol.clone(),
                name: instrument.name.clone(),
                exchange: instrument.key.exchange().to_string(),
                live: self.is_live(&instrument.key),
                underlying: instrument.underlying.clone(),
                has_fno: instrument
                    .underlying
                    .as_deref()
                    .map(|u| master.has_derivatives(u))
                    .unwrap_or(false),
                quote: self
                    .book
                    .get(&instrument.key)
                    .map(|q| quote_dto(q, self.master.as_deref())),
            })
            .collect()
    }

    pub fn watched(&self) -> Vec<String> {
        self.watch.active().iter().map(|k| k.as_wire()).collect()
    }

    /// The NSE cash-equity universe for the Stocks tab. Metadata is local; only
    /// watched symbols have a quote, so opening the tab never touches the venue.
    pub fn stock_rows(&self) -> Vec<StockRowDto> {
        let Some(master) = self.master.as_deref() else {
            return Vec::new();
        };
        master
            .cash_equities()
            .into_iter()
            .map(|instrument| self.stock_row(instrument, master))
            .collect()
    }

    /// Stocks currently in the active container (i.e. the watch list). This is
    /// what the Stocks tab shows by default; the rest of the universe is only
    /// reached through search.
    pub fn watched_stock_rows(&self) -> Vec<StockRowDto> {
        let Some(master) = self.master.as_deref() else {
            return Vec::new();
        };
        let mut rows: Vec<StockRowDto> = self
            .watch
            .active()
            .iter()
            .filter_map(|key| master.get(key))
            .filter(|instrument| fv_master::is_cash_equity(instrument))
            .map(|instrument| self.stock_row(instrument, master))
            .collect();
        rows.sort_by(|a, b| a.symbol.cmp(&b.symbol).then_with(|| a.key.cmp(&b.key)));
        rows
    }

    /// Search restricted to NSE cash equities. Used by the Stocks tab flyout so
    /// an option or index cannot be added from this screen by accident.
    pub fn search_stocks(&self, query: &str, limit: usize) -> Vec<StockRowDto> {
        let Some(master) = self.master.as_deref() else {
            return Vec::new();
        };
        if query.trim().is_empty() {
            return Vec::new();
        }
        let limit = limit.clamp(1, 50);
        master
            .search(query, (limit * 20).clamp(limit, 500))
            .into_iter()
            .filter(|(instrument, _)| fv_master::is_cash_equity(instrument))
            .take(limit)
            .map(|(instrument, _)| self.stock_row(instrument, master))
            .collect()
    }

    /// Resolve one stored group key into the richer card row shape. Returns
    /// `None` when the master has not loaded that instrument yet.
    pub fn container_row(&self, wire: &str) -> Option<ContainerRowDto> {
        let key = SymbolKey::parse(wire).ok()?;
        let master = self.master.as_deref()?;
        let instrument = master.get(&key)?;
        let cash_equity = fv_master::is_cash_equity(instrument);
        let symbol = if cash_equity {
            fv_master::stock_display_symbol(instrument)
        } else {
            instrument.trading_symbol.trim().to_string()
        };
        Some(ContainerRowDto {
            key: wire.to_string(),
            symbol,
            name: instrument.name.clone(),
            exchange: key.exchange().to_string(),
            // The FINVASIA_SYMBOL shape often omits instrumentType for cash
            // equities; the master's own classifier is the authority there.
            instrument_type: if cash_equity
                || (instrument.instrument_type.trim().is_empty()
                    && !fv_master::is_index(instrument)
                    && !instrument.is_option()
                    && !instrument.is_future())
            {
                "EQ".to_string()
            } else {
                instrument.instrument_type.clone()
            },
            underlying: instrument.underlying.clone(),
            expiry: instrument.expiry.clone(),
            strike: instrument.strike.format(2),
            strike_type: instrument.strike_type.as_str().to_string(),
            lot_size: instrument.lot_size,
            is_index: fv_master::is_index(instrument),
            live: self.is_live(&key),
            quote: self.book.get(&key).map(|q| quote_dto(q, Some(master))),
        })
    }

    /// Underlyings that have derivatives, filtered by a typed prefix/substring.
    /// Used by the option-chain picker; it never touches the broker.
    pub fn chain_underlyings(&self, query: &str, limit: usize) -> Vec<String> {
        let Some(master) = self.master.as_deref() else {
            return Vec::new();
        };
        let needle = query.trim().to_ascii_uppercase();
        master
            .derivative_underlyings()
            .into_iter()
            .filter(|name| needle.is_empty() || name.contains(&needle))
            .take(limit.clamp(1, 500))
            .collect()
    }

    /// Build one group card. `keys` are the stored wire keys in group order.
    pub fn container_card(
        &self,
        name: &str,
        color: Option<String>,
        keys: &[String],
        custom_order: Vec<String>,
    ) -> ContainerCardDto {
        ContainerCardDto {
            name: name.to_string(),
            color,
            count: keys.len(),
            rows: keys
                .iter()
                .filter_map(|key| self.container_row(key))
                .collect(),
            custom_order,
        }
    }

    fn stock_row(&self, instrument: &Instrument, master: &MemoryMaster) -> StockRowDto {
        StockRowDto {
            key: instrument.key.as_wire(),
            symbol: fv_master::stock_display_symbol(instrument),
            name: instrument.name.clone(),
            exchange: instrument.key.exchange().to_string(),
            live: self.is_live(&instrument.key),
            underlying: instrument.underlying.clone(),
            has_fno: instrument
                .underlying
                .as_deref()
                .map(|u| master.has_derivatives(u))
                .unwrap_or(false),
            quote: self
                .book
                .get(&instrument.key)
                .map(|q| quote_dto(q, Some(master))),
        }
    }

    /// The union of every subscribed instrument: the active watch list, every
    /// Stocks group member and the option chain. Only the active watch list is
    /// the user's portfolio-style list (`stocks_watched`); group members exist
    /// so all cards have live quotes.
    fn desired_keys(&self) -> BTreeSet<SymbolKey> {
        self.watch
            .active()
            .union(&self.groups)
            .chain(self.positions.iter())
            .chain(self.chains.values().flat_map(|group| group.iter()))
            .cloned()
            .collect::<BTreeSet<_>>()
    }

    fn is_live(&self, key: &SymbolKey) -> bool {
        self.watch.active().contains(key)
            || self.groups.contains(key)
            || self.positions.contains(key)
            || self.chains.values().any(|group| group.contains(key))
    }

    /// Subscribe the instruments currently held in the broker position book.
    /// Their live LTP drives MTM/P&L in the UI without re-polling the broker.
    pub fn set_position_keys(&mut self, keys: Vec<SymbolKey>) -> PlanDto {
        let before = self.desired_keys();
        let chain_keys: BTreeSet<SymbolKey> = self
            .chains
            .values()
            .flat_map(|group| group.iter().cloned())
            .collect();
        let base = self
            .watch
            .active()
            .union(&self.groups)
            .chain(chain_keys.iter())
            .count();
        let available = MAX_TOTAL_SUBSCRIPTIONS.saturating_sub(base);
        let mut accepted = BTreeSet::new();
        let mut rejected = Vec::new();
        let mut new_keys = 0usize;
        for key in keys {
            if self.watch.active().contains(&key)
                || self.groups.contains(&key)
                || chain_keys.contains(&key)
            {
                accepted.insert(key);
            } else if new_keys < available {
                accepted.insert(key);
                new_keys += 1;
            } else {
                rejected.push(key);
            }
        }
        self.positions = accepted;
        rejected.extend(self.clamp_chains());
        self.reconcile(before, rejected)
    }

    fn chain_key_count(&self) -> usize {
        self.chains.values().map(BTreeSet::len).sum()
    }

    /// Sorted unique keys the live feed must subscribe to right now.
    pub fn subscription_keys(&self) -> Vec<SymbolKey> {
        self.desired_keys().into_iter().collect()
    }

    /// Market-watch rows are the active watch list **plus** every group member.
    /// This is the user-facing "watching" set; the option-chain legs stay out.
    pub fn market_watch_keys(&self) -> Vec<SymbolKey> {
        self.watch.active().union(&self.groups).cloned().collect()
    }

    pub fn market_watch_quotes(&self) -> Vec<QuoteDto> {
        let master = self.master.as_deref();
        self.market_watch_keys()
            .iter()
            .filter_map(|key| self.book.get(key))
            .map(|quote| quote_dto(quote, master))
            .collect()
    }

    /// Replace the group-card subscription set.
    ///
    /// Group members that are also in the active watch or chain are free; the
    /// remaining slots are taken up to the global 100-instrument budget. Excess
    /// keys are reported in the plan rather than silently growing the socket.
    pub fn set_group_keys(&mut self, keys: Vec<SymbolKey>) -> PlanDto {
        let before = self.desired_keys();
        let chain_keys: BTreeSet<SymbolKey> = self
            .chains
            .values()
            .flat_map(|group| group.iter().cloned())
            .collect();
        let base = self
            .watch
            .active()
            .union(&self.groups)
            .chain(self.positions.iter())
            .chain(chain_keys.iter())
            .count();
        let available = MAX_TOTAL_SUBSCRIPTIONS.saturating_sub(base);

        let mut accepted: BTreeSet<SymbolKey> = BTreeSet::new();
        let mut rejected: Vec<SymbolKey> = Vec::new();
        for key in keys {
            if self.watch.active().contains(&key)
                || chain_keys.contains(&key)
                || accepted.len() < available
            {
                accepted.insert(key);
            } else {
                rejected.push(key);
            }
        }
        self.groups = accepted;
        // Group growth can push chain groups over the budget.
        rejected.extend(self.clamp_chains());
        self.reconcile(before, rejected)
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
        // Growing the watch list can push chain groups over the global budget.
        rejected.extend(self.clamp_chains());
        self.reconcile(before, rejected)
    }

    /// Trim all chain groups so the union stays within
    /// [`MAX_TOTAL_SUBSCRIPTIONS`]. Existing groups keep their first keys; later
    /// groups yield first. Returns the dropped keys.
    fn clamp_chains(&mut self) -> Vec<SymbolKey> {
        let base = self
            .watch
            .active()
            .union(&self.groups)
            .chain(self.positions.iter())
            .count();
        let mut available = MAX_TOTAL_SUBSCRIPTIONS.saturating_sub(base);
        let mut dropped: Vec<SymbolKey> = Vec::new();
        for group in self.chains.values_mut() {
            if group.len() <= available {
                available -= group.len();
                continue;
            }
            let keep: BTreeSet<SymbolKey> = group.iter().take(available).cloned().collect();
            dropped.extend(group.difference(&keep).cloned());
            *group = keep;
            available = 0;
        }
        dropped
    }

    /// Replace one chain tab's subscription group. Other pinned chains stay
    /// intact; only the remaining global budget is used by this group.
    pub fn set_chain(&mut self, chain_id: &str, keys: Vec<SymbolKey>) -> PlanDto {
        let before = self.desired_keys();
        let target: BTreeSet<SymbolKey> = keys.into_iter().collect();

        // Keys already subscribed elsewhere are free; only new keys consume
        // the global budget.
        let mut free_keys: BTreeSet<SymbolKey> =
            self.watch.active().union(&self.groups).cloned().collect();
        free_keys.extend(self.positions.iter().cloned());
        for (id, group) in &self.chains {
            if id.as_str() != chain_id {
                free_keys.extend(group.iter().cloned());
            }
        }
        let available = self
            .chain_limit
            .min(MAX_TOTAL_SUBSCRIPTIONS.saturating_sub(free_keys.len()));

        let mut accepted: BTreeSet<SymbolKey> = BTreeSet::new();
        let mut rejected: Vec<SymbolKey> = Vec::new();
        let mut new_keys = 0usize;
        for key in target {
            if free_keys.contains(&key) {
                accepted.insert(key);
            } else if new_keys < available {
                accepted.insert(key);
                new_keys += 1;
            } else {
                rejected.push(key);
            }
        }
        self.chains.insert(chain_id.to_string(), accepted);
        if !rejected.is_empty() {
            tracing::warn!(
                chain = chain_id,
                requested = rejected.len() + self.chains[chain_id].len(),
                accepted = self.chains[chain_id].len(),
                dropped = rejected.len(),
                "option chain request clamped to the subscription budget"
            );
        }
        self.reconcile(before, rejected)
    }

    /// Drop one chain tab's subscriptions, leaving other pinned chains intact.
    pub fn clear_chain(&mut self, chain_id: &str) -> PlanDto {
        let before = self.desired_keys();
        self.chains.remove(chain_id);
        self.reconcile(before, Vec::new())
    }

    /// Drop every chain subscription (used when leaving all chain tabs).
    pub fn clear_all_chains(&mut self) -> PlanDto {
        let before = self.desired_keys();
        self.chains.clear();
        self.reconcile(before, Vec::new())
    }

    /// Total option-chain legs across all pinned tabs.
    pub fn chain_len(&self) -> usize {
        self.chain_key_count()
    }

    pub fn chain_contains(&self, chain_id: &str, key: &SymbolKey) -> bool {
        self.chains
            .get(chain_id)
            .map(|group| group.contains(key))
            .unwrap_or(false)
    }

    /// Total unique instruments currently subscribed (watch + groups + chains).
    pub fn subscribed_len(&self) -> usize {
        self.desired_keys().len()
    }

    /// One option-chain row: static instrument metadata plus the live quote.
    pub fn chain_row(&self, instrument: &Instrument, live: bool) -> ChainRowDto {
        ChainRowDto {
            key: instrument.key.as_wire(),
            exchange: instrument.key.exchange().to_string(),
            symbol: instrument.trading_symbol.clone(),
            expiry: instrument.expiry.clone(),
            strike: instrument.strike.format(2),
            strike_type: instrument.strike_type.as_str().to_string(),
            lot_size: instrument.lot_size,
            quote: self
                .book
                .get(&instrument.key)
                .map(|q| quote_dto(q, self.master.as_deref())),
            live,
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
            watched: self.market_watch_keys().len(),
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
                .map(|(instrument, _)| instrument_dto(instrument, self.master.as_deref()))
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
            .map(|i| instrument_dto(i, self.master.as_deref()))
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

fn quote_dto(quote: &Quote, master: Option<&MemoryMaster>) -> QuoteDto {
    let instrument = master.and_then(|m| m.get(&quote.key));
    let is_index = instrument.map(fv_master::is_index).unwrap_or(false);
    let underlying = instrument.and_then(|i| i.underlying.clone());
    let has_fno = match (master, underlying.as_deref()) {
        (Some(m), Some(u)) => m.has_derivatives(u),
        _ => false,
    };

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
        oi: quote.oi,
        prev_oi: quote.prev_oi,
        bid: quote.best_bid.map(|p| price_dto(p, d)),
        ask: quote.best_ask.map(|p| price_dto(p, d)),
        bid_qty: quote.best_bid_qty,
        ask_qty: quote.best_ask_qty,
        trend,
        seq: quote.seq,
        updated_at_ms: quote.updated_at_ms,
        is_index,
        underlying: underlying.clone(),
        has_fno,
    }
}

/// A row for an instrument with no ticks yet: metadata only, no prices.
fn placeholder_dto(instrument: &Instrument, master: Option<&MemoryMaster>) -> QuoteDto {
    let has_fno = match (master, instrument.underlying.as_deref()) {
        (Some(m), Some(u)) => m.has_derivatives(u),
        _ => false,
    };
    QuoteDto {
        key: instrument.key.as_wire(),
        trading_symbol: Some(instrument.trading_symbol.clone()),
        ltp: None,
        change: None,
        pct: None,
        open: None,
        high: None,
        low: None,
        prev_close: None,
        volume: None,
        oi: None,
        prev_oi: None,
        bid: None,
        ask: None,
        bid_qty: None,
        ask_qty: None,
        trend: Trend::Flat,
        seq: 0,
        updated_at_ms: 0,
        is_index: fv_master::is_index(instrument),
        underlying: instrument.underlying.clone(),
        has_fno,
    }
}

fn instrument_dto(instrument: &Instrument, master: Option<&MemoryMaster>) -> InstrumentDto {
    let has_fno = match (master, instrument.underlying.as_deref()) {
        (Some(m), Some(u)) => m.has_derivatives(u),
        _ => false,
    };
    InstrumentDto {
        has_fno,
        key: instrument.key.as_wire(),
        trading_symbol: instrument.trading_symbol.clone(),
        name: instrument.name.clone(),
        underlying: instrument.underlying.clone(),
        expiry: instrument.expiry.clone(),
        strike: instrument.strike.format(2),
        strike_type: instrument.strike_type.as_str().to_string(),
        lot_size: instrument.lot_size,
        instrument_type: instrument.instrument_type.clone(),
        is_index: fv_master::is_index(instrument),
    }
}

/// Compile-time-ish build label for the status bar.
pub fn build_label(version: &str, date: &str) -> String {
    format!("{version} ({date})")
}
