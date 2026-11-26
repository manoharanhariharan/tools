//! In-memory cache. Used by tests, CI, and when MongoDB is unavailable.

use crate::model::{OrderDoc, SnapshotDoc, TrackerEntry};
use crate::StoreError;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct MemoryStore {
    /// key: (underlying, instrument)
    trackers: Mutex<HashMap<(String, String), TrackerEntry>>,
    snapshots: Mutex<Vec<SnapshotDoc>>,
    orders: Mutex<Vec<OrderDoc>>,
}

impl MemoryStore {
    pub fn tracker_get(
        &self,
        underlying: &str,
        instrument: &str,
    ) -> Result<Option<TrackerEntry>, StoreError> {
        let map = self.trackers.lock().expect("tracker mutex poisoned");
        Ok(map
            .get(&(underlying.to_string(), instrument.to_string()))
            .cloned())
    }

    pub fn tracker_upsert(&self, entry: &TrackerEntry) -> Result<(), StoreError> {
        let mut map = self.trackers.lock().expect("tracker mutex poisoned");
        map.insert(entry.key(), entry.clone());
        Ok(())
    }

    pub fn append_snapshot(&self, doc: &SnapshotDoc) -> Result<(), StoreError> {
        let mut rows = self.snapshots.lock().expect("snapshot mutex poisoned");
        let id = doc.identity();
        if let Some(existing) = rows.iter_mut().find(|r| r.identity() == id) {
            *existing = doc.clone();
        } else {
            rows.push(doc.clone());
        }
        Ok(())
    }

    pub fn snapshots_for(
        &self,
        underlying: &str,
        date: &str,
    ) -> Result<Vec<SnapshotDoc>, StoreError> {
        let rows = self.snapshots.lock().expect("snapshot mutex poisoned");
        let mut out: Vec<SnapshotDoc> = rows
            .iter()
            .filter(|r| r.underlying == underlying && r.date_stored_for == date)
            .cloned()
            .collect();
        out.sort_by(|a, b| a.candle.cmp(&b.candle));
        Ok(out)
    }

    pub fn journal_order(&self, doc: &OrderDoc) -> Result<(), StoreError> {
        let mut rows = self.orders.lock().expect("order mutex poisoned");
        if let Some(existing) = rows.iter_mut().find(|r| r.event_key == doc.event_key) {
            *existing = doc.clone();
        } else {
            rows.push(doc.clone());
        }
        Ok(())
    }

    pub fn recent_orders(&self, limit: u32) -> Result<Vec<OrderDoc>, StoreError> {
        let rows = self.orders.lock().expect("order mutex poisoned");
        let mut out: Vec<OrderDoc> = rows.iter().rev().take(limit as usize).cloned().collect();
        out.reverse();
        Ok(out)
    }

    pub fn snapshot_count(&self) -> usize {
        self.snapshots.lock().map(|s| s.len()).unwrap_or(0)
    }

    pub fn order_count(&self) -> usize {
        self.orders.lock().map(|s| s.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fv_core::Price;

    fn snapshot(candle: &str, strike: &str) -> SnapshotDoc {
        SnapshotDoc {
            candle: candle.into(),
            underlying: "NIFTY".into(),
            strike_type: "CE".into(),
            strike_price: Price::parse(strike).unwrap(),
            expiry: "2026-11-26".into(),
            lot_size: 75,
            open: Price::parse("100").unwrap(),
            high: Price::parse("110").unwrap(),
            low: Price::parse("90").unwrap(),
            close: Price::parse("105").unwrap(),
            volume: 10,
            total_volume_traded: 10,
            cutoff_volume_traded: None,
            oi: 5,
            snapshot_time: "09:14:00".into(),
            date_stored_for: "2026-11-11".into(),
        }
    }

    #[test]
    fn tracker_round_trip() {
        let s = MemoryStore::default();
        let e = TrackerEntry::new("NIFTY", "NIFTY:EQ:0:", "2026-11-11").found();
        s.tracker_upsert(&e).unwrap();
        let got = s.tracker_get("NIFTY", "NIFTY:EQ:0:").unwrap().unwrap();
        assert_eq!(got, e);
        assert!(s.tracker_get("NIFTY", "MISSING").unwrap().is_none());
    }

    #[test]
    fn tracker_upsert_replaces_by_key() {
        let s = MemoryStore::default();
        s.tracker_upsert(&TrackerEntry::new("NIFTY", "k", "2026-11-11"))
            .unwrap();
        s.tracker_upsert(&TrackerEntry::new("NIFTY", "k", "2026-11-12").found())
            .unwrap();
        let got = s.tracker_get("NIFTY", "k").unwrap().unwrap();
        assert_eq!(got.date_updated_on, "2026-11-12");
        assert!(got.found_instrument);
    }

    #[test]
    fn snapshots_are_idempotent_and_sorted() {
        let s = MemoryStore::default();
        s.append_snapshot(&snapshot("2026-11-11T09:20:00", "24000"))
            .unwrap();
        s.append_snapshot(&snapshot("2026-11-11T09:14:00", "24000"))
            .unwrap();
        // Same identity as the first insert: replaces, does not duplicate.
        s.append_snapshot(&snapshot("2026-11-11T09:20:00", "24000"))
            .unwrap();
        assert_eq!(s.snapshot_count(), 2);

        let rows = s.snapshots_for("NIFTY", "2026-11-11").unwrap();
        assert_eq!(rows[0].candle, "2026-11-11T09:14:00");
        assert_eq!(rows[1].candle, "2026-11-11T09:20:00");
    }

    #[test]
    fn snapshots_are_filtered_by_underlying_and_date() {
        let s = MemoryStore::default();
        s.append_snapshot(&snapshot("2026-11-11T09:14:00", "24000"))
            .unwrap();
        assert!(s
            .snapshots_for("BANKNIFTY", "2026-11-11")
            .unwrap()
            .is_empty());
        assert!(s.snapshots_for("NIFTY", "2026-11-12").unwrap().is_empty());
    }

    #[test]
    fn order_journal_is_idempotent() {
        let s = MemoryStore::default();
        let mut o = OrderDoc {
            event_key: OrderDoc::event_key("1", Some("Fill"), Some("COMPLETE"), Some("8841")),
            noren_order_no: "1".into(),
            account_id: None,
            exchange: Some("NSE".into()),
            trading_symbol: Some("RELIANCE-EQ".into()),
            status: Some("COMPLETE".into()),
            report_type: Some("Fill".into()),
            transaction_type: Some("B".into()),
            product: Some("C".into()),
            price_type: Some("LMT".into()),
            retention: Some("DAY".into()),
            quantity: Some(10),
            filled_quantity: Some(4),
            price: Some(Price::parse("1272.30").unwrap()),
            average_price: Some(Price::parse("1272.30").unwrap()),
            exchange_order_id: None,
            fill_id: Some("8841".into()),
            fill_time: None,
            remarks: None,
            journaled_at: "2026-11-26T10:00:00Z".into(),
        };
        s.journal_order(&o).unwrap();
        o.filled_quantity = Some(5);
        s.journal_order(&o).unwrap();
        assert_eq!(s.order_count(), 1, "same event must not duplicate");
        let rows = s.recent_orders(10).unwrap();
        assert_eq!(rows[0].filled_quantity, Some(5));
    }
}
