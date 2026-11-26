//! MongoDB backend.
//!
//! Reads and writes the collections already present in the local `STOCK`
//! database. Collection names are overridable so a different environment (or a
//! test database) needs no code change.

use crate::model::{OrderDoc, SnapshotDoc, TrackerEntry};
use crate::StoreError;
use bson::{doc, Bson, Document};
use futures_util::TryStreamExt;
use fv_core::Price;
use mongodb::Client;

pub const DEFAULT_TRACKER_COLLECTION: &str = "DATA_STORED_TRACKER";
pub const DEFAULT_SNAPSHOT_COLLECTION: &str = "SNAPSHOT_INTRADAY";
pub const DEFAULT_ORDER_COLLECTION: &str = "FV_ORDERS";

pub struct MongoStore {
    client: Client,
    database: String,
    tracker_collection: String,
    snapshot_collection: String,
    order_collection: String,
}

impl std::fmt::Debug for MongoStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MongoStore")
            .field("database", &self.database)
            .field("tracker_collection", &self.tracker_collection)
            .field("snapshot_collection", &self.snapshot_collection)
            .field("order_collection", &self.order_collection)
            .finish()
    }
}

impl MongoStore {
    pub async fn connect(uri: &str, database: &str) -> Result<Self, StoreError> {
        let client = Client::with_uri_str(uri)
            .await
            .map_err(|e| StoreError::Mongo(e.to_string()))?;
        Ok(MongoStore {
            client,
            database: database.to_string(),
            tracker_collection: DEFAULT_TRACKER_COLLECTION.to_string(),
            snapshot_collection: DEFAULT_SNAPSHOT_COLLECTION.to_string(),
            order_collection: DEFAULT_ORDER_COLLECTION.to_string(),
        })
    }

    pub fn with_collections(
        mut self,
        tracker: impl Into<String>,
        snapshot: impl Into<String>,
        order: impl Into<String>,
    ) -> Self {
        self.tracker_collection = tracker.into();
        self.snapshot_collection = snapshot.into();
        self.order_collection = order.into();
        self
    }

    fn db(&self) -> mongodb::Database {
        self.client.database(&self.database)
    }

    /// Terminate the driver's background workers. Must be awaited before the
    /// tokio runtime is dropped, otherwise the pool panics on a closed channel.
    pub async fn shutdown(self) {
        let MongoStore { client, .. } = self;
        client.shutdown().await;
    }

    pub async fn ping(&self) -> Result<(), StoreError> {
        self.db()
            .run_command(doc! { "ping": 1 })
            .await
            .map(|_| ())
            .map_err(|e| StoreError::Mongo(e.to_string()))
    }

    pub async fn tracker_get(
        &self,
        underlying: &str,
        instrument: &str,
    ) -> Result<Option<TrackerEntry>, StoreError> {
        let coll = self.db().collection::<Document>(&self.tracker_collection);
        let found = coll
            .find_one(doc! { "underlying": underlying, "instrument": instrument })
            .await
            .map_err(|e| StoreError::Mongo(e.to_string()))?;
        Ok(found.as_ref().and_then(tracker_from_doc))
    }

    pub async fn tracker_upsert(&self, entry: &TrackerEntry) -> Result<(), StoreError> {
        let coll = self.db().collection::<Document>(&self.tracker_collection);
        let set = doc! {
            "underlying": &entry.underlying,
            "instrument": &entry.instrument,
            "date_updated_on": &entry.date_updated_on,
            "found_instrument": entry.found_instrument,
            "notFoundCount": entry.not_found_count,
            "lastUpdated": entry.last_updated.clone().unwrap_or_default(),
        };
        coll.update_one(
            doc! { "underlying": &entry.underlying, "instrument": &entry.instrument },
            doc! { "$set": set },
        )
        .upsert(true)
        .await
        .map(|_| ())
        .map_err(|e| StoreError::Mongo(e.to_string()))
    }

