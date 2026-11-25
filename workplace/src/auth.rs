//! Authentication: single-admin password, opaque server-side sessions, a
//! bounded login throttle, and the middleware that protects `/api`.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, Method, header};
use axum::middleware::Next;
use axum::response::Response;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::AppState;
use crate::domain::now_millis;
use crate::error::{AppError, AppResult};

pub const SESSION_COOKIE: &str = "daybook_session";
pub const MIN_PASSWORD_LEN: usize = 10;
pub const MAX_PASSWORD_LEN: usize = 200;
pub const PIN_LEN: usize = 4;

const MAX_TRACKED_IPS: usize = 4_096;
const MAX_FAILURES: u32 = 5;
const FAILURE_WINDOW: Duration = Duration::from_secs(5 * 60);
const BLOCK_DURATION: Duration = Duration::from_secs(5 * 60);

/// Per-IP login throttle. Memory is bounded: at most `MAX_TRACKED_IPS`
/// entries, pruned opportunistically. Never grows without limit.
#[derive(Debug)]
struct Tracked {
    window_start: Instant,
    failures: u32,
    blocked_until: Option<Instant>,
}

pub struct LoginLimiter {
    inner: Mutex<HashMap<IpAddr, Tracked>>,
    max_failures: u32,
    window: Duration,
    block_for: Duration,
}

impl std::fmt::Debug for LoginLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = self.inner.lock().map(|m| m.len()).unwrap_or(0);
        f.debug_struct("LoginLimiter")
            .field("tracked", &len)
            .finish()
    }
}

impl Default for LoginLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl LoginLimiter {
    pub fn new() -> Self {
        Self::with_limits(MAX_FAILURES, FAILURE_WINDOW, BLOCK_DURATION)
    }

    pub fn with_limits(max_failures: u32, window: Duration, block_for: Duration) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            max_failures,
            window,
            block_for,
        }
    }

    /// Is this IP currently allowed to attempt a login?
    pub fn is_allowed(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut map = match self.inner.lock() {
            Ok(map) => map,
            Err(poisoned) => poisoned.into_inner(),
        };
        prune_map(&mut map, now, self.window);

        let Some(entry) = map.get_mut(&ip) else {
            return true;
        };
        if entry.blocked_until.is_some_and(|until| until > now) {
            return false;
        }
        if now.duration_since(entry.window_start) >= self.window {
            entry.window_start = now;
            entry.failures = 0;
            entry.blocked_until = None;
        }
        entry.failures < self.max_failures
    }

    pub fn record_failure(&self, ip: IpAddr) {
        let now = Instant::now();
        let mut map = match self.inner.lock() {
            Ok(map) => map,
            Err(poisoned) => poisoned.into_inner(),
        };
        prune_map(&mut map, now, self.window);
        let entry = map.entry(ip).or_insert_with(|| Tracked {
            window_start: now,
            failures: 0,
            blocked_until: None,
        });
        if now.duration_since(entry.window_start) >= self.window {
            entry.window_start = now;
            entry.failures = 0;
            entry.blocked_until = None;
        }
        entry.failures = entry.failures.saturating_add(1);
        if entry.failures >= self.max_failures {
            entry.blocked_until = Some(now + self.block_for);
        }
    }

    pub fn record_success(&self, ip: IpAddr) {
        let mut map = match self.inner.lock() {
            Ok(map) => map,
            Err(poisoned) => poisoned.into_inner(),
        };
        map.remove(&ip);
    }

    #[cfg(test)]
    fn tracked_count(&self) -> usize {
        self.inner.lock().map(|m| m.len()).unwrap_or(0)
    }
}

pub fn validate_password_policy(password: &str) -> Result<(), String> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(format!(
            "password must be at least {MIN_PASSWORD_LEN} characters"
        ));
    }
    if password.chars().count() > MAX_PASSWORD_LEN {
        return Err(format!(
            "password must be at most {MAX_PASSWORD_LEN} characters"
        ));
    }
    if password.chars().all(|c| c.is_whitespace()) {
        return Err("password may not be blank".to_string());
    }
    Ok(())
}

