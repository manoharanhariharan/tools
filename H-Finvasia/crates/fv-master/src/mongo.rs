//! MongoDB-backed master loading.
//!
//! Reads the two master shapes present in the local `STOCK` database and
//! normalises them into [`Instrument`]:
//!
//! * `FINVASIA_SYMBOL` — Finvasia-native (`token` is a **string**).
//! * `INSTRUMENT`      — Kite-shaped (`instrument_token` is numeric,
//!   `tradingsymbol` is lower-case).
//!
//! Both are read tolerantly: field types vary between Int32/Int64/Double/String
//! and expiry is a string in one collection and a BSON date in the other.

use crate::index::{Instrument, MasterError, MemoryMaster, StrikeType};
use bson::{doc, Bson, Document};
use futures_util::TryStreamExt;
use fv_core::{Price, SymbolKey};
use mongodb::Client;

/// Where to load the master from.
#[derive(Debug, Clone)]
pub struct MasterConfig {
    pub uri: String,
    pub database: String,
    pub collection: String,
    pub limit: Option<i64>,
}

impl Default for MasterConfig {
    fn default() -> Self {
        MasterConfig {
            uri: "mongodb://127.0.0.1:27017".to_string(),
            database: "STOCK".to_string(),
            collection: "FINVASIA_SYMBOL".to_string(),
            limit: None,
        }
    }
}

impl MasterConfig {
    /// Read from the environment, falling back to locals. This keeps macOS and
    /// Linux developers working without code changes.
    pub fn from_env() -> Self {
        let mut cfg = MasterConfig::default();
        if let Ok(uri) = std::env::var("FV_MONGO_URI") {
            if !uri.trim().is_empty() {
                cfg.uri = uri;
            }
        }
        if let Ok(db) = std::env::var("FV_MONGO_DB") {
            if !db.trim().is_empty() {
                cfg.database = db;
            }
        }
        if let Ok(coll) = std::env::var("FV_MONGO_MASTER_COLLECTION") {
            if !coll.trim().is_empty() {
                cfg.collection = coll;
            }
        }
        cfg
    }
}

/// Load the whole master and build the in-memory index.
pub async fn load_master(cfg: &MasterConfig) -> Result<MemoryMaster, MasterError> {
    let client = Client::with_uri_str(&cfg.uri)
        .await
        .map_err(|e| MasterError::Mongo(e.to_string()))?;
    // Shut the driver down explicitly: its background pool must not outlive the
    // runtime, and the master is fully materialised in memory by this point.
    let result = load_master_with_client(&client, cfg).await;
    client.shutdown().await;
    result
}

pub async fn load_master_with_client(
    client: &Client,
    cfg: &MasterConfig,
) -> Result<MemoryMaster, MasterError> {
    let collection = client
        .database(&cfg.database)
        .collection::<Document>(&cfg.collection);

    let projection = doc! {
        "_id": 0,
        "exchange": 1,
        "token": 1,
        "instrument_token": 1,
        "exchange_token": 1,
        "Symbol": 1,
        "tradingSymbol": 1,
        "tradingsymbol": 1,
        "underlying": 1,
        "name": 1,
        "expiry": 1,
        "strikePrice": 1,
        "strike": 1,
        "strikeType": 1,
        "instrumentType": 1,
        "instrument_type": 1,
        "lotSize": 1,
        "lot_size": 1,
        "segment": 1,
        "tick_size": 1,
    };

    let mut find = collection
        .find(doc! {})
        .projection(projection)
        .batch_size(5_000);
    if let Some(limit) = cfg.limit {
        find = find.limit(limit);
    }
    let mut cursor = find.await.map_err(|e| MasterError::Mongo(e.to_string()))?;

    let mut items = Vec::new();
    let mut skipped = 0usize;
    while let Some(document) = cursor
        .try_next()
        .await
        .map_err(|e| MasterError::Mongo(e.to_string()))?
    {
        match instrument_from_doc(&document) {
            Some(inst) => items.push(inst),
            None => skipped += 1,
        }
    }

    if skipped > 0 {
        tracing::warn!(
            skipped,
            loaded = items.len(),
            collection = %cfg.collection,
            "instruments skipped (missing exchange/token)"
        );
    }
    tracing::info!(
        loaded = items.len(),
        database = %cfg.database,
        collection = %cfg.collection,
        "instrument master loaded"
    );

    MemoryMaster::build(items)
}

