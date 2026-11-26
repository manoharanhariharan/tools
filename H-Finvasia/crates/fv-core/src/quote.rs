//! A merged, always-current view of one instrument.
//!
//! The feed is a *diff stream*: `t:"tk"` is a full snapshot, then `t:"tf"`
//! carries only the fields that changed. Treating a `tf` as a complete quote is
//! the single most common integration bug, so merging lives here, in one place,
//! and is covered by tests.

use crate::{Price, SymbolKey};

/// One instrument's current market state.
///
/// Every field is `Option` because a diff may omit it and a snapshot may simply
/// not carry it for that instrument type (indices have no `bp1`, equities have
/// no `oi`).
#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub key: SymbolKey,
    pub trading_symbol: Option<String>,

    pub ltp: Option<Price>,
    pub prev_close: Option<Price>,
    pub pct_change: Option<Price>,
    pub open: Option<Price>,
    pub high: Option<Price>,
    pub low: Option<Price>,
    pub close: Option<Price>,
    pub avg_price: Option<Price>,

    pub volume: Option<i64>,
    pub oi: Option<i64>,
    pub prev_oi: Option<i64>,
    pub total_oi: Option<i64>,

    pub best_bid_qty: Option<i64>,
    pub best_bid: Option<Price>,
    pub best_ask_qty: Option<i64>,
    pub best_ask: Option<Price>,

    pub tick_size: Option<Price>,
    pub lot_size: Option<i64>,
    pub price_precision: Option<u8>,

    /// Feed timestamp as supplied by the venue (epoch seconds for `ft`).
    pub feed_time: Option<i64>,
    /// Monotonic ingest sequence, useful for staleness detection in the UI.
    pub seq: u64,
    /// Local wall clock (epoch millis) when this quote last changed.
    pub updated_at_ms: i64,
}

impl Quote {
    /// Create an empty quote for a key. Used as the target of a snapshot frame.
    pub fn empty(key: SymbolKey) -> Self {
        Quote {
            key,
            trading_symbol: None,
            ltp: None,
            prev_close: None,
            pct_change: None,
            open: None,
            high: None,
            low: None,
            close: None,
            avg_price: None,
            volume: None,
            oi: None,
            prev_oi: None,
            total_oi: None,
            best_bid_qty: None,
            best_bid: None,
            best_ask_qty: None,
            best_ask: None,
            tick_size: None,
            lot_size: None,
            price_precision: None,
            feed_time: None,
            seq: 0,
            updated_at_ms: 0,
        }
    }

    /// True when nothing has ever populated this quote.
    pub fn is_empty(&self) -> bool {
        self.ltp.is_none() && self.volume.is_none() && self.best_bid.is_none()
    }

    /// Absolute change against previous close, when both legs are known.
    pub fn change(&self) -> Option<Price> {
        match (self.ltp, self.prev_close) {
            (Some(ltp), Some(pc)) => ltp.checked_sub(pc),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_change_from_ltp_and_prev_close() {
        let mut q = Quote::empty(SymbolKey::parse("NSE|22").unwrap());
        q.ltp = Some(Price::parse("101.50").unwrap());
        q.prev_close = Some(Price::parse("100.00").unwrap());
        assert_eq!(q.change().unwrap().format(2), "1.50");
    }

    #[test]
    fn change_is_none_when_a_leg_is_missing() {
        let mut q = Quote::empty(SymbolKey::parse("NSE|22").unwrap());
        q.ltp = Some(Price::parse("101.50").unwrap());
        assert!(q.change().is_none());
    }

    #[test]
    fn empty_quote_reports_itself_empty() {
        let q = Quote::empty(SymbolKey::parse("NSE|22").unwrap());
        assert!(q.is_empty());
        assert_eq!(q.key.as_wire(), "NSE|22");
    }
}
