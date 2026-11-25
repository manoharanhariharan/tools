pub mod auth_routes;
pub mod employees;
pub mod entries;
pub mod health;
pub mod reports;
pub mod sync;
pub mod types;

use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::request::Parts;
use axum::routing::get;
use axum::{Json, Router};
use serde::de::DeserializeOwned;

use crate::AppState;
use crate::error::{AppError, AppResult};

/// JSON extractor that reports failures using our uniform error envelope.
pub struct AppJson<T>(pub T);

impl<T, S> FromRequest<S> for AppJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(AppJson(value)),
            Err(rejection) => {
                tracing::debug!(error = %rejection, "request body rejected");
                Err(AppError::BadRequest("malformed or missing JSON body"))
            }
        }
    }
}

/// Query-string extractor with the same error envelope.
pub struct AppQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for AppQuery<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Query::<T>::from_request_parts(parts, state).await {
            Ok(axum::extract::Query(value)) => Ok(AppQuery(value)),
            Err(rejection) => {
                tracing::debug!(error = %rejection, "query string rejected");
                Err(AppError::BadRequest("invalid query parameters"))
            }
        }
    }
}

/// True when SQLite rejected a write because a unique index was violated.
pub fn is_unique_violation(err: &sqlx::Error) -> bool {
    err.as_database_error()
        .is_some_and(|db_err| db_err.is_unique_violation())
}

/// Resolve an optional `from`/`to` range, defaulting to the current IST month
/// when neither is supplied. Partial ranges are rejected rather than guessed.
pub fn resolve_range(from: Option<&str>, to: Option<&str>) -> AppResult<(String, String)> {
    match (from, to) {
        (Some(from), Some(to)) => {
            let (f, t) = crate::domain::date::validate_range(from, to)
                .map_err(|err| AppError::Validation(vec![err]))?;
            Ok((crate::domain::date::iso(f), crate::domain::date::iso(t)))
        }
        (None, None) => {
            let today = crate::domain::date::today_ist();
            let (f, t) = crate::domain::date::month_range(today.year(), u8::from(today.month()))
                .ok_or_else(|| {
                    AppError::internal(anyhow::anyhow!("cannot resolve the current month"))
                })?;
            Ok((crate::domain::date::iso(f), crate::domain::date::iso(t)))
        }
        _ => Err(AppError::Validation(vec![crate::error::FieldError::new(
            "from",
            "provide both from and to, or neither",
        )])),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health))
        .route("/auth/login", axum::routing::post(auth_routes::login))
        .route(
            "/auth/login/employee",
            axum::routing::post(auth_routes::employee_login),
        )
        .route(
            "/auth/password",
            axum::routing::post(auth_routes::change_password),
        )
        .route("/auth/logout", axum::routing::post(auth_routes::logout))
        .route("/auth/me", get(auth_routes::me))
        .route("/employees", get(employees::list).post(employees::create))
        .route(
            "/employees/{id}",
            get(employees::get_one)
                .patch(employees::update)
                .delete(employees::delete),
        )
        .route(
            "/employees/{id}/pin",
            get(employees::pin_status)
                .put(employees::set_pin)
                .delete(employees::clear_pin),
        )
        .route("/entries", get(entries::list).post(entries::create))
        .route(
            "/entries/{id}",
            get(entries::get_one)
                .patch(entries::update)
                .delete(entries::delete),
        )
        .route("/sync/state", get(sync::state))
        .route("/sync/pull", get(sync::pull))
        .route("/sync/push", axum::routing::post(sync::push))
        .route("/earning-types", get(types::list).post(types::create))
        .route(
            "/earning-types/{id}",
            axum::routing::patch(types::update).delete(types::delete),
        )
        .route("/reports/summary", get(reports::summary))
        .route("/reports/export.csv", get(reports::export_csv))
        .route("/reports/print.html", get(reports::print_html))
        .route("/reports/report.pdf", get(reports::report_pdf))
}
