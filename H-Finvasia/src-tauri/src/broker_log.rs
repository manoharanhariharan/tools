//! Broker-traffic log helpers.
//!
//! The detailed order/account logs are deliberately temporary diagnostics: they
//! capture the exact request and response so a broker rejection can be explained
//! without guessing. Secrets are redacted before anything reaches the log file.

use serde_json::Value;
use std::path::PathBuf;

/// Keys whose values must never be written to disk.
const SENSITIVE_KEYS: &[&str] = &[
    "susertoken",
    "jkey",
    "jdata",
    "pwd",
    "password",
    "factor2",
    "appkey",
    "api_secret",
    "apisecret",
    "totp_secret",
    "totpsecret",
    "f2token",
    "access_token",
    "accesstoken",
];

fn is_sensitive(key: &str) -> bool {
    let key = key.trim().to_ascii_lowercase();
    SENSITIVE_KEYS.contains(&key.as_str())
}

fn redact_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                if is_sensitive(key) {
                    *item = Value::String("***".into());
                } else {
                    redact_value(item);
                }
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                redact_value(item);
            }
        }
        _ => {}
    }
}

/// Redact known secret fields from a JSON document. Non-JSON text is returned
/// unchanged (callers may clip it).
pub fn redact_json(text: &str) -> String {
    match serde_json::from_str::<Value>(text) {
        Ok(mut value) => {
            redact_value(&mut value);
            value.to_string()
        }
        Err(_) => text.to_string(),
    }
}

/// Redact `jKey` and JSON secrets from the `jData=...&jKey=...` form body.
pub fn redact_form_body(body: &str) -> String {
    let (data, has_key) = match body.split_once("&jKey=") {
        Some((data, _)) => (data, true),
        None => (body, false),
    };
    let data = match data.strip_prefix("jData=") {
        Some(json) => format!("jData={}", redact_json(json)),
        None => data.to_string(),
    };
    if has_key {
        format!("{data}&jKey=***")
    } else {
        data
    }
}

/// Bound one log field so a large position/order book cannot flood the file.
pub fn clip(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut out: String = text.chars().take(limit).collect();
    out.push_str("...<clipped>");
    out
}

/// Where the diagnostic log lives. Matches Tauri's `app_config_dir` on Windows
/// (`%APPDATA%\com.hfinvasia.desktop`) and falls back to the temp directory.
pub fn log_dir() -> PathBuf {
    if let Some(base) = std::env::var_os("APPDATA") {
        PathBuf::from(base)
            .join("com.hfinvasia.desktop")
            .join("logs")
    } else {
        std::env::temp_dir().join("h-finvasia-logs")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_session_and_credential_fields() {
        let text = redact_json(
            r#"{"stat":"Ok","susertoken":"secret-token","actid":"AB1234","pwd":"hash","factor2":"123456"}"#,
        );
        assert!(!text.contains("secret-token"));
        assert!(!text.contains("hash"));
        assert!(!text.contains("123456"));
        assert!(text.contains("AB1234"), "non-secret fields stay readable");
    }

    #[test]
    fn redacts_form_jkey_and_nested_secrets() {
        let body = redact_form_body(r#"jData={"uid":"U","pwd":"hash"}&jKey=session-token"#);
        assert!(!body.contains("hash"));
        assert!(!body.contains("session-token"));
        assert!(body.contains("jKey=***"));
        assert!(body.contains("\"uid\":\"U\""));
    }

    #[test]
    fn leaves_plain_text_and_clips_large_bodies() {
        assert_eq!(redact_json("not json"), "not json");
        assert_eq!(clip("abcdef", 3), "abc...<clipped>");
        assert_eq!(clip("abc", 3), "abc");
    }
}