    pub async fn append_snapshot(&self, snapshot: &SnapshotDoc) -> Result<(), StoreError> {
        let coll = self.db().collection::<Document>(&self.snapshot_collection);
        let filter = doc! {
            "dateStoredFor": &snapshot.date_stored_for,
            "candle": &snapshot.candle,
            "underlying": &snapshot.underlying,
            "strikeType": &snapshot.strike_type,
            "strikePrice": snapshot.strike_price.as_f64(),
        };
        // The legacy collection stores OHLC as doubles; converting at the
        // boundary keeps exact arithmetic inside the app.
        let set = doc! {
            "expiry": &snapshot.expiry,
            "lotSize": snapshot.lot_size,
            "open": snapshot.open.as_f64(),
            "high": snapshot.high.as_f64(),
            "low": snapshot.low.as_f64(),
            "close": snapshot.close.as_f64(),
            "volume": snapshot.volume,
            "totalVolumeTraded": snapshot.total_volume_traded,
            "cutoffVolumeTraded": snapshot.cutoff_volume_traded,
            "oi": snapshot.oi,
            "snapshotTime": &snapshot.snapshot_time,
            "dateStoredFor": &snapshot.date_stored_for,
            "candle": &snapshot.candle,
            "underlying": &snapshot.underlying,
            "strikeType": &snapshot.strike_type,
            "strikePrice": snapshot.strike_price.as_f64(),
        };
        coll.update_one(filter, doc! { "$set": set })
            .upsert(true)
            .await
            .map(|_| ())
            .map_err(|e| StoreError::Mongo(e.to_string()))
    }

    pub async fn snapshots_for(
        &self,
        underlying: &str,
        date: &str,
    ) -> Result<Vec<SnapshotDoc>, StoreError> {
        let coll = self.db().collection::<Document>(&self.snapshot_collection);
        let mut cursor = coll
            .find(doc! { "underlying": underlying, "dateStoredFor": date })
            .sort(doc! { "candle": 1 })
            .await
            .map_err(|e| StoreError::Mongo(e.to_string()))?;
        let mut out = Vec::new();
        while let Some(d) = cursor
            .try_next()
            .await
            .map_err(|e| StoreError::Mongo(e.to_string()))?
        {
            if let Some(s) = snapshot_from_doc(&d) {
                out.push(s);
            }
        }
        Ok(out)
    }

    pub async fn journal_order(&self, order: &OrderDoc) -> Result<(), StoreError> {
        let coll = self.db().collection::<Document>(&self.order_collection);
        let set = order_to_doc(order);
        coll.update_one(doc! { "event_key": &order.event_key }, doc! { "$set": set })
            .upsert(true)
            .await
            .map(|_| ())
            .map_err(|e| StoreError::Mongo(e.to_string()))
    }

    pub async fn recent_orders(&self, limit: u32) -> Result<Vec<OrderDoc>, StoreError> {
        let coll = self.db().collection::<Document>(&self.order_collection);
        let mut cursor = coll
            .find(doc! {})
            .sort(doc! { "journaled_at": -1 })
            .limit(limit as i64)
            .await
            .map_err(|e| StoreError::Mongo(e.to_string()))?;
        let mut out = Vec::new();
        while let Some(d) = cursor
            .try_next()
            .await
            .map_err(|e| StoreError::Mongo(e.to_string()))?
        {
            if let Some(o) = order_from_doc(&d) {
                out.push(o);
            }
        }
        out.reverse();
        Ok(out)
    }
}

fn tracker_from_doc(doc: &Document) -> Option<TrackerEntry> {
    Some(TrackerEntry {
        underlying: doc.get_str("underlying").ok()?.to_string(),
        instrument: doc.get_str("instrument").ok()?.to_string(),
        date_updated_on: doc.get_str("date_updated_on").ok()?.to_string(),
        found_instrument: doc.get_bool("found_instrument").unwrap_or(false),
        not_found_count: doc
            .get_i32("notFoundCount")
            .ok()
            .or_else(|| doc.get_i64("notFoundCount").ok().map(|v| v as i32))
            .unwrap_or(0),
        last_updated: doc.get_str("lastUpdated").ok().map(|s| s.to_string()),
    })
}

