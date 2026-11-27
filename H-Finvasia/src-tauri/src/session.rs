//! Session resolution.
//!
//! Three token sources, tried in order, cheapest and least intrusive first:
//!
//! 1. `FV_ACCESS_TOKEN` â€” explicit override for CI and tests.
//! 2. `STOCK.CONFIGS_GLOBAL.FINVASIA_LOGIN_DETAIL` â€” the session already
//!    persisted by the user's existing tooling. Adopted **read-only**: logging in
//!    again would invalidate the session their other tools are using.
//! 3. `QuickAuth` with the TOTP secret from `fv-cred.json` â€” only when no stored
//!    token exists or it no longer validates.
//!
//! Credentials are never written back to the database and never logged.

use crate::broker_log;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// REST base that is actually live. `NorenWClientTP` (used by older clients)
/// now answers 502; `NorenWClientAPI` is the working endpoint.
pub const REST_BASE: &str = "https://api.shoonya.com/NorenWClientAPI";

/// Sent as `apkversion`; the venue only requires a plausible client version.
const APKVERSION: &str = "js:1.0.0";

/// Stable device id. The venue treats it as opaque.
const IMEI: &str = "98fe0484-ef1e-4635-b6f5-3809af541f64";

/// Known instrument used to prove a token still works (RELIANCE, NSE).
const PROBE_EXCHANGE: &str = "NSE";
const PROBE_TOKEN: &str = "2885";

#[derive(Debug, Clone)]
pub struct Credentials {
    pub userid: String,
    pub password: String,
    pub api_secret: String,
    pub totp_secret: String,
    /// Optional explicit account id. The WS `actid` is the trading account, which
    /// is the user id unless the broker says otherwise.
    pub actid: Option<String>,
    /// Optional vendor code for `QuickAuth` (`vc`).
    pub vendor_code: Option<String>,
}

impl Credentials {
    pub fn actid(&self) -> String {
        self.actid
            .clone()
            .filter(|a| !a.trim().is_empty())
            .unwrap_or_else(|| self.userid.clone())
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TokenSource {
    Env,
    Stored,
    Login,
}

impl TokenSource {
    pub fn as_str(self) -> &'static str {
        match self {
            TokenSource::Env => "env",
            TokenSource::Stored => "stored",
            TokenSource::Login => "login",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub uid: String,
    pub actid: String,
    pub source: String,
    pub has_credentials: bool,
    pub has_stored_token: bool,
    pub stored_date: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub token: String,
    pub uid: String,
    pub actid: String,
    pub source: TokenSource,
}

// ---------------------------------------------------------------------------
// Credential file
// ---------------------------------------------------------------------------

/// Candidate locations, in order. `FV_CRED_FILE` wins so the file can live
/// anywhere without hardcoding a machine-specific path.
pub fn credential_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(explicit) = std::env::var("FV_CRED_FILE") {
        if !explicit.trim().is_empty() {
            out.push(PathBuf::from(explicit));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            out.push(dir.join("fv-cred.json"));
        }
    }
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        out.push(PathBuf::from(home).join(".fv").join("fv-cred.json"));
    }
    out.push(PathBuf::from("C:/repo/fv-cred.json"));
    out
}

pub fn load_credentials() -> Result<Credentials, String> {
    let mut tried = Vec::new();
    for path in credential_paths() {
        if !path.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        return parse_credentials(&text).map_err(|e| format!("{}: {e}", path.display()));
    }
    Err(format!(
        "no credential file found (set FV_CRED_FILE). Looked in: {}",
        try_list(&mut tried)
    ))
}

fn try_list(tried: &mut Vec<String>) -> String {
    let joined = credential_paths()
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    tried.push(joined.clone());
    joined
}

/// Accepts `{"authparams":{...}}` (the shape used by the existing tooling) and a
/// flat object for convenience.
pub fn parse_credentials(text: &str) -> Result<Credentials, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("invalid json: {e}"))?;
    let obj = value
        .get("authparams")
        .cloned()
        .unwrap_or_else(|| value.clone());

    let get = |keys: &[&str]| -> Option<String> {
        keys.iter()
            .find_map(|k| obj.get(*k).and_then(|v| v.as_str()))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let userid = get(&["userid", "uid", "userId"]).ok_or("missing userid")?;
    let password = get(&["password", "pwd"]).ok_or("missing password")?;
    let api_secret = get(&["api_secret", "apiSecret"]).ok_or("missing api_secret")?;
    let totp_secret = get(&["f2token", "totp_secret", "totpSecret"]).ok_or("missing f2token")?;
    // Deliberately NOT reading `report_schoonya`: in the existing file that field
    // holds a password, and using it as an account id / vendor code silently
    // breaks the websocket handshake.
    let actid = get(&["actid", "account_id"]);
    let vendor_code = get(&["vendor_code", "vc"]);

    Ok(Credentials {
        userid,
        password,
        api_secret,
        totp_secret,
        actid,
        vendor_code,
    })
}

