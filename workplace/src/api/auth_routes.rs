use std::net::SocketAddr;

use axum::Json;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;
use crate::api::AppJson;
use crate::auth;
use crate::db;
use crate::error::{AppError, AppResult, FieldError};

#[derive(Debug, Deserialize)]
pub struct LoginBody {
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordBody {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    AppJson(body): AppJson<LoginBody>,
) -> AppResult<Response> {
    let ip = peer.ip();
    if !state.limiter.is_allowed(ip) {
        return Err(AppError::TooManyRequests);
    }

    let hash: Option<String> =
        sqlx::query_scalar("SELECT value FROM settings WHERE key = 'admin_password_hash'")
            .fetch_optional(&state.pool)
            .await?;
    let Some(hash) = hash else {
        return Err(AppError::internal(anyhow::anyhow!(
            "admin password hash missing from settings"
        )));
    };

    // Argon2 is intentionally expensive; keep it off the async runtime.
    let password = body.password;
    let verified = tokio::task::spawn_blocking(move || auth::verify_password(&password, &hash))
        .await
        .map_err(|err| {
            AppError::internal(anyhow::anyhow!("password verification task failed: {err}"))
        })?;

    if !verified {
        state.limiter.record_failure(ip);
        return Err(AppError::Unauthorized);
    }
    state.limiter.record_success(ip);

    let (token, expires_at) = auth::create_session(&state.pool, state.session_ttl()).await?;
    let cookie = auth::session_cookie(
        &token,
        state.config.session_ttl_secs,
        state.config.secure_cookies(),
    );

    let mut response =
        Json(json!({ "authenticated": true, "expires_at": expires_at })).into_response();
    response.headers_mut().insert(header::SET_COOKIE, cookie);
    Ok(response)
}

/// Change the admin password from an authenticated session. The current
/// password is re-verified, and other devices are signed out.
pub async fn change_password(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    AppJson(body): AppJson<ChangePasswordBody>,
) -> AppResult<Json<serde_json::Value>> {
    let ip = peer.ip();
    if !state.limiter.is_allowed(ip) {
        return Err(AppError::TooManyRequests);
    }
    if let Err(message) = auth::validate_password_policy(&body.new_password) {
        return Err(AppError::Validation(vec![FieldError::new(
            "new_password",
            message,
        )]));
    }
    if body.new_password == body.current_password {
        return Err(AppError::Validation(vec![FieldError::new(
            "new_password",
            "must be different from the current password",
        )]));
    }

    let token = auth::cookie_token(&headers).ok_or(AppError::Unauthorized)?;
    let hash: Option<String> =
        sqlx::query_scalar("SELECT value FROM settings WHERE key = 'admin_password_hash'")
            .fetch_optional(&state.pool)
            .await?;
    let Some(hash) = hash else {
        return Err(AppError::internal(anyhow::anyhow!(
            "admin password hash missing from settings"
        )));
    };

    // Argon2 is intentionally expensive; keep it off the async runtime.
    let current = body.current_password;
    let verified = tokio::task::spawn_blocking(move || auth::verify_password(&current, &hash))
        .await
        .map_err(|err| {
            AppError::internal(anyhow::anyhow!("password verification task failed: {err}"))
        })?;
    if !verified {
        state.limiter.record_failure(ip);
        return Err(AppError::Validation(vec![FieldError::new(
            "current_password",
            "is incorrect",
        )]));
    }
    state.limiter.record_success(ip);

    let keep = auth::hash_token(&token);
    db::set_admin_password_keep_session(&state.pool, &body.new_password, Some(&keep)).await?;
    Ok(Json(json!({ "updated": true })))
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Response> {
    if let Some(token) = auth::cookie_token(&headers) {
        auth::delete_session(&state.pool, &token).await?;
    }
    let mut response = Json(json!({ "authenticated": false })).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        auth::cleared_session_cookie(state.config.secure_cookies()),
    );
    Ok(response)
}

pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<(StatusCode, Json<MeResponse>)> {
    let expires_at = match auth::cookie_token(&headers) {
        Some(token) => auth::session_expires_at(&state.pool, &token).await?,
        None => None,
    };
    Ok((
        StatusCode::OK,
        Json(MeResponse {
            authenticated: expires_at.is_some(),
            expires_at,
        }),
    ))
}