pub fn validate_pin_policy(pin: &str) -> Result<(), String> {
    if pin.chars().count() != PIN_LEN || !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("PIN must be exactly {PIN_LEN} digits"));
    }
    Ok(())
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("password hashing failed: {e}"))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Create a session and return `(raw_token, expires_at_ms)`. Only the hash is
/// persisted, so a database leak does not hand out live sessions.
pub async fn create_session(
    pool: &SqlitePool,
    ttl: Duration,
    role: SessionRole,
    employee_id: Option<&str>,
    name: &str,
) -> AppResult<(String, i64)> {
    let token = generate_token();
    let hash = hash_token(&token);
    let now = now_millis();
    let expires_at = now.saturating_add(ttl.as_millis() as i64);
    sqlx::query(
        "INSERT INTO sessions (token_hash, created_at, expires_at, role, employee_id, name)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&hash)
    .bind(now)
    .bind(expires_at)
    .bind(role.as_str())
    .bind(employee_id)
    .bind(name)
    .execute(pool)
    .await?;
    Ok((token, expires_at))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRole {
    Admin,
    Employee,
}

impl SessionRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            SessionRole::Admin => "admin",
            SessionRole::Employee => "employee",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "admin" => Some(SessionRole::Admin),
            "employee" => Some(SessionRole::Employee),
            _ => None,
        }
    }
}

/// The authenticated subject recovered from a session cookie.
#[derive(Debug, Clone)]
pub struct Identity {
    pub role: SessionRole,
    pub employee_id: Option<String>,
    pub name: String,
    pub expires_at: i64,
}

impl Identity {
    pub fn is_admin(&self) -> bool {
        self.role == SessionRole::Admin
    }

    pub fn require_admin(&self) -> AppResult<()> {
        if self.is_admin() {
            Ok(())
        } else {
            Err(AppError::Forbidden("admin access required"))
        }
    }

    /// The employee id an employee session is restricted to (admin has no scope).
    pub fn employee_scope(&self) -> Option<&str> {
        if self.is_admin() {
            None
        } else {
            self.employee_id.as_deref()
        }
    }

    /// Audit-trail actor tuple: (role, employee id, display name).
    pub fn actor(&self) -> (&'static str, Option<&str>, &str) {
        (self.role.as_str(), self.employee_id.as_deref(), &self.name)
    }
}

/// Extractor for the identity inserted by `require_auth`.
pub struct Auth(pub Identity);

impl<S: Send + Sync> FromRequestParts<S> for Auth {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Identity>()
            .cloned()
            .map(Auth)
            .ok_or(AppError::Unauthorized)
    }
}

pub async fn session_identity(pool: &SqlitePool, token: &str) -> AppResult<Option<Identity>> {
    if token.is_empty() || token.len() > 128 {
        return Ok(None);
    }
    let row: Option<(String, Option<String>, String, i64)> = sqlx::query_as(
        "SELECT role, employee_id, name, expires_at FROM sessions WHERE token_hash = ?",
    )
    .bind(hash_token(token))
    .fetch_optional(pool)
    .await?;
    match row {
        Some((role, employee_id, name, expires_at)) if expires_at > now_millis() => {
            let Some(role) = SessionRole::parse(&role) else {
                return Ok(None);
            };
            Ok(Some(Identity {
                role,
                employee_id,
                name,
                expires_at,
            }))
        }
        _ => Ok(None),
    }
}

pub async fn delete_session(pool: &SqlitePool, token: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(hash_token(token))
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn cleanup_expired_sessions(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
        .bind(now_millis())
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|raw| raw.split(';'))
        .filter_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            (name == SESSION_COOKIE && !value.is_empty()).then(|| value.to_string())
        })
        .next()
}

pub fn session_cookie(token: &str, ttl_secs: i64, secure: bool) -> HeaderValue {
    let mut value =
        format!("{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={ttl_secs}");
    if secure {
        value.push_str("; Secure");
    }
    HeaderValue::from_str(&value).expect("session cookie is valid header data")
}

pub fn cleared_session_cookie(secure: bool) -> HeaderValue {
    let mut value = format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0");
    if secure {
        value.push_str("; Secure");
    }
    HeaderValue::from_str(&value).expect("cleared cookie is valid header data")
}

/// Keep the throttle map bounded under address churn. Expired entries are
/// dropped first; if the map is still full, the oldest window is evicted.
fn prune_map(map: &mut HashMap<IpAddr, Tracked>, now: Instant, window: Duration) {
    if map.len() < MAX_TRACKED_IPS {
        return;
    }
    map.retain(|_, tracked| {
        tracked.blocked_until.is_some_and(|until| until > now)
            || now.duration_since(tracked.window_start) < window
    });
    while map.len() >= MAX_TRACKED_IPS {
        let Some(oldest) = map
            .iter()
            .min_by_key(|(_, tracked)| tracked.window_start)
            .map(|(ip, _)| *ip)
        else {
            break;
        };
        map.remove(&oldest);
    }
}

fn is_public(path: &str) -> bool {
    !path.starts_with("/api/")
        || matches!(
            path,
            "/api/health"
                | "/api/auth/login"
                | "/api/auth/login/employee"
                | "/api/auth/logout"
                | "/api/auth/me"
        )
}

