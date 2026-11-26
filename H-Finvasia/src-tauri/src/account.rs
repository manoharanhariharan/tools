//! Read-only account data over REST.
//!
//! The persisted session is **web-scoped**, so account endpoints answer on
//! `NorenWClientWeb` while `NorenWClientAPI` returns 401 for them. The client
//! therefore tries the preferred base and transparently falls back to the other
//! one on a session error, then remembers which worked.
//!
//! Nothing here writes: positions, funds and the order book are read-only.

use fv_core::Price;
use serde::Serialize;
use serde_json::Value;
use std::sync::atomic::{AtomicU8, Ordering};

pub const WEB_BASE: &str = "https://trade.shoonya.com/NorenWClientWeb";
pub const API_BASE: &str = "https://api.shoonya.com/NorenWClientAPI";

/// Which base last worked: 0 unknown, 1 web, 2 api.
static PREFERRED: AtomicU8 = AtomicU8::new(0);

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct FundsDto {
    /// Profile name from `Limits` (e.g. SHOONYA).
    pub profile: Option<String>,
    pub cash: Option<String>,
    pub payin: Option<String>,
    pub payout: Option<String>,
    pub uncleared: Option<String>,
    pub blocked: Option<String>,
    pub turnover: Option<String>,
    pub turnover_limit: Option<String>,
    pub pending_order_value: Option<String>,
    pub brokerage: Option<String>,
    pub peak_margin: Option<String>,
    pub premium: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PositionDto {
    pub exchange: String,
    pub symbol: String,
    pub token: String,
    pub product: String,
    pub product_alias: String,
    pub instrument: String,
    pub lot_size: i64,
    pub net_qty: i64,
    pub net_avg: Option<String>,
    pub ltp: Option<String>,
    pub mtm: Option<String>,
    pub pnl: Option<String>,
    pub day_buy_qty: i64,
    pub day_sell_qty: i64,
    pub buy_value: Option<String>,
    pub sell_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct PositionTotals {
    pub buy_qty: i64,
    pub sell_qty: i64,
    pub buy_value: String,
    pub sell_value: String,
    pub net_value: String,
    pub mtm: String,
    /// Realised P&L (sum of `rpnl`).
    pub rpnl: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PositionsView {
    pub funds: Option<FundsDto>,
    pub positions: Vec<PositionDto>,
    pub totals: PositionTotals,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OrderDto {
    pub order_no: String,
    pub status: String,
    pub side: String,
    pub price_type: String,
    pub retention: String,
    pub quantity: i64,
    pub price: Option<String>,
    pub average_price: Option<String>,
    pub filled: i64,
    pub pending: i64,
    pub rejection: Option<String>,
    pub rejected_by: Option<String>,
    pub exchange: String,
    pub symbol: String,
    pub exchange_order_id: Option<String>,
    pub remarks: Option<String>,
    pub order_time: Option<String>,
}

// ---------------------------------------------------------------------------
// JSON helpers
// ---------------------------------------------------------------------------

fn text(value: &Value, key: &str) -> Option<String> {
    match value.get(key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

fn number(value: &Value, key: &str) -> i64 {
    match value.get(key) {
        Some(Value::String(s)) => s.trim().parse::<i64>().unwrap_or(0),
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        _ => 0,
    }
}

fn price_text(value: &Value, key: &str) -> Option<String> {
    let raw = text(value, key)?;
    let parsed = Price::parse_rounded(&raw).ok()?;
    Some(parsed.format(2))
}

fn price_value(value: &Value, key: &str) -> Option<Price> {
    Price::parse_rounded(&text(value, key)?).ok()
}

pub fn parse_funds(value: &Value) -> FundsDto {
    FundsDto {
        profile: text(value, "prfname"),
        cash: price_text(value, "cash"),
        payin: price_text(value, "payin"),
        payout: price_text(value, "payout"),
        uncleared: price_text(value, "unclearedcash"),
        blocked: price_text(value, "blk_amt"),
        turnover: price_text(value, "turnover"),
        turnover_limit: price_text(value, "turnoverlmt"),
        pending_order_value: price_text(value, "pendordvallmt"),
        brokerage: price_text(value, "brokerage"),
        peak_margin: price_text(value, "peak_mar"),
        premium: price_text(value, "premium"),
    }
}

pub fn parse_positions(rows: &[Value]) -> Vec<PositionDto> {
    rows.iter()
        .map(|row| PositionDto {
            exchange: text(row, "exch").unwrap_or_default(),
            symbol: text(row, "tsym").unwrap_or_default(),
            token: text(row, "token").unwrap_or_default(),
            product: text(row, "prd").unwrap_or_default(),
            product_alias: text(row, "s_prdt_ali").unwrap_or_default(),
            instrument: text(row, "instname").unwrap_or_default(),
            lot_size: number(row, "ls"),
            net_qty: number(row, "netqty"),
            net_avg: price_text(row, "netavgprc"),
            ltp: price_text(row, "lp"),
            mtm: price_text(row, "urmtom"),
            pnl: price_text(row, "rpnl"),
            day_buy_qty: number(row, "daybuyqty"),
            day_sell_qty: number(row, "daysellqty"),
            buy_value: price_text(row, "totbuyamt"),
            sell_value: price_text(row, "totsellamt"),
        })
        .collect()
}

pub fn parse_orders(rows: &[Value]) -> Vec<OrderDto> {
    rows.iter()
        .map(|row| {
            let quantity = number(row, "qty");
            let filled = number(row, "fillshares");
            OrderDto {
                order_no: text(row, "norenordno").unwrap_or_default(),
                status: text(row, "status").unwrap_or_default(),
                side: text(row, "trantype").unwrap_or_default(),
                price_type: text(row, "prctyp").unwrap_or_default(),
                retention: text(row, "ret").unwrap_or_default(),
                quantity,
                price: price_text(row, "prc"),
                average_price: price_text(row, "avgprc"),
                filled,
                pending: (quantity - filled).max(0),
                rejection: text(row, "rejreason"),
                rejected_by: text(row, "rejby"),
                exchange: text(row, "exch").unwrap_or_default(),
                symbol: text(row, "tsym").unwrap_or_default(),
                exchange_order_id: text(row, "exchordid"),
                remarks: text(row, "remarks"),
                order_time: text(row, "exch_tm").or_else(|| text(row, "norentm")),
            }
        })
        .collect()
}

/// Footer totals for the position book (display only: exact fixed-point sums).
pub fn position_totals(rows: &[PositionDto], raw: &[Value]) -> PositionTotals {
    let mut buy_qty = 0i64;
    let mut sell_qty = 0i64;
    let mut buy_value = Price::ZERO;
    let mut sell_value = Price::ZERO;
    let mut mtm = Price::ZERO;
    let mut rpnl = Price::ZERO;

    for (index, row) in rows.iter().enumerate() {
        buy_qty = buy_qty.saturating_add(row.day_buy_qty);
        sell_qty = sell_qty.saturating_add(row.day_sell_qty);
        if let Some(source) = raw.get(index) {
            if let Some(value) = price_value(source, "totbuyamt") {
                buy_value = buy_value.checked_add(value).unwrap_or(buy_value);
            }
            if let Some(value) = price_value(source, "totsellamt") {
                sell_value = sell_value.checked_add(value).unwrap_or(sell_value);
            }
            if let Some(value) = price_value(source, "urmtom") {
                mtm = mtm.checked_add(value).unwrap_or(mtm);
            }
            if let Some(value) = price_value(source, "rpnl") {
                rpnl = rpnl.checked_add(value).unwrap_or(rpnl);
            }
        }
    }

    let net = buy_value.checked_sub(sell_value).unwrap_or(Price::ZERO);

    PositionTotals {
        buy_qty,
        sell_qty,
        buy_value: buy_value.format(2),
        sell_value: sell_value.format(2),
        net_value: net.format(2),
        mtm: mtm.format(2),
        rpnl: rpnl.format(2),
    }
}

// ---------------------------------------------------------------------------
// Transport
// ---------------------------------------------------------------------------

pub struct RestClient {
    http: reqwest::Client,
}

impl RestClient {
    pub fn new() -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|e| format!("http client: {e}"))?;
        Ok(RestClient { http })
    }

    async fn post_at(
        &self,
        base: &str,
        route: &str,
        json: &str,
        token: &str,
    ) -> Result<Value, String> {
        let body = format!("jData={json}&jKey={token}");
        let response = self
            .http
            .post(format!("{base}/{route}"))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|e| format!("{route}: {e}"))?;

        let status = response.status();
        let body = response.text().await.map_err(|e| format!("{route}: {e}"))?;
        if status.as_u16() == 401 {
            return Err(format!("SESSION: {body}"));
        }
        serde_json::from_str::<Value>(&body)
            .map_err(|_| format!("{route}: HTTP {status}, non-JSON body"))
    }

    /// POST with automatic base fallback (web â‡„ api) when the session scope is
    /// wrong for the chosen host.
    pub async fn post(&self, route: &str, json: &str, token: &str) -> Result<Value, String> {
        let preferred = match PREFERRED.load(Ordering::Relaxed) {
            2 => API_BASE,
            _ => WEB_BASE,
        };
        let other = if preferred == WEB_BASE {
            API_BASE
        } else {
            WEB_BASE
        };

        match self.post_at(preferred, route, json, token).await {
            Ok(value) => {
                PREFERRED.store(if preferred == WEB_BASE { 1 } else { 2 }, Ordering::Relaxed);
                Ok(value)
            }
            Err(err) if err.starts_with("SESSION:") => {
                // Retry once against the other host; a mismatched scope looks
                // exactly like an expired session.
                let value = self.post_at(other, route, json, token).await?;
                PREFERRED.store(if other == WEB_BASE { 1 } else { 2 }, Ordering::Relaxed);
                Ok(value)
            }
            Err(err) => Err(err),
        }
    }
}

fn rows(value: Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items,
        Value::Object(map) => {
            // A single-row response arrives as an object with `stat`.
            if map.contains_key("stat") && !map.contains_key("tsym") {
                Vec::new()
            } else {
                vec![Value::Object(map)]
            }
        }
        _ => Vec::new(),
    }
}

pub async fn funds(
    client: &RestClient,
    uid: &str,
    actid: &str,
    token: &str,
) -> Result<FundsDto, String> {
    let json = format!(r#"{{"uid":"{uid}","actid":"{actid}"}}"#);
    let value = client.post("Limits", &json, token).await?;
    Ok(parse_funds(&value))
}

pub async fn positions(
    client: &RestClient,
    uid: &str,
    actid: &str,
    token: &str,
) -> Result<PositionsView, String> {
    let json = format!(r#"{{"uid":"{uid}","actid":"{actid}"}}"#);
    let value = client.post("PositionBook", &json, token).await?;
    let raw = rows(value);
    let parsed = parse_positions(&raw);
    let totals = position_totals(&parsed, &raw);
    let funds = funds(client, uid, actid, token).await.ok();
    Ok(PositionsView {
        funds,
        positions: parsed,
        totals,
    })
}

pub async fn orders(client: &RestClient, uid: &str, token: &str) -> Result<Vec<OrderDto>, String> {
    let json = format!(r#"{{"uid":"{uid}"}}"#);
    let value = client.post("OrderBook", &json, token).await?;
    Ok(parse_orders(&rows(value)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_positions() -> Vec<Value> {
        let raw = r#"[
          {"stat":"Ok","exch":"NFO","tsym":"NIFTY01DEC26C26150","prd":"M","s_prdt_ali":"NRML",
           "token":"51001","instname":"OPT","ls":"65","netqty":"2","netavgprc":"88.95",
           "lp":"73.90","urmtom":"-1950.00","rpnl":"0.00","daybuyqty":"2","daysellqty":"0",
           "totbuyamt":"11563.50","totsellamt":"0.00"}
        ]"#;
        serde_json::from_str::<Vec<Value>>(raw).unwrap()
    }

    #[test]
    fn parses_a_position_row() {
        let raw = sample_positions();
        let parsed = parse_positions(&raw);
        assert_eq!(parsed.len(), 1);
        let p = &parsed[0];
        assert_eq!(p.symbol, "NIFTY01DEC26C26150");
        assert_eq!(p.product_alias, "NRML");
        assert_eq!(p.net_qty, 2);
        assert_eq!(p.net_avg.as_deref(), Some("88.95"));
        assert_eq!(p.ltp.as_deref(), Some("73.90"));
        assert_eq!(p.mtm.as_deref(), Some("-1950.00"));
    }

    #[test]
    fn totals_use_exact_fixed_point_sums() {
        let raw = sample_positions();
        let parsed = parse_positions(&raw);
        let totals = position_totals(&parsed, &raw);
        assert_eq!(totals.buy_qty, 2);
        assert_eq!(totals.sell_qty, 0);
        assert_eq!(totals.buy_value, "11563.50");
        assert_eq!(totals.sell_value, "0.00");
        assert_eq!(totals.net_value, "11563.50");
        assert_eq!(totals.mtm, "-1950.00");
        assert_eq!(totals.rpnl, "0.00");
    }

    #[test]
    fn parses_funds() {
        let raw: Value = serde_json::from_str(
            r#"{"stat":"Ok","cash":"10675.73","payin":"0.00","payout":"0.00",
                "unclearedcash":"0.00","blk_amt":"15000.00","turnover":"123456.78",
                "turnoverlmt":"1000000.00","pendordvallmt":"5000.00","brokerage":"12.34",
                "peak_mar":"25000.00","premium":"-100.00"}"#,
        )
        .unwrap();
        let funds = parse_funds(&raw);
        assert_eq!(funds.cash.as_deref(), Some("10675.73"));
        assert_eq!(funds.blocked.as_deref(), Some("15000.00"));
        assert_eq!(funds.peak_margin.as_deref(), Some("25000.00"));
    }

    #[test]
    fn parses_orders_and_pending_quantity() {
        let raw: Vec<Value> = serde_json::from_str(
            r#"[
              {"stat":"Ok","norenordno":"24121500001234","status":"COMPLETE","trantype":"B",
               "prctyp":"LMT","ret":"DAY","qty":"65","fillshares":"65","prc":"88.95",
               "avgprc":"88.95","exch":"NFO","tsym":"NIFTY01DEC26C26150",
               "exchordid":"250620000000343421","remarks":"x"}
            ]"#,
        )
        .unwrap();
        let orders = parse_orders(&raw);
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].status, "COMPLETE");
        assert_eq!(orders[0].pending, 0);
        assert_eq!(orders[0].price.as_deref(), Some("88.95"));
    }

    #[test]
    fn rejects_are_kept() {
        let raw: Vec<Value> = serde_json::from_str(
            r#"[{"norenordno":"1","status":"REJECTED","qty":"65","fillshares":"0",
                 "rejreason":"RED:Margin Shortfall","rejby":"RMS","trantype":"S"}]"#,
        )
        .unwrap();
        let orders = parse_orders(&raw);
        assert_eq!(orders[0].rejection.as_deref(), Some("RED:Margin Shortfall"));
        assert_eq!(orders[0].rejected_by.as_deref(), Some("RMS"));
        assert_eq!(orders[0].pending, 65);
    }

    #[test]
    fn empty_book_is_empty_not_an_error() {
        assert!(parse_positions(&rows(Value::Array(vec![]))).is_empty());
        assert!(parse_orders(&rows(Value::Null)).is_empty());
    }
}
