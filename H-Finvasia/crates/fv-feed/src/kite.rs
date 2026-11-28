//! Kite Connect websocket groundwork.
//!
//! This module deliberately contains **no network connection**. It captures the
//! web `enctoken` handshake shape used by the existing `kite-client` so a second
//! feed can be added later without reverse-engineering the protocol again:
//!
//! ```text
//! wss://ws.zerodha.com/?api_key=kitefront&user_id=<KITE_USERID>
//!   &enctoken=<percent-encoded enctoken>&uid=<millis>
//!   &user-agent=kite3-web&version=3.0.0
//! ```
//!
//! Authentication is in the URL (not a first frame). Subscriptions and modes are
//! JSON text frames; ticks are binary. The actual tick decoder and socket live
//! in the future provider implementation.

use serde_json::json;

/// Web ticker root used by Kite web sessions.
pub const KITE_WS_ROOT: &str = "wss://ws.zerodha.com/";
/// Fixed app key used by the Kite web ticker.
pub const KITE_API_KEY: &str = "kitefront";
/// Web client identity sent in the URL and as a header.
pub const KITE_USER_AGENT: &str = "kite3-web";
/// Web client protocol version.
pub const KITE_VERSION: &str = "3.0.0";

/// Configuration needed to open a Kite web ticker. The `enc_token` is the
/// `LATEST_KITE_ENC_KEY` value, never the public session token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KiteTickerConfig {
    pub user_id: String,
    pub enc_token: String,
    pub root: String,
    pub api_key: String,
    pub user_agent: String,
    pub version: String,
}

impl KiteTickerConfig {
    pub fn new(user_id: impl Into<String>, enc_token: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            enc_token: enc_token.into(),
            root: KITE_WS_ROOT.to_string(),
            api_key: KITE_API_KEY.to_string(),
            user_agent: KITE_USER_AGENT.to_string(),
            version: KITE_VERSION.to_string(),
        }
    }

    pub fn websocket_url(&self) -> String {
        self.websocket_url_at(now_millis())
    }

    /// Deterministic URL builder used by tests.
    pub fn websocket_url_at(&self, uid_millis: u128) -> String {
        format!(
            "{root}?api_key={api}&user_id={user}&enctoken={token}&uid={uid}&user-agent={ua}&version={version}",
            root = self.root,
            api = encode_component(&self.api_key),
            user = encode_component(&self.user_id),
            token = encode_component(&self.enc_token),
            uid = uid_millis,
            ua = encode_component(&self.user_agent),
            version = encode_component(&self.version),
        )
    }

    /// Headers required by the web ticker. The caller adds these when opening
    /// the socket; this function never performs I/O.
    pub fn headers(&self) -> [(&'static str, String); 2] {
        [
            ("X-Kite-Version", "3".to_string()),
            ("User-Agent", self.user_agent.clone()),
        ]
    }
}

/// `{"a":"subscribe","v":[token,...]}` for the Kite binary ticker.
pub fn subscribe_frame(tokens: &[u64]) -> String {
    json!({ "a": "subscribe", "v": tokens }).to_string()
}

/// `{"a":"mode","v":["full",token,...]}`. Full mode carries depth and OI.
pub fn mode_full_frame(tokens: &[u64]) -> String {
    let mut values = Vec::with_capacity(tokens.len() + 1);
    values.push(serde_json::Value::String("full".to_string()));
    values.extend(tokens.iter().map(|token| json!(token)));
    json!({ "a": "mode", "v": values }).to_string()
}

/// Split an instrument list between the primary feed and a secondary feed.
/// The primary keeps at most `primary_limit` keys; the remainder is returned in
/// order for the secondary provider. Used when Shoonya's 100-instrument budget
/// is full and a Kite feed can carry the overflow.
pub fn split_for_providers(primary_limit: usize, keys: Vec<String>) -> (Vec<String>, Vec<String>) {
    let mut primary = Vec::new();
    let mut secondary = Vec::new();
    for key in keys {
        if primary.len() < primary_limit {
            primary.push(key);
        } else {
            secondary.push(key);
        }
    }
    (primary, secondary)
}

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

/// Percent-encode a URL component the same way `encodeURIComponent` does for
/// the token/user fields (unreserved characters stay literal).
fn encode_component(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_url_matches_the_web_ticker_shape() {
        let cfg = KiteTickerConfig::new("AB1234", "enc+token/value==");
        let url = cfg.websocket_url_at(1_700_000_000_000);
        assert!(url.starts_with("wss://ws.zerodha.com/?api_key=kitefront"));
        assert!(url.contains("user_id=AB1234"));
        assert!(url.contains("enctoken=enc%2Btoken%2Fvalue%3D%3D"));
        assert!(url.contains("uid=1700000000000"));
        assert!(url.contains("user-agent=kite3-web"));
        assert!(url.contains("version=3.0.0"));
    }

    #[test]
    fn frames_are_valid_json() {
        let subscribe: serde_json::Value =
            serde_json::from_str(&subscribe_frame(&[256265, 738561])).unwrap();
        assert_eq!(subscribe["a"], "subscribe");
        assert_eq!(subscribe["v"][0], 256265);

        let mode: serde_json::Value = serde_json::from_str(&mode_full_frame(&[256265])).unwrap();
        assert_eq!(mode["a"], "mode");
        assert_eq!(mode["v"][0], "full");
        assert_eq!(mode["v"][1], 256265);
    }

    #[test]
    fn split_keeps_primary_full_and_spills_the_rest() {
        let keys: Vec<String> = (0..5).map(|index| format!("NSE|{index}")).collect();
        let (primary, secondary) = split_for_providers(2, keys.clone());
        assert_eq!(primary, keys[..2]);
        assert_eq!(secondary, keys[2..]);

        let (all, none) = split_for_providers(10, keys);
        assert_eq!(all.len(), 5);
        assert!(none.is_empty());
    }
}
