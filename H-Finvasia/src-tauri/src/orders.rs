//! Live order preparation and submission.
//!
//! The UI always builds a ticket first; nothing here sends a one-click order.
//! The only write route is `PlaceOrder`, and every request is validated before
//! it reaches the venue.
//!
//! Money is never held as `f64`: limit prices are parsed through [`Price`] and
//! formatted with the instrument's two-decimal convention before they are put
//! on the wire.

use crate::account;
use fv_core::Price;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderDefaults {
    /// Shoonya product code: I (intraday), C (delivery), M (margin).
    pub product: String,
    /// LMT, MKT (protected), SL-L or TSL (trailing SL-L).
    pub price_type: String,
    /// DAY or IOC.
    pub validity: String,
    pub lots: i64,
}

impl Default for OrderDefaults {
    fn default() -> Self {
        OrderDefaults {
            product: "I".into(),
            price_type: "LMT".into(),
            validity: "DAY".into(),
            lots: 1,
        }
    }
}

impl OrderDefaults {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.product.as_str(), "I" | "C" | "M") {
            return Err("product must be I, C or M".into());
        }
        if !matches!(self.price_type.as_str(), "LMT" | "MKT" | "SL-L" | "TSL") {
            return Err("price type must be LMT, MKT, SL-L or TSL".into());
        }
        if !matches!(self.validity.as_str(), "DAY" | "IOC") {
            return Err("validity must be DAY or IOC".into());
        }
        if !(1..=10_000).contains(&self.lots) {
            return Err("lots must be between 1 and 10,000".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaceOrderRequest {
    /// `EXCH|token` as used everywhere else in the app.
    pub key: String,
    /// B or S.
    pub side: String,
    pub lots: i64,
    pub product: String,
    pub price_type: String,
    pub price: Option<String>,
    /// Required for SL-L and TSL.
    #[serde(default)]
    pub trigger_price: Option<String>,
    /// TSL trail distance from the LTP, required for TSL.
    #[serde(default)]
    pub trail: Option<String>,
    pub validity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MarginDto {
    /// Margin blocked for this order (`marginused`).
    pub required: String,
    /// Post-trade account margin (`marginusedtrade`), when supplied.
    pub after_trade: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SquareOffRequest {
    pub key: String,
    pub side: String,
    pub quantity: i64,
    pub product: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OrderResult {
    pub order_no: String,
    pub status: String,
}

pub fn file_path(config_dir: &Path) -> PathBuf {
    config_dir.join("order-defaults.json")
}

pub fn read_defaults(path: &Path) -> OrderDefaults {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<OrderDefaults>(&text).ok())
        .filter(|defaults| defaults.validate().is_ok())
        .unwrap_or_default()
}

pub fn write_defaults(path: &Path, defaults: &OrderDefaults) -> Result<(), String> {
    defaults.validate()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(defaults).map_err(|e| format!("encode: {e}"))?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, text).map_err(|e| format!("write {}: {e}", temp.display()))?;
    std::fs::rename(&temp, path).map_err(|e| format!("rename into {}: {e}", path.display()))
}

/// Unique per-process remarks. Shoonya has no client order id, so the remarks
/// are the reconciliation tag recommended by the venue.
static ORDER_SEQ: AtomicU64 = AtomicU64::new(0);

fn unique_remark() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let seq = ORDER_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("hfv-{millis}-{seq}")
}

fn parse_positive(raw: Option<&str>, what: &str) -> Result<Price, String> {
    let text = raw
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{what} is required"))?;
    let value = Price::parse_rounded(text).map_err(|_| format!("{what} is not valid"))?;
    if value.scaled() <= 0 {
        return Err(format!("{what} must be greater than zero"));
    }
    Ok(value)
}

/// Market orders are converted to a marketable limit because Shoonya blocks
/// raw MKT on API orders. The extra 1% (minimum one tick) makes the order cross.
fn protected_market_price(base: Price, side: &str) -> Result<Price, String> {
    let buffer = Price::from_scaled((base.scaled().abs() / 100).max(50_000));
    let price = if side == "B" {
        base.checked_add(buffer)
    } else {
        base.checked_sub(buffer)
    }
    .ok_or_else(|| "protected market price is out of range".to_string())?;
    if price.scaled() <= 0 {
        return Err("protected market price must be greater than zero".into());
    }
    Ok(price)
}

fn resolve_price(
    request: &PlaceOrderRequest,
    side: &str,
) -> Result<(&'static str, Price, Option<Price>), String> {
    let requested = request.price_type.trim().to_ascii_uppercase();
    match requested.as_str() {
        "LMT" => Ok((
            "LMT",
            parse_positive(request.price.as_deref(), "limit price")?,
            None,
        )),
        "MKT" => {
            let base = parse_positive(request.price.as_deref(), "market reference price")?;
            Ok(("LMT", protected_market_price(base, side)?, None))
        }
        "SL-L" => {
            let limit = parse_positive(request.price.as_deref(), "limit price")?;
            let trigger = parse_positive(request.trigger_price.as_deref(), "trigger price")?;
            Ok(("SL-L", limit, Some(trigger)))
        }
        "TSL" => {
            let limit = parse_positive(request.price.as_deref(), "limit price")?;
            let trigger = parse_positive(request.trigger_price.as_deref(), "trigger price")?;
            parse_positive(request.trail.as_deref(), "trail amount")?;
            Ok(("SL-L", limit, Some(trigger)))
        }
        _ => Err("price type must be LMT, MKT, SL-L or TSL".into()),
    }
}

fn validate_common(
    request: &PlaceOrderRequest,
    uid: &str,
    actid: &str,
    exchange: &str,
    trading_symbol: &str,
) -> Result<(String, String, String), String> {
    if uid.trim().is_empty() || actid.trim().is_empty() {
        return Err("session is missing uid/actid".into());
    }
    if exchange.trim().is_empty() || trading_symbol.trim().is_empty() {
        return Err("instrument metadata is incomplete".into());
    }
    let side = request.side.trim().to_ascii_uppercase();
    if !matches!(side.as_str(), "B" | "S") {
        return Err("side must be B or S".into());
    }
    let product = request.product.trim().to_ascii_uppercase();
    if !matches!(product.as_str(), "I" | "C" | "M") {
        return Err("product must be I, C or M".into());
    }
    let validity = request.validity.trim().to_ascii_uppercase();
    if !matches!(validity.as_str(), "DAY" | "IOC") {
        return Err("validity must be DAY or IOC".into());
    }
    Ok((side, product, validity))
}

/// Validate a ticket and turn it into the raw `jData` object sent to
/// `PlaceOrder`. `lot_size` comes from the local instrument master.
pub fn build_payload(
    request: &PlaceOrderRequest,
    uid: &str,
    actid: &str,
    exchange: &str,
    trading_symbol: &str,
    lot_size: i64,
) -> Result<String, String> {
    if lot_size <= 0 {
        return Err("instrument lot size is invalid".into());
    }
    if !(1..=10_000).contains(&request.lots) {
        return Err("lots must be between 1 and 10,000".into());
    }
    let qty = request
        .lots
        .checked_mul(lot_size)
        .filter(|qty| *qty > 0 && *qty <= 1_000_000_000)
        .ok_or_else(|| "quantity is out of range".to_string())?;
    build_payload_for_qty(request, uid, actid, exchange, trading_symbol, qty)
}

/// Same validation as [`build_payload`] but with an explicit share quantity.
/// Used by square-off, where the position already carries the exact quantity.
pub fn build_payload_for_qty(
    request: &PlaceOrderRequest,
    uid: &str,
    actid: &str,
    exchange: &str,
    trading_symbol: &str,
    qty: i64,
) -> Result<String, String> {
    if !(1..=1_000_000_000).contains(&qty) {
        return Err("quantity is out of range".into());
    }
    let (side, product, validity) = validate_common(request, uid, actid, exchange, trading_symbol)?;
    let (price_type, price, trigger) = resolve_price(request, &side)?;

    let remarks = unique_remark();
    Ok(json!({
        "uid": uid,
        "actid": actid,
        "exch": exchange,
        "tsym": trading_symbol,
        "qty": qty.to_string(),
        "prc": price.format(2),
        "trgprc": trigger.map(|value| value.format(2)).unwrap_or_else(|| "0".into()),
        "dscqty": "0",
        "prd": product,
        "trantype": side,
        "prctyp": price_type,
        "ret": validity,
        "ordersource": "API",
        "remarks": remarks,
    })
    .to_string())
}

/// Margin-calculator payload for `GetBasketMargin`. Shoonya rejects raw
/// MKT/SL-M there too, so the same protected LMT/SL-L conversion is used.
pub fn build_margin_payload(
    request: &PlaceOrderRequest,
    uid: &str,
    actid: &str,
    exchange: &str,
    trading_symbol: &str,
    lot_size: i64,
) -> Result<String, String> {
    if lot_size <= 0 {
        return Err("instrument lot size is invalid".into());
    }
    if !(1..=10_000).contains(&request.lots) {
        return Err("lots must be between 1 and 10,000".into());
    }
    let qty = request
        .lots
        .checked_mul(lot_size)
        .filter(|qty| *qty > 0 && *qty <= 1_000_000_000)
        .ok_or_else(|| "quantity is out of range".to_string())?;
    let (side, product, _validity) =
        validate_common(request, uid, actid, exchange, trading_symbol)?;
    let (price_type, price, trigger) = resolve_price(request, &side)?;
    Ok(json!({
        "uid": uid,
        "actid": actid,
        "exch": exchange,
        "tsym": trading_symbol,
        "qty": qty.to_string(),
        "prc": price.format(2),
        "trgprc": trigger.map(|value| value.format(2)).unwrap_or_else(|| "0".into()),
        "prd": product,
        "trantype": side,
        "prctyp": price_type,
    })
    .to_string())
}

/// Build a `ModifyOrder` payload for the trailing-stop monitor.
pub fn build_modify_payload(
    order_no: &str,
    uid: &str,
    exchange: &str,
    trading_symbol: &str,
    qty: i64,
    price: Price,
    trigger: Price,
) -> String {
    json!({
        "uid": uid,
        "norenordno": order_no,
        "exch": exchange,
        "tsym": trading_symbol,
        "qty": qty.to_string(),
        "prc": price.format(2),
        "trgprc": trigger.format(2),
        "prctyp": "SL-L",
        "ret": "DAY",
        "dscqty": "0",
    })
    .to_string()
}

pub fn parse_place_response(value: &Value) -> Result<OrderResult, String> {
    let stat = value.get("stat").and_then(Value::as_str).unwrap_or("");
    if !stat.eq_ignore_ascii_case("Ok") {
        let message = value
            .get("emsg")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .unwrap_or("order rejected by the broker");
        return Err(message.to_string());
    }
    let order_no = value
        .get("norenordno")
        .and_then(|v| match v {
            Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
        .ok_or_else(|| "broker accepted the order but returned no order number".to_string())?;
    Ok(OrderResult {
        order_no,
        status: "submitted".into(),
    })
}

fn price_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|raw| match raw {
        Value::String(s) if !s.trim().is_empty() => Price::parse_rounded(s.trim())
            .ok()
            .map(|price| price.format(2)),
        Value::Number(n) => Price::parse_rounded(&n.to_string())
            .ok()
            .map(|price| price.format(2)),
        _ => None,
    })
}

pub fn parse_margin_response(value: &Value) -> Result<MarginDto, String> {
    let stat = value.get("stat").and_then(Value::as_str).unwrap_or("");
    if !stat.eq_ignore_ascii_case("Ok") {
        let message = value
            .get("emsg")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .unwrap_or("margin calculation rejected by the broker");
        return Err(message.to_string());
    }
    let required = price_field(value, "marginused")
        .or_else(|| price_field(value, "marginusedtrade"))
        .ok_or_else(|| "broker returned no margin amount".to_string())?;
    Ok(MarginDto {
        required,
        after_trade: price_field(value, "marginusedtrade"),
    })
}

pub fn parse_ok_response(value: &Value) -> Result<(), String> {
    let stat = value.get("stat").and_then(Value::as_str).unwrap_or("");
    if stat.eq_ignore_ascii_case("Ok") {
        Ok(())
    } else {
        let message = value
            .get("emsg")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .unwrap_or("broker rejected the request");
        Err(message.to_string())
    }
}

pub async fn submit(
    client: &account::RestClient,
    token: &str,
    payload: &str,
) -> Result<OrderResult, String> {
    let value = client.post_write("PlaceOrder", payload, token).await?;
    parse_place_response(&value)
}

pub async fn modify(
    client: &account::RestClient,
    token: &str,
    payload: &str,
) -> Result<(), String> {
    let value = client.post_write("ModifyOrder", payload, token).await?;
    parse_ok_response(&value)
}

pub async fn margin(
    client: &account::RestClient,
    token: &str,
    payload: &str,
) -> Result<MarginDto, String> {
    let value = client.post_write("GetBasketMargin", payload, token).await?;
    parse_margin_response(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> PlaceOrderRequest {
        PlaceOrderRequest {
            key: "NFO|51001".into(),
            side: "B".into(),
            lots: 2,
            product: "I".into(),
            price_type: "LMT".into(),
            price: Some("88.95".into()),
            trigger_price: None,
            trail: None,
            validity: "DAY".into(),
        }
    }

    #[test]
    fn defaults_are_valid_and_round_trip() {
        let defaults = OrderDefaults::default();
        defaults.validate().unwrap();
        let path = std::env::temp_dir().join(format!(
            "fv-order-defaults-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let mut changed = defaults.clone();
        changed.product = "M".into();
        changed.lots = 3;
        write_defaults(&path, &changed).unwrap();
        assert_eq!(read_defaults(&path), changed);
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(read_defaults(&path), OrderDefaults::default());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn payload_carries_side_quantity_and_price() {
        let payload = build_payload(&request(), "FN1", "FN1", "NFO", "NIFTY01DEC26C25900", 65)
            .expect("valid");
        let value: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(value["uid"], "FN1");
        assert_eq!(value["actid"], "FN1");
        assert_eq!(value["exch"], "NFO");
        assert_eq!(value["tsym"], "NIFTY01DEC26C25900");
        assert_eq!(value["qty"], "130");
        assert_eq!(value["prc"], "88.95");
        assert_eq!(value["trantype"], "B");
        assert_eq!(value["prctyp"], "LMT");
        assert_eq!(value["ret"], "DAY");
        assert_eq!(value["prd"], "I");
        assert_eq!(value["ordersource"], "API");
        assert!(value["remarks"].as_str().unwrap().starts_with("hfv-"));
    }

    #[test]
    fn market_orders_are_converted_to_a_protected_limit() {
        let mut req = request();
        req.price_type = "MKT".into();
        req.price = Some("100.00".into());
        let payload = build_payload(&req, "u", "a", "NFO", "X", 1).expect("protected market");
        let value: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(
            value["prctyp"], "LMT",
            "Shoonya blocks raw MKT on API orders"
        );
        assert_eq!(value["prc"], "101.00");
        assert_eq!(value["trgprc"], "0");
    }

    #[test]
    fn stop_loss_and_trailing_orders_carry_a_trigger() {
        let mut req = request();
        req.side = "S".into();
        req.price_type = "SL-L".into();
        req.price = Some("98.00".into());
        req.trigger_price = Some("99.00".into());
        let payload = build_payload(&req, "u", "a", "NFO", "X", 1).unwrap();
        let value: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(value["prctyp"], "SL-L");
        assert_eq!(value["prc"], "98.00");
        assert_eq!(value["trgprc"], "99.00");

        req.price_type = "TSL".into();
        req.trail = Some("2.00".into());
        let payload = build_payload(&req, "u", "a", "NFO", "X", 1).unwrap();
        let value: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(value["prctyp"], "SL-L");
        assert_eq!(value["trgprc"], "99.00");

        req.trail = None;
        assert!(build_payload(&req, "u", "a", "NFO", "X", 1).is_err());
        req.trail = Some("2".into());
        req.trigger_price = None;
        assert!(build_payload(&req, "u", "a", "NFO", "X", 1).is_err());
    }

    #[test]
    fn margin_payload_uses_the_same_protected_conversion() {
        let mut req = request();
        req.price_type = "MKT".into();
        req.price = Some("100.00".into());
        let payload = build_margin_payload(&req, "u", "a", "NFO", "X", 65).unwrap();
        let value: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(value["qty"], "130");
        assert_eq!(value["prctyp"], "LMT");
        assert_eq!(value["prc"], "101.00");

        let margin = parse_margin_response(&serde_json::json!({
            "stat": "Ok",
            "marginused": "12345.60",
            "marginusedtrade": "22345.60"
        }))
        .unwrap();
        assert_eq!(margin.required, "12345.60");
        assert_eq!(margin.after_trade.as_deref(), Some("22345.60"));
        assert!(
            parse_margin_response(&serde_json::json!({"stat":"Not_Ok","emsg":"no margin"}))
                .is_err()
        );
    }

    #[test]
    fn modify_payload_carries_the_order_number_and_new_trigger() {
        let payload = build_modify_payload(
            "24121500001234",
            "u",
            "NFO",
            "X",
            65,
            Price::parse("99.00").unwrap(),
            Price::parse("100.00").unwrap(),
        );
        let value: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(value["norenordno"], "24121500001234");
        assert_eq!(value["prctyp"], "SL-L");
        assert_eq!(value["prc"], "99.00");
        assert_eq!(value["trgprc"], "100.00");
        assert!(parse_ok_response(&serde_json::json!({"stat":"Ok"})).is_ok());
    }

    #[test]
    fn bad_tickets_are_rejected_before_the_wire() {
        assert!(build_payload(&request(), "", "a", "NFO", "X", 1).is_err());
        assert!(build_payload(&request(), "u", "a", "NFO", "X", 0).is_err());

        let mut req = request();
        req.side = "X".into();
        assert!(build_payload(&req, "u", "a", "NFO", "X", 1).is_err());

        let mut req = request();
        req.price = Some("0".into());
        assert!(build_payload(&req, "u", "a", "NFO", "X", 1).is_err());

        let mut req = request();
        req.price = None;
        assert!(build_payload(&req, "u", "a", "NFO", "X", 1).is_err());

        let mut req = request();
        req.lots = 0;
        assert!(build_payload(&req, "u", "a", "NFO", "X", 65).is_err());
    }

    #[test]
    fn parses_ok_and_rejection() {
        let ok = serde_json::json!({"stat":"Ok","norenordno":"24121500001234"});
        let result = parse_place_response(&ok).unwrap();
        assert_eq!(result.order_no, "24121500001234");

        let bad = serde_json::json!({"stat":"Not_Ok","emsg":"RED:Margin Shortfall"});
        assert_eq!(
            parse_place_response(&bad).unwrap_err(),
            "RED:Margin Shortfall"
        );

        let no_number = serde_json::json!({"stat":"Ok"});
        assert!(parse_place_response(&no_number).is_err());
    }
}