// ---------------------------------------------------------------------------
// Persisted token (read-only)
// ---------------------------------------------------------------------------

fn mongo_uri() -> String {
    std::env::var("FV_MONGO_URI").unwrap_or_else(|_| "mongodb://127.0.0.1:27017".to_string())
}

fn mongo_db() -> String {
    std::env::var("FV_MONGO_DB").unwrap_or_else(|_| "STOCK".to_string())
}

/// Read `FINVASIA_LOGIN_DETAIL` / `FINVASIA_LOGIN_DATE` without writing anything.
pub async fn persisted_token() -> Option<(String, Option<String>)> {
    use bson::doc;
    use mongodb::Client;

    let client = Client::with_uri_str(mongo_uri()).await.ok()?;
    let collection = client
        .database(&mongo_db())
        .collection::<bson::Document>("CONFIGS_GLOBAL");

    let find = |key: &'static str| {
        let collection = collection.clone();
        async move {
            collection
                .find_one(doc! { "configKey": key })
                .await
                .ok()
                .flatten()
                .and_then(|d| d.get_str("configValue").ok().map(str::to_string))
        }
    };

    let token = find("FINVASIA_LOGIN_DETAIL").await?;
    let date = find("FINVASIA_LOGIN_DATE").await;
    client.shutdown().await;
    Some((token.trim().to_string(), date))
}

/// Persist a freshly logged-in token in the same `CONFIGS_GLOBAL` documents the
/// fv-client uses, so the next launch (and the other tools) can adopt it.
pub async fn persist_token(token: &str) -> Result<(), String> {
    use bson::doc;
    use mongodb::Client;

    let client = Client::with_uri_str(mongo_uri())
        .await
        .map_err(|e| format!("mongo: {e}"))?;
    let collection = client
        .database(&mongo_db())
        .collection::<bson::Document>("CONFIGS_GLOBAL");
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();

    for (key, value) in [
        ("FINVASIA_LOGIN_DETAIL", token.to_string()),
        ("FINVASIA_LOGIN_DATE", date),
    ] {
        collection
            .update_one(
                doc! { "configKey": key },
                doc! {
                    "$set": {
                        "configKey": key,
                        "configValue": value,
                    }
                },
            )
            .upsert(true)
            .await
            .map_err(|e| format!("persist {key}: {e}"))?;
    }
    client.shutdown().await;
    Ok(())
}

// ---------------------------------------------------------------------------
// REST
// ---------------------------------------------------------------------------

fn form_body(json: &str, jkey: Option<&str>) -> String {
    match jkey {
        // The Noren endpoints expect raw JSON after `jData=`; percent-encoding
        // it breaks authentication.
        Some(key) => format!("jData={json}&jKey={key}"),
        None => format!("jData={json}"),
    }
}

