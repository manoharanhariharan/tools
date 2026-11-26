//! Document models shared by both cache backends.
//!
//! `TrackerEntry` and `SnapshotDoc` mirror the field names already present in the
//! local `STOCK` database so the new app reads existing data without a migration.
//! `OrderDoc` targets `FV_ORDERS`, which is currently empty and therefore free to
//! be defined by this app.

use fv_core::Price;
use serde::{Deserialize, Serialize};

/// Mirrors `STOCK.DATA_STORED_TRACKER`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackerEntry {
    pub underlying: String,
    /// Composite instrument key as used by the existing data, e.g. `NIFTY:EQ:0:`.
    pub instrument: String,
    /// `YYYY-MM-DD`.
    pub date_updated_on: String,
    pub found_instrument: bool,
    pub not_found_count: i32,
    pub last_updated: Option<String>,
}

impl TrackerEntry {
    pub fn new(
        underlying: impl Into<String>,
        instrument: impl Into<String>,
        date: impl Into<String>,
    ) -> Self {
        TrackerEntry {
            underlying: underlying.into(),
            instrument: instrument.into(),
            date_updated_on: date.into(),
            found_instrument: false,
            not_found_count: 0,
            last_updated: None,
        }
    }

    pub fn found(mut self) -> Self {
        self.found_instrument = true;
        self.not_found_count = 0;
        self
    }

    pub fn not_found(mut self) -> Self {
        self.found_instrument = false;
        self.not_found_count = self.not_found_count.saturating_add(1);
        self
    }

    pub fn key(&self) -> (String, String) {
        (self.underlying.clone(), self.instrument.clone())
    }
}

/// One intraday snapshot row. Mirrors `STOCK.SNAPSHOT_INTRADAY`.
///
/// The legacy collection stores OHLC as BSON doubles. Money is modelled as
/// [`Price`] here and converted at the Mongo boundary, so arithmetic everywhere
/// else stays exact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotDoc {
    /// `YYYY-MM-DDTHH:MM:SS` of the candle/bucket start.
    pub candle: String,
    pub underlying: String,
    pub strike_type: String,
    pub strike_price: Price,
    pub expiry: String,
    pub lot_size: i64,
    pub open: Price,
    pub high: Price,
    pub low: Price,
    pub close: Price,
    pub volume: i64,
    pub total_volume_traded: i64,
    pub cutoff_volume_traded: Option<i64>,
    pub oi: i64,
    pub snapshot_time: String,
    /// `YYYY-MM-DD`.
    pub date_stored_for: String,
}

impl SnapshotDoc {
    /// Natural identity of a snapshot row, used for idempotent upserts.
    pub fn identity(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.date_stored_for,
            self.candle,
            self.underlying,
            self.strike_type,
            self.strike_price.format(2)
        )
    }
}

/// One journaled order event. Targets `STOCK.FV_ORDERS`.
///
/// The journal is append-oriented but idempotent: `event_key` is the natural key,
/// so a reconnect replay of the same `om` frame does not duplicate a row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderDoc {
    pub event_key: String,
    pub noren_order_no: String,
    pub account_id: Option<String>,
    pub exchange: Option<String>,
    pub trading_symbol: Option<String>,
    pub status: Option<String>,
    pub report_type: Option<String>,
    pub transaction_type: Option<String>,
    pub product: Option<String>,
    pub price_type: Option<String>,
    pub retention: Option<String>,
    pub quantity: Option<i64>,
    pub filled_quantity: Option<i64>,
    pub price: Option<Price>,
    pub average_price: Option<Price>,
    pub exchange_order_id: Option<String>,
    pub fill_id: Option<String>,
    pub fill_time: Option<String>,
    pub remarks: Option<String>,
    pub journaled_at: String,
}

impl OrderDoc {
    /// Derive the idempotency key from the order number plus the lifecycle event.
    ///
    /// A fill event is additionally keyed by fill id, because one order can have
    /// several partial fills that all share the same status string.
    pub fn event_key(
        noren_order_no: &str,
        report_type: Option<&str>,
        status: Option<&str>,
        fill_id: Option<&str>,
    ) -> String {
        format!(
            "{}|{}|{}|{}",
            noren_order_no,
            report_type.unwrap_or("-"),
            status.unwrap_or("-"),
            fill_id.unwrap_or("-")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_event_key_is_stable_and_disambiguates_fills() {
        let a = OrderDoc::event_key(
            "24121500001234",
            Some("Fill"),
            Some("COMPLETE"),
            Some("8841"),
        );
        let b = OrderDoc::event_key(
            "24121500001234",
            Some("Fill"),
            Some("COMPLETE"),
            Some("8841"),
        );
        let c = OrderDoc::event_key(
            "24121500001234",
            Some("Fill"),
            Some("COMPLETE"),
            Some("8842"),
        );
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn tracker_found_and_not_found_transitions() {
        let e = TrackerEntry::new("NIFTY", "NIFTY:EQ:0:", "2026-11-26")
            .not_found()
            .not_found();
        assert_eq!(e.not_found_count, 2);
        assert!(!e.found_instrument);
        let e = e.found();
        assert_eq!(e.not_found_count, 0);
        assert!(e.found_instrument);
    }

    #[test]
    fn snapshot_identity_distinguishes_strikes() {
        let mut d = SnapshotDoc {
            candle: "2026-11-11T09:14:00".into(),
            underlying: "NIFTY".into(),
            strike_type: "CE".into(),
            strike_price: Price::parse("24000").unwrap(),
            expiry: "2026-11-26".into(),
            lot_size: 75,
            open: Price::ZERO,
            high: Price::ZERO,
            low: Price::ZERO,
            close: Price::ZERO,
            volume: 0,
            total_volume_traded: 0,
            cutoff_volume_traded: None,
            oi: 0,
            snapshot_time: "09:14:00".into(),
            date_stored_for: "2026-11-11".into(),
        };
        let a = d.identity();
        d.strike_price = Price::parse("24100").unwrap();
        assert_ne!(a, d.identity());
    }
}
