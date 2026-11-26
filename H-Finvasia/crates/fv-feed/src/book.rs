//! The merged quote book.
//!
//! Contract (from the venue docs): the first message per subscription is a full
//! snapshot (`tk`), every later message is a diff (`tf`) carrying only changed
//! fields. Merging must therefore be field-wise, and a new snapshot must *reset*
//! previously-known fields so stale values cannot survive a resubscribe.

use crate::frame::{Frame, Touchline};
use fv_core::{Quote, SymbolKey};
use std::collections::HashMap;

/// What a frame did to the book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    Ignored,
    Snapshot(SymbolKey),
    Update(SymbolKey),
}

#[derive(Debug, Default)]
pub struct QuoteBook {
    quotes: HashMap<SymbolKey, Quote>,
    seq: u64,
}

impl QuoteBook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.quotes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.quotes.is_empty()
    }

    pub fn get(&self, key: &SymbolKey) -> Option<&Quote> {
        self.quotes.get(key)
    }

    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// Keys in a deterministic order (used for stable delta frames and tests).
    pub fn keys_sorted(&self) -> Vec<SymbolKey> {
        let mut keys: Vec<SymbolKey> = self.quotes.keys().cloned().collect();
        keys.sort();
        keys
    }

    /// Apply one decoded frame.
    pub fn apply(&mut self, frame: &Frame, now_ms: i64) -> Applied {
        match frame {
            Frame::Snapshot(t) => match t.symbol() {
                Some(key) => {
                    self.seq += 1;
                    // A snapshot replaces state entirely.
                    let mut quote = Quote::empty(key.clone());
                    merge(&mut quote, t);
                    quote.seq = self.seq;
                    quote.updated_at_ms = now_ms;
                    self.quotes.insert(key.clone(), quote);
                    Applied::Snapshot(key)
                }
                None => Applied::Ignored,
            },
            Frame::Diff(t) => match t.symbol() {
                Some(key) => {
                    self.seq += 1;
                    let seq = self.seq;
                    let entry = self
                        .quotes
                        .entry(key.clone())
                        .or_insert_with(|| Quote::empty(key.clone()));
                    merge(entry, t);
                    entry.seq = seq;
                    entry.updated_at_ms = now_ms;
                    Applied::Update(key)
                }
                None => Applied::Ignored,
            },
            _ => Applied::Ignored,
        }
    }

    /// Remove instruments that are no longer subscribed, so memory tracks the
    /// watch set instead of growing for the whole session.
    pub fn retain_keys(&mut self, keep: &std::collections::BTreeSet<SymbolKey>) {
        self.quotes.retain(|k, _| keep.contains(k));
    }
}