fn price_from_bson(value: Option<&Bson>) -> Price {
    match value {
        Some(Bson::Double(f)) => Price::parse_rounded(&format!("{f}")).unwrap_or(Price::ZERO),
        Some(Bson::Int32(i)) => Price::from_scaled(*i as i64 * 1_000_000),
        Some(Bson::Int64(i)) => Price::from_scaled(*i * 1_000_000),
        Some(Bson::Decimal128(d)) => Price::parse_rounded(&d.to_string()).unwrap_or(Price::ZERO),
        Some(Bson::String(s)) => Price::parse_rounded(s).unwrap_or(Price::ZERO),
        _ => Price::ZERO,
    }
}

fn snapshot_from_doc(doc: &Document) -> Option<SnapshotDoc> {
    Some(SnapshotDoc {
        candle: doc.get_str("candle").ok()?.to_string(),
        underlying: doc.get_str("underlying").ok()?.to_string(),
        strike_type: doc.get_str("strikeType").unwrap_or("").to_string(),
        strike_price: price_from_bson(doc.get("strikePrice")),
        expiry: doc.get_str("expiry").unwrap_or("").to_string(),
        lot_size: doc.get_i64("lotSize").ok().unwrap_or(0),
        open: price_from_bson(doc.get("open")),
        high: price_from_bson(doc.get("high")),
        low: price_from_bson(doc.get("low")),
        close: price_from_bson(doc.get("close")),
        volume: doc.get_i64("volume").ok().unwrap_or(0),
        total_volume_traded: doc.get_i64("totalVolumeTraded").ok().unwrap_or(0),
        cutoff_volume_traded: doc.get_i64("cutoffVolumeTraded").ok(),
        oi: doc.get_i64("oi").ok().unwrap_or(0),
        snapshot_time: doc.get_str("snapshotTime").unwrap_or("").to_string(),
        date_stored_for: doc.get_str("dateStoredFor").ok()?.to_string(),
    })
}

/// Journal rows carry both `price_e6` (exact integer) and `price_text`
/// (human readable). The venue's decimals survive byte-exact.
fn order_to_doc(order: &OrderDoc) -> Document {
    let mut d = doc! {
        "event_key": &order.event_key,
        "noren_order_no": &order.noren_order_no,
        "journaled_at": &order.journaled_at,
    };
    macro_rules! opt_str {
        ($field:ident, $name:literal) => {
            if let Some(v) = order.$field.as_ref() {
                d.insert($name, v.clone());
            }
        };
    }
    macro_rules! opt_i64 {
        ($field:ident, $name:literal) => {
            if let Some(v) = order.$field {
                d.insert($name, v);
            }
        };
    }
    macro_rules! opt_price {
        ($field:ident, $name:literal) => {
            if let Some(p) = order.$field {
                d.insert(format!("{}_e6", $name), p.scaled());
                d.insert(format!("{}_text", $name), p.format(6));
            }
        };
    }
    opt_str!(account_id, "account_id");
    opt_str!(exchange, "exchange");
    opt_str!(trading_symbol, "trading_symbol");
    opt_str!(status, "status");
    opt_str!(report_type, "report_type");
    opt_str!(transaction_type, "transaction_type");
    opt_str!(product, "product");
    opt_str!(price_type, "price_type");
    opt_str!(retention, "retention");
    opt_i64!(quantity, "quantity");
    opt_i64!(filled_quantity, "filled_quantity");
    opt_price!(price, "price");
    opt_price!(average_price, "average_price");
    opt_str!(exchange_order_id, "exchange_order_id");
    opt_str!(fill_id, "fill_id");
    opt_str!(fill_time, "fill_time");
    opt_str!(remarks, "remarks");
    d
}