/// Authentication gate for everything under `/api` except the public routes.
/// A valid session has its identity inserted into the request extensions for
/// handlers to read via the `Auth` extractor. Static assets are public; the UI
/// itself decides what to render.
pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> AppResult<Response> {
    if is_public(req.uri().path()) {
        return Ok(next.run(req).await);
    }
    let token = cookie_token(req.headers()).ok_or(AppError::Unauthorized)?;
    let identity = session_identity(&state.pool, &token)
        .await?
        .ok_or(AppError::Unauthorized)?;
    req.extensions_mut().insert(identity);
    Ok(next.run(req).await)
}

/// Require a custom header on mutating API calls. Combined with `SameSite=Lax`
/// cookies this blocks cross-site form posts; a cross-origin `fetch` cannot add
/// this header without a CORS preflight, which the server never approves.
pub async fn require_client_header(req: Request, next: Next) -> AppResult<Response> {
    let mutating = matches!(
        *req.method(),
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );
    if mutating && req.uri().path().starts_with("/api/") {
        let present = req
            .headers()
            .get("x-daybook-client")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| !v.trim().is_empty());
        if !present {
            return Err(AppError::Forbidden("missing X-Daybook-Client header"));
        }
    }
    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(last: u8) -> IpAddr {
        IpAddr::from([127, 0, 0, last])
    }

    #[test]
    fn password_policy_enforced() {
        assert!(validate_password_policy("short").is_err());
        assert!(validate_password_policy("0123456789").is_ok());
        assert!(validate_password_policy(&"a".repeat(201)).is_err());
        assert!(validate_password_policy("          ").is_err());
    }

    #[test]
    fn hashes_and_verifies_passwords() {
        let hash = hash_password("correct horse battery").unwrap();
        assert!(verify_password("correct horse battery", &hash));
        assert!(!verify_password("wrong", &hash));
        assert!(!verify_password("correct horse battery", "not-a-hash"));
        assert!(hash.starts_with("$argon2"));
    }

    #[test]
    fn tokens_are_opaque_and_hashed_stably() {
        let token = generate_token();
        assert!(token.len() >= 40);
        assert_eq!(hash_token(&token).len(), 64);
        assert_ne!(hash_token(&token), token);
        assert_ne!(generate_token(), token);
    }

    #[test]
    fn limiter_blocks_after_repeated_failures_and_resets_on_success() {
        let limiter =
            LoginLimiter::with_limits(3, Duration::from_secs(60), Duration::from_secs(60));
        assert!(limiter.is_allowed(ip(1)));
        assert!(limiter.is_allowed(ip(2)));

        for _ in 0..2 {
            limiter.record_failure(ip(1));
        }
        assert!(limiter.is_allowed(ip(1)));
        limiter.record_failure(ip(1));
        assert!(!limiter.is_allowed(ip(1)));
        // Other clients are unaffected.
        assert!(limiter.is_allowed(ip(2)));

        limiter.record_success(ip(1));
        assert!(limiter.is_allowed(ip(1)));
    }

    #[test]
    fn limiter_entries_expire() {
        let limiter =
            LoginLimiter::with_limits(1, Duration::from_millis(30), Duration::from_millis(30));
        limiter.record_failure(ip(9));
        assert!(!limiter.is_allowed(ip(9)));
        std::thread::sleep(Duration::from_millis(50));
        assert!(limiter.is_allowed(ip(9)));
    }

    #[test]
    fn limiter_map_is_bounded() {
        let limiter =
            LoginLimiter::with_limits(1, Duration::from_secs(60), Duration::from_secs(60));
        for i in 0..(MAX_TRACKED_IPS as u32 + 32) {
            let octets = i.to_be_bytes();
            limiter.record_failure(IpAddr::from([10, octets[1], octets[2], octets[3]]));
        }
        assert!(limiter.tracked_count() <= MAX_TRACKED_IPS);
    }

    #[test]
    fn reads_session_cookie_from_request_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("other=1; daybook_session=abc123; theme=dark"),
        );
        assert_eq!(cookie_token(&headers).as_deref(), Some("abc123"));

        let mut empty = HeaderMap::new();
        empty.insert(header::COOKIE, HeaderValue::from_static("other=1"));
        assert!(cookie_token(&empty).is_none());
    }

    #[test]
    fn cookie_flags_follow_tls_setting() {
        let plain = session_cookie("tok", 60, false);
        let text = plain.to_str().unwrap();
        assert!(text.contains("HttpOnly"));
        assert!(text.contains("SameSite=Lax"));
        assert!(!text.contains("Secure"));

        let secure = session_cookie("tok", 60, true);
        assert!(secure.to_str().unwrap().contains("; Secure"));
        assert!(
            cleared_session_cookie(true)
                .to_str()
                .unwrap()
                .contains("Max-Age=0")
        );
    }
}