/// Map one Mongo document to an [`Instrument`]. `None` when the identity fields
/// are unusable.
pub fn instrument_from_doc(doc: &Document) -> Option<Instrument> {
    let exchange = first_string(doc, &["exchange", "Exchange", "StockExchange"])?;
    let token = first_key_string(doc, &["token", "instrument_token", "exchange_token"])?;
    let key = SymbolKey::new(&exchange, &token).ok()?;

    let trading_symbol = first_string(doc, &["tradingSymbol", "tradingsymbol", "Symbol"])
        .unwrap_or_else(|| token.clone());
    let underlying = first_string(doc, &["underlying", "Symbol", "name"]);
    let name = first_string(doc, &["name", "CompanyName", "Symbol"]);
    let expiry = first_expiry(doc);
    let strike = first_price(doc, &["strikePrice", "strike"]).unwrap_or(Price::ZERO);
    let lot_size = first_i64(doc, &["lotSize", "lot_size"]).unwrap_or(0);
    let tick_size = first_price(doc, &["tick_size", "tickSize", "TickSize"]);
    let segment = first_string(doc, &["segment", "Segment", "InteropExchSeg"]);
    let instrument_type = first_string(
        doc,
        &["instrument_type", "instrumentType", "InstrumentName"],
    )
    .unwrap_or_default();

    let strike_type = first_string(doc, &["strikeType", "strike_type"])
        .filter(|s| !s.trim().is_empty())
        .map(|s| StrikeType::parse(&s))
        .or_else(|| {
            let t = StrikeType::parse(&instrument_type);
            if t == StrikeType::Other {
                None
            } else {
                Some(t)
            }
        })
        .unwrap_or(StrikeType::Other);

    Some(Instrument {
        key,
        trading_symbol,
        name,
        underlying,
        expiry,
        strike,
        strike_type,
        lot_size,
        instrument_type,
        segment,
        tick_size,
    })
}

fn first_string(doc: &Document, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(v) = doc.get(*key) {
            let s = match v {
                Bson::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
                Bson::Symbol(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
                _ => None,
            };
            if s.is_some() {
                return s;
            }
        }
    }
    None
}

/// Like [`first_string`] but also renders numeric BSON values (tokens are
/// Int32/Int64 in the Kite-shaped collection).
fn first_key_string(doc: &Document, keys: &[&str]) -> Option<String> {
    for key in keys {
        match doc.get(*key) {
            Some(Bson::String(s)) if !s.trim().is_empty() => return Some(s.trim().to_string()),
            Some(Bson::Int32(i)) => return Some(i.to_string()),
            Some(Bson::Int64(i)) => return Some(i.to_string()),
            Some(Bson::Double(f)) if f.fract() == 0.0 => return Some(format!("{}", *f as i64)),
            _ => {}
        }
    }
    None
}

fn first_i64(doc: &Document, keys: &[&str]) -> Option<i64> {
    for key in keys {
        match doc.get(*key) {
            Some(Bson::Int32(i)) => return Some(*i as i64),
            Some(Bson::Int64(i)) => return Some(*i),
            Some(Bson::Double(f)) => return Some(*f as i64),
            Some(Bson::String(s)) => {
                if let Ok(v) = s.trim().parse::<i64>() {
                    return Some(v);
                }
            }
            _ => {}
        }
    }
    None
}

