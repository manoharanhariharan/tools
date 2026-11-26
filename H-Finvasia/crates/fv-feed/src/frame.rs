//! Frame decoding.
//!
//! The feed is JSON with two-character keys. Numbers arrive as *strings*
//! (`"1287.30"`), and occasionally as JSON numbers, so every numeric field uses
//! a lenient deserializer.

use crate::FeedError;
use fv_core::{Price, SymbolKey};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// Coarse frame classification, derived without allocating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    Snapshot,
    Diff,
    OrderUpdate,
    ConnectAck,
    UnsubAck,
    /// A control frame we send or that the venue echoes.
    Request,
    Unknown,
}

/// Locate the `"t"` value without building a `serde_json::Value`.
///
/// This is the fast path described in `fv-features.md` §7.6: routing an `om`
/// frame must not require touching any market-data field, otherwise a tick burst
/// delays order state.
pub fn peek_type(raw: &[u8]) -> Option<&[u8]> {
    let limit = raw.len().min(96);
    let mut i = 0;
    while i + 3 < limit {
        if raw[i] == b'"' && raw[i + 1] == b't' && raw[i + 2] == b'"' {
            let mut j = i + 3;
            while j < limit && raw[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < limit && raw[j] == b':' {
                j += 1;
                while j < limit && raw[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < limit && raw[j] == b'"' {
                    let start = j + 1;
                    let mut end = start;
                    while end < limit && raw[end] != b'"' {
                        end += 1;
                    }
                    if end < limit && end > start {
                        return Some(&raw[start..end]);
                    }
                }
            }
        }
        i += 1;
    }
    None
}

/// Classify a frame by its `t` value. Allocation-free.
pub fn peek_kind(raw: &[u8]) -> FrameKind {
    match peek_type(raw) {
        Some(b"tk") => FrameKind::Snapshot,
        Some(b"tf") => FrameKind::Diff,
        Some(b"om") => FrameKind::OrderUpdate,
        Some(b"ak") | Some(b"ck") => FrameKind::ConnectAck,
        Some(b"uk") => FrameKind::UnsubAck,
        Some(b"a") | Some(b"t") | Some(b"u") => FrameKind::Request,
        _ => FrameKind::Unknown,
    }
}

/// A decoded frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    /// `t:"tk"` — full snapshot for one instrument.
    Snapshot(Touchline),
    /// `t:"tf"` — partial update; merge, do not replace.
    Diff(Touchline),
    /// `t:"om"` — order lifecycle event.
    Order(OrderUpdate),
    /// Handshake / unsubscribe acknowledgement.
    Ack(Ack),
    /// Any other `t` value, preserved for observability.
    Unknown { kind: String },
}

impl Frame {
    pub fn kind(&self) -> FrameKind {
        match self {
            Frame::Snapshot(_) => FrameKind::Snapshot,
            Frame::Diff(_) => FrameKind::Diff,
            Frame::Order(_) => FrameKind::OrderUpdate,
            Frame::Ack(_) => FrameKind::UnsubAck,
            Frame::Unknown { .. } => FrameKind::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ack {
    pub kind: String,
    pub status: Option<String>,
    pub keys: Option<String>,
}

impl Ack {
    /// The venue returns `s:"Ok"` for connect acks and `s:"OK"` in some
    /// socket-open variants; accept both, case-insensitively.
    pub fn is_ok(&self) -> bool {
        self.status
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("ok"))
            .unwrap_or(false)
    }
}

/// Touchline snapshot or diff. Every field is optional because a diff carries
/// only what changed and a snapshot omits fields that do not apply to the
/// instrument type.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct Touchline {
    #[serde(rename = "t", default)]
    pub kind: String,
    #[serde(rename = "e", default, deserialize_with = "de::opt_string")]
    pub exchange: Option<String>,
    #[serde(rename = "tk", default, deserialize_with = "de::opt_string")]
    pub token: Option<String>,

    #[serde(rename = "ts", default, deserialize_with = "de::opt_string")]
    pub trading_symbol: Option<String>,
    #[serde(rename = "pp", default, deserialize_with = "de::opt_u8")]
    pub price_precision: Option<u8>,
    #[serde(rename = "ti", default, deserialize_with = "de::opt_price")]
    pub tick_size: Option<Price>,
    #[serde(rename = "ls", default, deserialize_with = "de::opt_i64")]
    pub lot_size: Option<i64>,

    #[serde(rename = "lp", default, deserialize_with = "de::opt_price")]
    pub ltp: Option<Price>,
    #[serde(rename = "pc", default, deserialize_with = "de::opt_price")]
    pub pct_change: Option<Price>,
    #[serde(rename = "o", default, deserialize_with = "de::opt_price")]
    pub open: Option<Price>,
    #[serde(rename = "h", default, deserialize_with = "de::opt_price")]
    pub high: Option<Price>,
    #[serde(rename = "l", default, deserialize_with = "de::opt_price")]
    pub low: Option<Price>,
    #[serde(rename = "c", default, deserialize_with = "de::opt_price")]
    pub close: Option<Price>,
    #[serde(rename = "ap", default, deserialize_with = "de::opt_price")]
    pub avg_price: Option<Price>,

    #[serde(rename = "v", default, deserialize_with = "de::opt_i64")]
    pub volume: Option<i64>,
    #[serde(rename = "oi", default, deserialize_with = "de::opt_i64")]
    pub oi: Option<i64>,
    #[serde(rename = "poi", default, deserialize_with = "de::opt_i64")]
    pub prev_oi: Option<i64>,
    #[serde(rename = "toi", default, deserialize_with = "de::opt_i64")]
    pub total_oi: Option<i64>,

    #[serde(rename = "bq1", default, deserialize_with = "de::opt_i64")]
    pub best_bid_qty: Option<i64>,
    #[serde(rename = "bp1", default, deserialize_with = "de::opt_price")]
    pub best_bid: Option<Price>,
    #[serde(rename = "sq1", default, deserialize_with = "de::opt_i64")]
    pub best_ask_qty: Option<i64>,
    #[serde(rename = "sp1", default, deserialize_with = "de::opt_price")]
    pub best_ask: Option<Price>,

    #[serde(rename = "ft", default, deserialize_with = "de::opt_i64")]
    pub feed_time: Option<i64>,
}

impl Touchline {
    /// Resolve the instrument this frame refers to.
    pub fn symbol(&self) -> Option<SymbolKey> {
        match (self.exchange.as_deref(), self.token.as_deref()) {
            (Some(e), Some(t)) => SymbolKey::new(e, t).ok(),
            _ => None,
        }
    }
}

/// `t:"om"` order lifecycle event.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct OrderUpdate {
    #[serde(rename = "t", default)]
    pub kind: String,
    #[serde(rename = "norenordno", default, deserialize_with = "de::opt_string")]
    pub noren_order_no: Option<String>,
    #[serde(rename = "uid", default, deserialize_with = "de::opt_string")]
    pub user_id: Option<String>,
    #[serde(rename = "actid", default, deserialize_with = "de::opt_string")]
    pub account_id: Option<String>,
    #[serde(rename = "exch", default, deserialize_with = "de::opt_string")]
    pub exchange: Option<String>,
    #[serde(rename = "tsym", default, deserialize_with = "de::opt_string")]
    pub trading_symbol: Option<String>,

    #[serde(rename = "qty", default, deserialize_with = "de::opt_i64")]
    pub quantity: Option<i64>,
    #[serde(rename = "prc", default, deserialize_with = "de::opt_price")]
    pub price: Option<Price>,
    #[serde(rename = "pcode", default, deserialize_with = "de::opt_string")]
    pub product: Option<String>,

    #[serde(rename = "status", default, deserialize_with = "de::opt_string")]
    pub status: Option<String>,
    #[serde(rename = "reporttype", default, deserialize_with = "de::opt_string")]
    pub report_type: Option<String>,
    #[serde(rename = "trantype", default, deserialize_with = "de::opt_string")]
    pub transaction_type: Option<String>,
    #[serde(rename = "prctyp", default, deserialize_with = "de::opt_string")]
    pub price_type: Option<String>,
    #[serde(rename = "ret", default, deserialize_with = "de::opt_string")]
    pub retention: Option<String>,

    #[serde(rename = "fillshares", default, deserialize_with = "de::opt_i64")]
    pub filled_quantity: Option<i64>,
    #[serde(rename = "avgprc", default, deserialize_with = "de::opt_price")]
    pub average_price: Option<Price>,

    #[serde(rename = "fltm", default, deserialize_with = "de::opt_string")]
    pub fill_time: Option<String>,
    #[serde(rename = "flid", default, deserialize_with = "de::opt_string")]
    pub fill_id: Option<String>,
    #[serde(rename = "flqty", default, deserialize_with = "de::opt_i64")]
    pub fill_quantity: Option<i64>,
    #[serde(rename = "flprc", default, deserialize_with = "de::opt_price")]
    pub fill_price: Option<Price>,

    #[serde(rename = "exchordid", default, deserialize_with = "de::opt_string")]
    pub exchange_order_id: Option<String>,
    #[serde(rename = "remarks", default, deserialize_with = "de::opt_string")]
    pub remarks: Option<String>,
}

impl OrderUpdate {
    /// Open quantity, when both legs are known.
    pub fn open_quantity(&self) -> Option<i64> {
        match (self.quantity, self.filled_quantity) {
            (Some(q), Some(f)) => Some(q - f),
            (Some(q), None) => Some(q),
            _ => None,
        }
    }
}

#[derive(Deserialize)]
struct RawAck {
    #[serde(rename = "t", default)]
    t: String,
    #[serde(rename = "s", default)]
    s: Option<String>,
    #[serde(rename = "k", default)]
    k: Option<String>,
}

/// Decode one frame.
pub fn parse(raw: &[u8]) -> Result<Frame, FeedError> {
    match peek_kind(raw) {
        FrameKind::Snapshot => Ok(Frame::Snapshot(serde_json::from_slice(raw)?)),
        FrameKind::Diff => Ok(Frame::Diff(serde_json::from_slice(raw)?)),
        FrameKind::OrderUpdate => Ok(Frame::Order(serde_json::from_slice(raw)?)),
        FrameKind::ConnectAck | FrameKind::UnsubAck | FrameKind::Request => {
            let a: RawAck = serde_json::from_slice(raw)?;
            Ok(Frame::Ack(Ack {
                kind: a.t,
                status: a.s,
                keys: a.k,
            }))
        }
        FrameKind::Unknown => {
            // Validate well-formedness: corruption must not be mistaken for a
            // new frame type. `IgnoredAny` validates without materialising.
            serde_json::from_slice::<serde::de::IgnoredAny>(raw)?;
            let kind = peek_type(raw)
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_default();
            Ok(Frame::Unknown { kind })
        }
    }
}

mod de {
    use super::*;

    pub(super) fn opt_price<'de, D>(d: D) -> Result<Option<Price>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = Option::<Value>::deserialize(d)?;
        Ok(v.as_ref().and_then(price_from_value))
    }

    fn price_from_value(v: &Value) -> Option<Price> {
        match v {
            Value::String(s) => Price::parse_rounded(s.trim()).ok(),
            Value::Number(n) => Price::parse_rounded(&n.to_string()).ok(),
            _ => None,
        }
    }

    pub(super) fn opt_i64<'de, D>(d: D) -> Result<Option<i64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = Option::<Value>::deserialize(d)?;
        Ok(v.as_ref().and_then(|v| match v {
            Value::String(s) => s.trim().parse::<i64>().ok(),
            Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
            _ => None,
        }))
    }

    pub(super) fn opt_u8<'de, D>(d: D) -> Result<Option<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = Option::<Value>::deserialize(d)?;
        Ok(v.as_ref().and_then(|v| match v {
            Value::String(s) => s.trim().parse::<u8>().ok(),
            Value::Number(n) => n.as_u64().map(|n| n.min(255) as u8),
            _ => None,
        }))
    }

    pub(super) fn opt_string<'de, D>(d: D) -> Result<Option<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = Option::<Value>::deserialize(d)?;
        Ok(v.as_ref().and_then(string_from_value))
    }

    fn string_from_value(v: &Value) -> Option<String> {
        match v {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peek_is_allocation_free_and_finds_type() {
        let raw = br#"{"t":"tk","e":"NSE","tk":"22"}"#;
        assert_eq!(peek_type(raw), Some(&b"tk"[..]));
        assert_eq!(peek_kind(raw), FrameKind::Snapshot);
    }

    #[test]
    fn peek_tolerates_key_order_and_whitespace() {
        let raw = br#"{ "e" : "NSE" , "t" : "tf" , "tk" : "22" }"#;
        assert_eq!(peek_kind(raw), FrameKind::Diff);
    }

    #[test]
    fn peek_finds_type_beyond_short_prefix() {
        let raw = br#"{"a":"0123456789012345678901234567890123456789","t":"om","norenordno":"1"}"#;
        assert_eq!(peek_kind(raw), FrameKind::OrderUpdate);
    }

    #[test]
    fn snapshot_decodes_all_documented_fields() {
        let raw = br#"{"t":"tk","e":"NSE","tk":"22","ts":"ACC-EQ","pp":"2","ti":"0.05","ls":"1",
            "lp":"1287.30","pc":"0.45","o":"1280.00","h":"1291.80","l":"1278.50","c":"1275.20",
            "ap":"1286.10","v":"980515","oi":"0","poi":"0","toi":"0",
            "bq1":"26","bp1":"1287.25","sq1":"6325","sp1":"1287.35","ft":"1670231374"}"#;
        let Frame::Snapshot(t) = parse(raw).unwrap() else {
            panic!("expected snapshot")
        };
        assert_eq!(t.symbol().unwrap().as_wire(), "NSE|22");
        assert_eq!(t.ltp.unwrap().format(2), "1287.30");
        assert_eq!(t.price_precision, Some(2));
        assert_eq!(t.tick_size.unwrap().format(2), "0.05");
        assert_eq!(t.volume, Some(980_515));
        assert_eq!(t.best_bid.unwrap().format(2), "1287.25");
        assert_eq!(t.best_ask_qty, Some(6325));
        assert_eq!(t.feed_time, Some(1_670_231_374));
    }

    #[test]
    fn diff_decodes_with_only_changed_fields() {
        let raw = br#"{"t":"tf","e":"NSE","tk":"11630","lp":"118.45","ft":"1670231245"}"#;
        let Frame::Diff(t) = parse(raw).unwrap() else {
            panic!("expected diff")
        };
        assert_eq!(t.ltp.unwrap().format(2), "118.45");
        assert!(t.open.is_none());
        assert!(t.volume.is_none());
    }

    #[test]
    fn numeric_json_values_are_accepted() {
        let raw = br#"{"t":"tf","e":"NSE","tk":11630,"lp":118.45,"v":166637}"#;
        let Frame::Diff(t) = parse(raw).unwrap() else {
            panic!("expected diff")
        };
        assert_eq!(t.token.as_deref(), Some("11630"));
        assert_eq!(t.ltp.unwrap().format(2), "118.45");
        assert_eq!(t.volume, Some(166_637));
    }

    #[test]
    fn order_update_decodes() {
        let raw = br#"{"t":"om","norenordno":"24121500001234","uid":"AB1234","actid":"AB1234",
            "exch":"NSE","tsym":"RELIANCE-EQ","qty":"10","prc":"1272.30","pcode":"C",
            "status":"COMPLETE","reporttype":"Fill","trantype":"B","prctyp":"LMT","ret":"DAY",
            "fillshares":"4","avgprc":"1272.30","fltm":"09:20:41 15-12-2024","flid":"8841",
            "flqty":"4","flprc":"1272.30","exchordid":"250620000000343421","remarks":"my_order_001"}"#;
        let Frame::Order(o) = parse(raw).unwrap() else {
            panic!("expected order")
        };
        assert_eq!(o.noren_order_no.as_deref(), Some("24121500001234"));
        assert_eq!(o.status.as_deref(), Some("COMPLETE"));
        assert_eq!(o.report_type.as_deref(), Some("Fill"));
        assert_eq!(o.price.unwrap().format(2), "1272.30");
        assert_eq!(o.open_quantity(), Some(6));
    }

    #[test]
    fn connect_ack_accepts_ok_in_any_case() {
        for raw in [
            &br#"{"t":"ak","uid":"AB1234","s":"Ok"}"#[..],
            &br#"{"t":"ck","s":"OK"}"#[..],
        ] {
            let Frame::Ack(a) = parse(raw).unwrap() else {
                panic!("expected ack")
            };
            assert!(a.is_ok());
        }
    }

    #[test]
    fn connect_ack_detects_failure() {
        let Frame::Ack(a) = parse(br#"{"t":"ak","s":"Not_Ok"}"#).unwrap() else {
            panic!("expected ack")
        };
        assert!(!a.is_ok());
    }

    #[test]
    fn unknown_frames_are_preserved_not_dropped() {
        let Frame::Unknown { kind } = parse(br#"{"t":"zz","x":"1"}"#).unwrap() else {
            panic!("expected unknown")
        };
        assert_eq!(kind, "zz");
    }

    #[test]
    fn malformed_json_is_an_error_not_a_panic() {
        assert!(parse(b"{not json").is_err());
        assert!(parse(b"").is_err());
    }
}
