//! Coalesced delta frames for the UI.
//!
//! Invariant: at most `max_per_frame` quotes leave the engine per interval, and
//! the remainder is *carried*, never dropped. That bounds the IPC payload and the
//! render cost per frame regardless of how many instruments are subscribed.

use fv_core::{Quote, SymbolKey};
use fv_feed::QuoteBook;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

/// One batch of changed quotes, ready to send to the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct DeltaBatch {
    /// Monotonic batch counter; the UI uses it to detect gaps.
    pub seq: u64,
    /// Local wall clock (epoch millis) at emit time.
    pub timestamp_ms: i64,
    /// Changed quotes, in deterministic key order.
    pub quotes: Vec<Quote>,
    /// Instruments still dirty after this batch.
    pub pending: usize,
}

impl DeltaBatch {
    pub fn is_empty(&self) -> bool {
        self.quotes.is_empty()
    }
}

/// Absorbs dirty notifications and emits bounded batches at a fixed rate.
#[derive(Debug)]
pub struct Coalescer {
    interval: Duration,
    max_per_frame: usize,
    dirty: BTreeSet<SymbolKey>,
    last_emit: Option<Instant>,
    seq: u64,
}

impl Coalescer {
    /// `hz` is the UI refresh rate (30 or 60). `max_per_frame` bounds payload
    /// size; with 500 scrips at 30 Hz, 600 is comfortable.
    pub fn new(hz: u32, max_per_frame: usize) -> Self {
        let hz = hz.max(1);
        Coalescer {
            interval: Duration::from_micros(1_000_000 / hz as u64),
            max_per_frame: max_per_frame.max(1),
            dirty: BTreeSet::new(),
            last_emit: None,
            seq: 0,
        }
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    pub fn max_per_frame(&self) -> usize {
        self.max_per_frame
    }

    /// Mark one instrument as changed. Duplicate marks collapse.
    pub fn mark(&mut self, key: SymbolKey) {
        self.dirty.insert(key);
    }

    pub fn mark_all(&mut self, keys: impl IntoIterator<Item = SymbolKey>) {
        self.dirty.extend(keys);
    }

    pub fn pending(&self) -> usize {
        self.dirty.len()
    }

    /// Emit if the interval has elapsed and there is something to send.
    pub fn flush_due(&mut self, now: Instant, now_ms: i64, book: &QuoteBook) -> Option<DeltaBatch> {
        if self.dirty.is_empty() {
            return None;
        }
        if let Some(last) = self.last_emit {
            if now.saturating_duration_since(last) < self.interval {
                return None;
            }
        }
        Some(self.emit(now, now_ms, book))
    }

    /// Emit regardless of the interval (window focus, shutdown, tests).
    pub fn flush_now(&mut self, now: Instant, now_ms: i64, book: &QuoteBook) -> Option<DeltaBatch> {
        if self.dirty.is_empty() {
            return None;
        }
        Some(self.emit(now, now_ms, book))
    }

    fn emit(&mut self, now: Instant, now_ms: i64, book: &QuoteBook) -> DeltaBatch {
        let keys: Vec<SymbolKey> = self
            .dirty
            .iter()
            .take(self.max_per_frame)
            .cloned()
            .collect();

        let mut quotes = Vec::with_capacity(keys.len());
        for key in &keys {
            if let Some(q) = book.get(key) {
                quotes.push(q.clone());
                self.dirty.remove(key);
            } else {
                // Marked dirty but no quote yet (frame not applied). Drop the
                // mark so it cannot spin forever; a later frame re-marks it.
                self.dirty.remove(key);
            }
        }

        self.seq += 1;
        self.last_emit = Some(now);

        DeltaBatch {
            seq: self.seq,
            timestamp_ms: now_ms,
            quotes,
            pending: self.dirty.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fv_feed::{parse, Applied};

    fn book_with(tokens: &[&str]) -> QuoteBook {
        let mut book = QuoteBook::new();
        for token in tokens {
            let raw = format!(r#"{{"t":"tk","e":"NSE","tk":"{token}","lp":"100.00"}}"#);
            let frame = parse(raw.as_bytes()).unwrap();
            assert!(matches!(book.apply(&frame, 0), Applied::Snapshot(_)));
        }
        book
    }

    fn key(token: &str) -> SymbolKey {
        SymbolKey::parse(&format!("NSE|{token}")).unwrap()
    }

    #[test]
    fn marks_collapse() {
        let mut c = Coalescer::new(30, 100);
        c.mark(key("1"));
        c.mark(key("1"));
        c.mark(key("2"));
        assert_eq!(c.pending(), 2);
    }

    #[test]
    fn flush_respects_the_interval() {
        let book = book_with(&["1"]);
        let mut c = Coalescer::new(30, 100); // ~33.3 ms
        let t0 = Instant::now();
        c.mark(key("1"));

        assert!(
            c.flush_due(t0, 0, &book).is_some(),
            "first flush is immediate"
        );
        c.mark(key("1"));
        assert!(
            c.flush_due(t0 + Duration::from_millis(10), 10, &book)
                .is_none(),
            "must not emit inside the interval"
        );
        assert!(
            c.flush_due(t0 + Duration::from_millis(40), 40, &book)
                .is_some(),
            "must emit after the interval"
        );
    }

    #[test]
    fn nothing_dirty_means_no_batch() {
        let book = book_with(&["1"]);
        let mut c = Coalescer::new(30, 100);
        assert!(c.flush_due(Instant::now(), 0, &book).is_none());
        let t = Instant::now();
        assert!(c.flush_now(t, 0, &book).is_none());
    }

    #[test]
    fn batch_is_bounded_and_remainder_carries_over() {
        let tokens: Vec<String> = (1..=10).map(|i| i.to_string()).collect();
        let refs: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
        let book = book_with(&refs);

        let mut c = Coalescer::new(30, 4);
        c.mark_all(tokens.iter().map(|t| key(t)));
        let t0 = Instant::now();

        let b1 = c.flush_now(t0, 0, &book).unwrap();
        assert_eq!(b1.quotes.len(), 4);
        assert_eq!(b1.pending, 6);

        let b2 = c.flush_now(t0, 0, &book).unwrap();
        assert_eq!(b2.quotes.len(), 4);
        assert_eq!(b2.pending, 2);

        let b3 = c.flush_now(t0, 0, &book).unwrap();
        assert_eq!(b3.quotes.len(), 2);
        assert_eq!(b3.pending, 0);

        assert!(c.flush_now(t0, 0, &book).is_none());
        // Sequence numbers are monotonic and gap-free.
        assert_eq!((b1.seq, b2.seq, b3.seq), (1, 2, 3));
    }

    #[test]
    fn batch_order_is_deterministic() {
        let book = book_with(&["30", "10", "20"]);
        let mut c = Coalescer::new(30, 10);
        c.mark(key("30"));
        c.mark(key("10"));
        c.mark(key("20"));
        let batch = c.flush_now(Instant::now(), 0, &book).unwrap();
        let order: Vec<String> = batch.quotes.iter().map(|q| q.key.as_wire()).collect();
        assert_eq!(order, vec!["NSE|10", "NSE|20", "NSE|30"]);
    }

    #[test]
    fn dirty_keys_without_quotes_are_dropped_not_looped() {
        let book = book_with(&["1"]);
        let mut c = Coalescer::new(30, 10);
        c.mark(key("1"));
        c.mark(key("999")); // never received a snapshot
        let batch = c.flush_now(Instant::now(), 0, &book).unwrap();
        assert_eq!(batch.quotes.len(), 1);
        assert_eq!(c.pending(), 0, "unknown key must not stay dirty forever");
    }

    #[test]
    fn one_delta_per_instrument_per_batch() {
        let book = book_with(&["1"]);
        let mut c = Coalescer::new(30, 10);
        for _ in 0..1000 {
            c.mark(key("1"));
        }
        let batch = c.flush_now(Instant::now(), 0, &book).unwrap();
        assert_eq!(batch.quotes.len(), 1);
    }
}