fn first_price(doc: &Document, keys: &[&str]) -> Option<Price> {
    for key in keys {
        match doc.get(*key) {
            Some(Bson::Double(f)) => {
                if let Ok(p) = Price::parse_rounded(&format!("{f}")) {
                    return Some(p);
                }
            }
            Some(Bson::Int32(i)) => return Some(Price::from_scaled(*i as i64 * 1_000_000)),
            Some(Bson::Int64(i)) => return Some(Price::from_scaled(*i * 1_000_000)),
            Some(Bson::Decimal128(d)) => {
                if let Ok(p) = Price::parse_rounded(&d.to_string()) {
                    return Some(p);
                }
            }
            Some(Bson::String(s)) => {
                if let Ok(p) = Price::parse_rounded(s) {
                    return Some(p);
                }
            }
            _ => {}
        }
    }
    None
}

/// Expiry is `"2027-02-23"` in `FINVASIA_SYMBOL` and an ISODate in `INSTRUMENT`.
fn first_expiry(doc: &Document) -> Option<String> {
    match doc.get("expiry") {
        Some(Bson::String(s)) if !s.trim().is_empty() => {
            let s = s.trim();
            Some(s.chars().take(10).collect())
        }
        Some(Bson::DateTime(dt)) => dt
            .try_to_rfc3339_string()
            .ok()
            .map(|s| s.chars().take(10).collect()),
        Some(Bson::Null) | None => None,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_finvasia_symbol_shape() {
        let doc = doc! {
            "exchange": "NFO",
            "token": "158614",
            "underlying": "ZYDUSLIFE",
            "lotSize": 450_i32,
            "tradingSymbol": "ZYDUSLIFE23FEB27P1680",
            "expiry": "2027-02-23",
            "strikeType": "PE",
            "strikePrice": 1680.0_f64,
        };
        let inst = instrument_from_doc(&doc).expect("mapped");
        assert_eq!(inst.key.as_wire(), "NFO|158614");
        assert_eq!(inst.trading_symbol, "ZYDUSLIFE23FEB27P1680");
        assert_eq!(inst.lot_size, 450);
        assert_eq!(inst.strike.format(2), "1680.00");
        assert_eq!(inst.strike_type, StrikeType::Put);
        assert_eq!(inst.expiry.as_deref(), Some("2027-02-23"));
        assert!(inst.is_option());
    }

    #[test]
    fn maps_kite_instrument_shape() {
        let doc = doc! {
            "instrument_token": 282257669_i64,
            "exchange_token": 1102569_i64,
            "tradingsymbol": "BANKEX26NOVFUT",
            "name": "BANKEX",
            "instrument_type": "FUT",
            "segment": "BFO-FUT",
            "exchange": "BFO",
            "strike": 0_i32,
            "lot_size": 30_i32,
            "tick_size": 0.05_f64,
            "expiry": bson::DateTime::from_millis(1_795_651_200_000),
        };
        let inst = instrument_from_doc(&doc).expect("mapped");
        assert_eq!(inst.key.as_wire(), "BFO|282257669");
        assert_eq!(inst.trading_symbol, "BANKEX26NOVFUT");
        assert!(inst.is_future());
        assert_eq!(inst.lot_size, 30);
        assert_eq!(inst.tick_size.unwrap().format(2), "0.05");
        assert_eq!(inst.expiry.as_deref().map(|s| s.len()), Some(10));
    }

    #[test]
    fn rejects_documents_without_identity() {
        assert!(instrument_from_doc(&doc! { "name": "no exchange" }).is_none());
        assert!(instrument_from_doc(&doc! { "exchange": "NSE" }).is_none());
    }

    #[test]
    fn tolerates_missing_optional_fields() {
        let doc = doc! { "exchange": "NSE", "token": "1" };
        let inst = instrument_from_doc(&doc).expect("mapped");
        assert_eq!(inst.trading_symbol, "1");
        assert_eq!(inst.lot_size, 0);
        assert!(inst.expiry.is_none());
    }
}
