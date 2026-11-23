//! Daybook: in-house daily employee earnings ledger.
//!
//! The crate is split so that business rules (money, pay computation, report
//! shaping) are pure and unit-testable, while HTTP and persistence concerns
//! live behind small modules.

pub mod api;
pub mod auth;
pub mod cert;
pub mod config;
pub mod db;
pub mod domain;
pub mod error;
pub mod reports;
pub mod repo;
pub mod web;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use sqlx::SqlitePool;
use tower_http::set_header::SetResponseHeaderLayer;

use crate::auth::LoginLimiter;
use crate::config::Config;
use crate::reports::pdf::PdfFont;

/// Shared, cheaply cloneable application state.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Arc<Config>,
    pub limiter: Arc<LoginLimiter>,
    pub pdf_font: Option<Arc<PdfFont>>,
    pub started_at: i64,
    /// Serialises write transactions. SQLite allows one writer anyway; taking
    /// an async gate up front avoids `SQLITE_BUSY` upgrade deadlocks between
    /// concurrent request handlers in this single-server deployment.
    pub write_gate: Arc<tokio::sync::Mutex<()>>,
}

impl AppState {
    pub fn new(pool: SqlitePool, config: Arc<Config>) -> Self {
        let pdf_font = match PdfFont::discover(config.pdf_font.as_deref()) {
            Ok(font) => {
                tracing::info!(path = %font.path.display(), "PDF font loaded");
                Some(Arc::new(font))
            }
            Err(err) => {
                tracing::warn!(error = %err, "PDF font unavailable; falling back to built-in Helvetica (INR rendered as 'Rs')");
                None
            }
        };
        Self {
            pool,
            config,
            limiter: Arc::new(LoginLimiter::new()),
            pdf_font,
            started_at: crate::domain::now_millis(),
            write_gate: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub fn session_ttl(&self) -> Duration {
        Duration::from_secs(self.config.session_ttl_secs as u64)
    }
}

/// Build the complete HTTP router. `pub` so integration tests can drive it
/// directly through `tower::ServiceExt` without binding a socket.
pub fn build_router(state: AppState) -> Router {
    let api = api::router();

    Router::new()
        .nest("/api", api)
        .fallback(web::static_handler)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require_admin,
        ))
        .layer(axum::middleware::from_fn(auth::require_client_header))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            axum::http::HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::X_FRAME_OPTIONS,
            axum::http::HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::REFERRER_POLICY,
            axum::http::HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::CONTENT_SECURITY_POLICY,
            axum::http::HeaderValue::from_static(
                "default-src 'self'; img-src 'self' data:; script-src 'self'; \
                 style-src 'self' 'unsafe-inline'; connect-src 'self'; \
                 manifest-src 'self'; base-uri 'none'; form-action 'none'; \
                 frame-ancestors 'none'",
            ),
        ))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state)
}