async fn post_route(
    client: &reqwest::Client,
    route: &str,
    json: &str,
    jkey: Option<&str>,
) -> Result<serde_json::Value, String> {
    let body = form_body(json, jkey);
    tracing::debug!(
        route,
        host = REST_BASE,
        request = %broker_log::redact_form_body(&body),
        "broker auth request"
    );
    let started = Instant::now();
    let response = match client
        .post(format!("{REST_BASE}/{route}"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(
                route,
                host = REST_BASE,
                elapsed_ms = started.elapsed().as_millis() as u64,
                %error,
                "broker auth transport failed"
            );
            return Err(format!("{route}: {error}"));
        }
    };

    let status = response.status();
    let text = match response.text().await {
        Ok(text) => text,
        Err(error) => {
            tracing::warn!(
                route,
                status = status.as_u16(),
                elapsed_ms = started.elapsed().as_millis() as u64,
                %error,
                "broker auth response read failed"
            );
            return Err(format!("{route}: {error}"));
        }
    };
    tracing::debug!(
        route,
        status = status.as_u16(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        response = %broker_log::clip(&broker_log::redact_json(&text), 8 * 1024),
        "broker auth response"
    );

    serde_json::from_str::<serde_json::Value>(&text)
        .map_err(|_| format!("{route}: HTTP {status}, non-JSON body: {}", truncate(&text)))
}

fn truncate(text: &str) -> String {
    text.chars().take(160).collect()
}

/// Cheap liveness check: fetch one well-known quote.
pub async fn validate_token(client: &reqwest::Client, token: &str, uid: &str) -> bool {
    let json = format!(r#"{{"uid":"{uid}","exch":"{PROBE_EXCHANGE}","token":"{PROBE_TOKEN}"}}"#);
    match post_route(client, "GetQuotes", &json, Some(token)).await {
        Ok(value) => value.get("stat").and_then(|s| s.as_str()) == Some("Ok"),
        Err(err) => {
            tracing::debug!(%err, "token validation failed");
            false
        }
    }
}

fn sha256_hex(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// RFC 6238 TOTP (SHA-1, 6 digits, 30 s period).
pub fn totp(secret_base32: &str, unix_seconds: u64) -> Result<String, String> {
    use data_encoding::BASE32_NOPAD;
    use hmac::{Hmac, Mac};
    use sha1::Sha1;

    let normalized: String = secret_base32
        .trim()
        .to_ascii_uppercase()
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '=')
        .collect();
    let key = BASE32_NOPAD
        .decode(normalized.as_bytes())
        .map_err(|e| format!("invalid TOTP secret: {e}"))?;

    let counter = unix_seconds / 30;
    let mut mac = Hmac::<Sha1>::new_from_slice(&key).map_err(|e| format!("hmac: {e}"))?;
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();

    let offset = (digest[19] & 0x0f) as usize;
    let binary = ((digest[offset] as u32 & 0x7f) << 24)
        | ((digest[offset + 1] as u32) << 16)
        | ((digest[offset + 2] as u32) << 8)
        | (digest[offset + 3] as u32);
    Ok(format!("{:06}", binary % 1_000_000))
}

/// `QuickAuth` login. Returns `(token, actid)`.
pub async fn quick_auth(
    client: &reqwest::Client,
    credentials: &Credentials,
) -> Result<(String, String), String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let factor2 = totp(&credentials.totp_secret, now)?;
    let appkey = sha256_hex(&format!(
        "{}|{}",
        credentials.userid, credentials.api_secret
    ));

    let json = format!(
        r#"{{"source":"API","apkversion":"{APKVERSION}","uid":"{}","pwd":"{}","factor2":"{}","vc":"{}","appkey":"{}","imei":"{}"}}"#,
        credentials.userid,
        sha256_hex(&credentials.password),
        factor2,
        credentials.vendor_code.clone().unwrap_or_default(),
        appkey,
        IMEI
    );

    let value = post_route(client, "QuickAuth", &json, None).await?;
    match value.get("stat").and_then(|s| s.as_str()) {
        Some("Ok") => {
            let token = value
                .get("susertoken")
                .and_then(|s| s.as_str())
                .ok_or("login returned no susertoken")?
                .to_string();
            let actid = value
                .get("actid")
                .and_then(|s| s.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| credentials.actid());
            Ok((token, actid))
        }
        _ => Err(format!(
            "login rejected: {}",
            value
                .get("emsg")
                .and_then(|s| s.as_str())
                .unwrap_or("unknown error")
        )),
    }
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("http client: {e}"))
}

/// Resolve a usable session: env â†’ stored (validated) â†’ fresh login.
pub async fn resolve_session() -> Result<Session, String> {
    let client = build_client()?;
    let credentials = load_credentials().ok();

    let uid = credentials
        .as_ref()
        .map(|c| c.userid.clone())
        .or_else(|| std::env::var("FV_UID").ok())
        .ok_or("no userid: set FV_CRED_FILE or FV_UID")?;
    let actid = credentials
        .as_ref()
        .map(|c| c.actid())
        .or_else(|| std::env::var("FV_ACTID").ok())
        .unwrap_or_else(|| uid.clone());

    if let Ok(token) = std::env::var("FV_ACCESS_TOKEN") {
        let token = token.trim().to_string();
        if !token.is_empty() {
            return Ok(Session {
                token,
                uid,
                actid,
                source: TokenSource::Env,
            });
        }
    }

    if let Some((token, date)) = persisted_token().await {
        if !token.is_empty() {
            if validate_token(&client, &token, &uid).await {
                tracing::info!(issued = ?date, "adopted persisted session");
                return Ok(Session {
                    token,
                    uid,
                    actid,
                    source: TokenSource::Stored,
                });
            }
            tracing::warn!("persisted session no longer validates; will log in");
        }
    }

    let credentials = credentials.ok_or("no stored session and no credential file to log in")?;
    let (token, actid) = quick_auth(&client, &credentials).await?;
    if let Err(err) = persist_token(&token).await {
        tracing::warn!(error = %err, "could not persist the fresh login token");
    }
    tracing::info!("logged in with TOTP");
    Ok(Session {
        token,
        uid,
        actid,
        source: TokenSource::Login,
    })
}