fn order_from_doc(d: &Document) -> Option<OrderDoc> {
    let price = d.get_i64("price_e6").ok().map(Price::from_scaled);
    let average_price = d.get_i64("average_price_e6").ok().map(Price::from_scaled);
    Some(OrderDoc {
        event_key: d.get_str("event_key").ok()?.to_string(),
        noren_order_no: d.get_str("noren_order_no").ok()?.to_string(),
        account_id: d.get_str("account_id").ok().map(str::to_string),
        exchange: d.get_str("exchange").ok().map(str::to_string),
        trading_symbol: d.get_str("trading_symbol").ok().map(str::to_string),
        status: d.get_str("status").ok().map(str::to_string),
        report_type: d.get_str("report_type").ok().map(str::to_string),
        transaction_type: d.get_str("transaction_type").ok().map(str::to_string),
        product: d.get_str("product").ok().map(str::to_string),
        price_type: d.get_str("price_type").ok().map(str::to_string),
        retention: d.get_str("retention").ok().map(str::to_string),
        quantity: d.get_i64("quantity").ok(),
        filled_quantity: d.get_i64("filled_quantity").ok(),
        price,
        average_price,
        exchange_order_id: d.get_str("exchange_order_id").ok().map(str::to_string),
        fill_id: d.get_str("fill_id").ok().map(str::to_string),
        fill_time: d.get_str("fill_time").ok().map(str::to_string),
        remarks: d.get_str("remarks").ok().map(str::to_string),
        journaled_at: d.get_str("journaled_at").ok()?.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_document_round_trips() {
        let mut e = TrackerEntry::new("NIFTY", "NIFTY:EQ:0:", "2026-11-26").found();
        e.last_updated = Some("2026-11-26T10:31:39".into());
        let d = doc! {
            "underlying": &e.underlying,
            "instrument": &e.instrument,
            "date_updated_on": &e.date_updated_on,
            "found_instrument": e.found_instrument,
            "notFoundCount": e.not_found_count,
            "lastUpdated": e.last_updated.clone().unwrap(),
        };
        assert_eq!(tracker_from_doc(&d).unwrap(), e);
    }

    #[test]
    fn order_document_preserves_exact_price() {
        let o = OrderDoc {
            event_key: OrderDoc::event_key("1", Some("Fill"), Some("COMPLETE"), Some("8841")),
            noren_order_no: "1".into(),
            account_id: Some("AB1234".into()),
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
            average_price: Some(Price::parse("0.000001").unwrap()),
            exchange_order_id: Some("250620000000343421".into()),
            fill_id: Some("8841".into()),
            fill_time: Some("09:20:41 15-12-2024".into()),
            remarks: Some("my_order_001".into()),
            journaled_at: "2026-11-26T10:00:00Z".into(),
        };
        let d = order_to_doc(&o);
        assert_eq!(d.get_i64("price_e6").unwrap(), 1_272_300_000);
        assert_eq!(d.get_str("price_text").unwrap(), "1272.300000");
        let back = order_from_doc(&d).unwrap();
        assert_eq!(back.price.unwrap().scaled(), o.price.unwrap().scaled());
        assert_eq!(
            back.average_price.unwrap().scaled(),
            o.average_price.unwrap().scaled(),
            "sub-paisa precision must survive the journal"
        );
        assert_eq!(back, o);
    }

    #[test]
    fn snapshot_document_parses_legacy_double_shape() {
        let d = doc! {
            "candle": "2026-11-11T09:14:00",
            "expiry": "",
            "strikePrice": 0.0_f64,
            "strikeType": "EQ",
            "underlying": "NIFTY",
            "close": 25337.8_f64,
            "dateStoredFor": "2026-11-11",
            "high": 25576.1_f64,
            "lotSize": 1_i64,
            "low": 25044.65_f64,
            "oi": 0_i64,
            "open": 25044.65_f64,
            "snapshotTime": "09:14:00",
            "totalVolumeTraded": 0_i64,
            "volume": 0_i64,
        };
        let s = snapshot_from_doc(&d).expect("parsed");
        assert_eq!(s.underlying, "NIFTY");
        assert_eq!(s.close.format(2), "25337.80");
        assert_eq!(s.date_stored_for, "2026-11-11");
        assert_eq!(s.strike_price, Price::ZERO);
    }
}
