use std::net::SocketAddr;

use axum::Json;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;
use crate::api::AppJson;
use crate::auth::{self, Auth, SessionRole};
use crate::db;
use crate::error::{AppError, AppResult, FieldError};
use crate::repo::employees;

#[derive(Debug, Deserialize)]
pub struct LoginBody {
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct EmployeeLoginBody {
    pub code: String,
    pub pin: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordBody {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub authenticated: bool,
    pub role: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub employee_id: Option<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
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

    let (token, expires_at) = auth::create_session(
        &state.pool,
        state.session_ttl(),
        SessionRole::Admin,
        None,
        "Administrator",
    )
    .await?;
    let cookie = auth::session_cookie(
        &token,
        state.config.session_ttl_secs,
        state.config.secure_cookies(),
    );

    let mut response = Json(json!({
        "authenticated": true,
        "role": "admin",
        "name": "Administrator",
        "expires_at": expires_at,
    }))
    .into_response();
    response.headers_mut().insert(header::SET_COOKIE, cookie);
    Ok(response)
}

/// Employee sign-in: live employee code + 4-digit PIN. Scoped to that employee.
pub async fn employee_login(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    AppJson(body): AppJson<EmployeeLoginBody>,
) -> AppResult<Response> {
    let ip = peer.ip();
    if !state.limiter.is_allowed(ip) {
        return Err(AppError::TooManyRequests);
    }

    let code = body.code.trim();
    let employee = employees::get_live_by_code(&state.pool, code)
        .await?
        .ok_or_else(|| {
            AppError::Validation(vec![FieldError::new("code", "unknown employee code")])
        })?;

    let Some(hash) = employee.pin_hash.clone() else {
        state.limiter.record_failure(ip);
        return Err(AppError::Validation(vec![FieldError::new(
            "pin",
            "no PIN is set for this employee",
        )]));
    };

    let pin = body.pin;
    let verified = tokio::task::spawn_blocking(move || auth::verify_password(&pin, &hash))
        .await
        .map_err(|err| {
            AppError::internal(anyhow::anyhow!("PIN verification task failed: {err}"))
        })?;

    if !verified {
        state.limiter.record_failure(ip);
        return Err(AppError::Validation(vec![FieldError::new(
            "pin",
            "incorrect PIN",
        )]));
    }
    state.limiter.record_success(ip);

    let (token, expires_at) = auth::create_session(
        &state.pool,
        state.session_ttl(),
        SessionRole::Employee,
        Some(&employee.id),
        &employee.name,
    )
    .await?;
    let cookie = auth::session_cookie(
        &token,
        state.config.session_ttl_secs,
        state.config.secure_cookies(),
    );

    let mut response = Json(json!({
        "authenticated": true,
        "role": "employee",
        "employee_id": employee.id,
        "name": employee.name,
        "code": employee.code,
        "expires_at": expires_at,
    }))
    .into_response();
    response.headers_mut().insert(header::SET_COOKIE, cookie);
    Ok(response)
}

/// Change the admin password from an authenticated session. The current
/// password is re-verified, and other devices are signed out.
pub async fn change_password(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Auth(auth): Auth,
    headers: HeaderMap,
    AppJson(body): AppJson<ChangePasswordBody>,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_admin()?;
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
    let identity = match auth::cookie_token(&headers) {
        Some(token) => auth::session_identity(&state.pool, &token).await?,
        None => None,
    };
    match identity {
        Some(identity) => {
            let code = match &identity.employee_id {
                Some(employee_id) => employees::get_live(&state.pool, employee_id)
                    .await?
                    .and_then(|employee| employee.code),
                None => None,
            };
            Ok((
                StatusCode::OK,
                Json(MeResponse {
                    authenticated: true,
                    role: identity.role.as_str(),
                    employee_id: identity.employee_id,
                    name: identity.name,
                    code,
                    expires_at: Some(identity.expires_at),
                }),
            ))
        }
        None => Ok((
            StatusCode::OK,
            Json(MeResponse {
                authenticated: false,
                role: "admin",
                employee_id: None,
                name: String::new(),
                code: None,
                expires_at: None,
            }),
        )),
    }
}