/// Resolve a session **without ever logging in**.
///
/// Used at startup so the app can connect automatically when a usable token
/// already exists, while never rotating a session just because the app opened.
pub async fn resolve_existing() -> Option<Session> {
    let client = build_client().ok()?;
    let credentials = load_credentials().ok();

    let uid = credentials
        .as_ref()
        .map(|c| c.userid.clone())
        .or_else(|| std::env::var("FV_UID").ok())?;
    let actid = credentials
        .as_ref()
        .map(|c| c.actid())
        .or_else(|| std::env::var("FV_ACTID").ok())
        .unwrap_or_else(|| uid.clone());

    if let Ok(token) = std::env::var("FV_ACCESS_TOKEN") {
        let token = token.trim().to_string();
        if !token.is_empty() && validate_token(&client, &token, &uid).await {
            return Some(Session {
                token,
                uid,
                actid,
                source: TokenSource::Env,
            });
        }
    }

    let (token, date) = persisted_token().await?;
    if token.is_empty() || !validate_token(&client, &token, &uid).await {
        tracing::info!("no valid stored session; waiting for an explicit connect");
        return None;
    }
    tracing::info!(issued = ?date, "auto-connecting with the stored session");
    Some(Session {
        token,
        uid,
        actid,
        source: TokenSource::Stored,
    })
}

/// Non-invasive status for the UI. Does not validate or log in.
pub async fn peek() -> SessionInfo {
    let credentials = load_credentials().ok();
    let has_env = std::env::var("FV_ACCESS_TOKEN")
        .map(|t| !t.trim().is_empty())
        .unwrap_or(false);
    let (has_stored, stored_date) = match persisted_token().await {
        Some((token, date)) => (!token.is_empty(), date),
        None => (false, None),
    };

    let uid = credentials
        .as_ref()
        .map(|c| c.userid.clone())
        .or_else(|| std::env::var("FV_UID").ok())
        .unwrap_or_default();

    SessionInfo {
        actid: credentials
            .as_ref()
            .map(|c| c.actid())
            .unwrap_or_else(|| uid.clone()),
        uid,
        source: if has_env {
            "env".to_string()
        } else if has_stored {
            "stored".to_string()
        } else if credentials.is_some() {
            "login".to_string()
        } else {
            "none".to_string()
        },
        has_credentials: credentials.is_some(),
        has_stored_token: has_stored,
        stored_date,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CRED: &str = r#"{
      "authparams": {
        "userid": "AB1234",
        "password": "secret",
        "f2token": "JBSWY3DPEHPK3PXP",
        "api_secret": "abcdef",
        "report_schoonya": "AB1234"
      }
    }"#;

    #[test]
    fn parses_the_nested_authparams_shape() {
        let c = parse_credentials(CRED).unwrap();
        assert_eq!(c.userid, "AB1234");
        assert_eq!(c.actid(), "AB1234");
        assert_eq!(c.totp_secret, "JBSWY3DPEHPK3PXP");
    }

    #[test]
    fn parses_a_flat_object_too() {
        let c = parse_credentials(
            r#"{"userid":"U","password":"P","api_secret":"S","f2token":"JBSWY3DPEHPK3PXP"}"#,
        )
        .unwrap();
        assert_eq!(c.userid, "U");
        assert_eq!(c.actid(), "U", "falls back to the user id");
    }

    #[test]
    fn rejects_incomplete_credentials() {
        assert!(parse_credentials(r#"{"authparams":{"userid":"U"}}"#).is_err());
        assert!(parse_credentials("not json").is_err());
    }

    #[test]
    fn totp_matches_rfc6238_vectors() {
        // RFC 6238 test secret ("12345678901234567890" base32) at T=59s.
        let secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        assert_eq!(totp(secret, 59).unwrap(), "287082");
        assert_eq!(totp(secret, 1111111109).unwrap(), "081804");
    }

    #[test]
    fn sha256_is_lowercase_hex() {
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn form_body_keeps_json_unencoded() {
        let body = form_body(r#"{"a":"b"}"#, Some("tok"));
        assert_eq!(body, r#"jData={"a":"b"}&jKey=tok"#);
        assert!(!body.contains("%7B"));
        assert_eq!(form_body("{}", None), "jData={}");
    }
}