fn merge(q: &mut Quote, t: &Touchline) {
    if let Some(v) = t.trading_symbol.as_ref() {
        q.trading_symbol = Some(v.clone());
    }
    if t.ltp.is_some() {
        q.ltp = t.ltp;
    }
    if t.pct_change.is_some() {
        q.pct_change = t.pct_change;
    }
    if t.open.is_some() {
        q.open = t.open;
    }
    if t.high.is_some() {
        q.high = t.high;
    }
    if t.low.is_some() {
        q.low = t.low;
    }
    if t.close.is_some() {
        q.close = t.close;
    }
    if t.avg_price.is_some() {
        q.avg_price = t.avg_price;
    }
    if t.volume.is_some() {
        q.volume = t.volume;
    }
    if t.oi.is_some() {
        q.oi = t.oi;
    }
    if t.prev_oi.is_some() {
        q.prev_oi = t.prev_oi;
    }
    if t.total_oi.is_some() {
        q.total_oi = t.total_oi;
    }
    if t.best_bid_qty.is_some() {
        q.best_bid_qty = t.best_bid_qty;
    }
    if t.best_bid.is_some() {
        q.best_bid = t.best_bid;
    }
    if t.best_ask_qty.is_some() {
        q.best_ask_qty = t.best_ask_qty;
    }
    if t.best_ask.is_some() {
        q.best_ask = t.best_ask;
    }
    if t.tick_size.is_some() {
        q.tick_size = t.tick_size;
    }
    if t.lot_size.is_some() {
        q.lot_size = t.lot_size;
    }
    if t.price_precision.is_some() {
        q.price_precision = t.price_precision;
    }
    if t.feed_time.is_some() {
        q.feed_time = t.feed_time;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::parse;

    fn key(s: &str) -> SymbolKey {
        SymbolKey::parse(s).unwrap()
    }

    #[test]
    fn snapshot_populates_quote() {
        let mut book = QuoteBook::new();
        let f =
            parse(br#"{"t":"tk","e":"NSE","tk":"22","lp":"1287.30","o":"1280.00","v":"980515"}"#)
                .unwrap();
        assert_eq!(book.apply(&f, 1_000), Applied::Snapshot(key("NSE|22")));
        let q = book.get(&key("NSE|22")).unwrap();
        assert_eq!(q.ltp.unwrap().format(2), "1287.30");
        assert_eq!(q.volume, Some(980_515));
        assert_eq!(q.updated_at_ms, 1_000);
        assert_eq!(q.seq, 1);
    }

    #[test]
    fn diff_preserves_fields_it_does_not_carry() {
        let mut book = QuoteBook::new();
        let snap = parse(br#"{"t":"tk","e":"NSE","tk":"22","lp":"1287.30","o":"1280.00","v":"980515","bp1":"1287.25"}"#).unwrap();
        book.apply(&snap, 1_000);

        let diff = parse(br#"{"t":"tf","e":"NSE","tk":"22","lp":"1288.00"}"#).unwrap();
        assert_eq!(book.apply(&diff, 2_000), Applied::Update(key("NSE|22")));

        let q = book.get(&key("NSE|22")).unwrap();
        assert_eq!(q.ltp.unwrap().format(2), "1288.00"); // changed
        assert_eq!(q.open.unwrap().format(2), "1280.00"); // preserved
        assert_eq!(q.volume, Some(980_515)); // preserved
        assert_eq!(q.best_bid.unwrap().format(2), "1287.25"); // preserved
        assert_eq!(q.updated_at_ms, 2_000);
        assert_eq!(q.seq, 2);
    }

    #[test]
    fn second_snapshot_resets_stale_fields() {
        let mut book = QuoteBook::new();
        let s1 =
            parse(br#"{"t":"tk","e":"NSE","tk":"22","lp":"100.00","h":"110.00","v":"5"}"#).unwrap();
        book.apply(&s1, 1);
        // Resubscribe returns a snapshot without `h` or `v`.
        let s2 = parse(br#"{"t":"tk","e":"NSE","tk":"22","lp":"101.00"}"#).unwrap();
        book.apply(&s2, 2);
        let q = book.get(&key("NSE|22")).unwrap();
        assert_eq!(q.ltp.unwrap().format(2), "101.00");
        assert!(q.high.is_none(), "stale high must not survive a snapshot");
        assert!(
            q.volume.is_none(),
            "stale volume must not survive a snapshot"
        );
    }

    #[test]
    fn diff_before_snapshot_is_tolerated() {
        let mut book = QuoteBook::new();
        let diff = parse(br#"{"t":"tf","e":"NSE","tk":"22","lp":"99.00"}"#).unwrap();
        assert_eq!(book.apply(&diff, 1), Applied::Update(key("NSE|22")));
        assert_eq!(
            book.get(&key("NSE|22")).unwrap().ltp.unwrap().format(2),
            "99.00"
        );
    }

    #[test]
    fn frames_without_symbol_are_ignored() {
        let mut book = QuoteBook::new();
        let f = parse(br#"{"t":"tf","lp":"99.00"}"#).unwrap();
        assert_eq!(book.apply(&f, 1), Applied::Ignored);
        assert!(book.is_empty());
    }

    #[test]
    fn non_quote_frames_are_ignored() {
        let mut book = QuoteBook::new();
        let f = parse(br#"{"t":"om","norenordno":"1","status":"COMPLETE"}"#).unwrap();
        assert_eq!(book.apply(&f, 1), Applied::Ignored);
    }

    #[test]
    fn retaining_keys_bounds_memory() {
        let mut book = QuoteBook::new();
        for token in ["1", "2", "3"] {
            let raw = format!(r#"{{"t":"tk","e":"NSE","tk":"{token}","lp":"10.00"}}"#);
            book.apply(&parse(raw.as_bytes()).unwrap(), 1);
        }
        let mut keep = std::collections::BTreeSet::new();
        keep.insert(key("NSE|2"));
        book.retain_keys(&keep);
        assert_eq!(book.len(), 1);
        assert!(book.get(&key("NSE|2")).is_some());
    }

    #[test]
    fn keys_are_returned_in_stable_order() {
        let mut book = QuoteBook::new();
        for token in ["30", "10", "20"] {
            let raw = format!(r#"{{"t":"tk","e":"NSE","tk":"{token}","lp":"10.00"}}"#);
            book.apply(&parse(raw.as_bytes()).unwrap(), 1);
        }
        let wire: Vec<String> = book.keys_sorted().iter().map(|k| k.as_wire()).collect();
        assert_eq!(wire, vec!["NSE|10", "NSE|20", "NSE|30"]);
    }
}
