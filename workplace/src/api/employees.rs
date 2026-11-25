use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::AppState;
use crate::api::AppJson;
use crate::auth::{self, Auth};
use crate::db;
use crate::domain::now_millis;
use crate::error::{AppError, AppResult, FieldError};
use crate::repo::employees::{self, EmployeeInput, EmployeeRow};

fn is_unique_violation(err: &sqlx::Error) -> bool {
    err.as_database_error()
        .is_some_and(|db_err| db_err.is_unique_violation())
}

fn view_json(row: &EmployeeRow) -> Option<Value> {
    serde_json::to_value(row).ok()
}

pub async fn list(
    State(state): State<AppState>,
    Auth(auth): Auth,
) -> AppResult<Json<Vec<EmployeeRow>>> {
    auth.require_admin()?;
    Ok(Json(employees::list_live(&state.pool).await?))
}

pub async fn get_one(
    State(state): State<AppState>,
    Auth(auth): Auth,
    Path(id): Path<String>,
) -> AppResult<Json<EmployeeRow>> {
    auth.require_admin()?;
    let row = employees::get_live(&state.pool, &id)
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    Ok(Json(row))
}

pub async fn create(
    State(state): State<AppState>,
    Auth(auth): Auth,
    AppJson(input): AppJson<EmployeeInput>,
) -> AppResult<(StatusCode, Json<EmployeeRow>)> {
    auth.require_admin()?;
    let validated = employees::validate_input(&input).map_err(AppError::Validation)?;
    let id = Uuid::now_v7().to_string();
    let now = now_millis();

    let _gate = state.write_gate.lock().await;
    let mut tx = state.pool.begin().await?;
    let seq = db::next_seq(&mut tx).await?;
    let row = match employees::insert(&mut tx, &id, &validated, now, seq).await {
        Ok(row) => row,
        Err(err) if is_unique_violation(&err) => {
            drop(tx);
            let existing = match &validated.code {
                Some(code) => employees::get_live_by_code(&state.pool, code).await?,
                None => None,
            };
            return Err(AppError::conflict(
                "duplicate_code",
                "an active employee already uses this code",
                existing.as_ref().and_then(view_json),
            ));
        }
        Err(err) => return Err(err.into()),
    };
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(row)))
}

pub async fn update(
    State(state): State<AppState>,
    Auth(auth): Auth,
    Path(id): Path<String>,
    AppJson(input): AppJson<EmployeeInput>,
) -> AppResult<Json<EmployeeRow>> {
    auth.require_admin()?;
    let validated = employees::validate_input(&input).map_err(AppError::Validation)?;

    // Reject unknown/deleted ids before opening a write transaction.
    employees::get_live(&state.pool, &id)
        .await?
        .ok_or(AppError::NotFound("employee"))?;

    let now = now_millis();
    let _gate = state.write_gate.lock().await;
    let mut tx = state.pool.begin().await?;
    let seq = db::next_seq(&mut tx).await?;
    let row = match employees::update(&mut tx, &id, &validated, now, seq).await {
        Ok(row) => row,
        Err(sqlx::Error::RowNotFound) => return Err(AppError::NotFound("employee")),
        Err(err) if is_unique_violation(&err) => {
            drop(tx);
            let existing = match &validated.code {
                Some(code) => employees::get_live_by_code(&state.pool, code).await?,
                None => None,
            };
            return Err(AppError::conflict(
                "duplicate_code",
                "an active employee already uses this code",
                existing.as_ref().and_then(view_json),
            ));
        }
        Err(err) => return Err(err.into()),
    };
    tx.commit().await?;
    Ok(Json(row))
}

pub async fn delete(
    State(state): State<AppState>,
    Auth(auth): Auth,
    Path(id): Path<String>,
) -> AppResult<Json<EmployeeRow>> {
    auth.require_admin()?;
    employees::get_live(&state.pool, &id)
        .await?
        .ok_or(AppError::NotFound("employee"))?;

    let now = now_millis();
    let _gate = state.write_gate.lock().await;
    let mut tx = state.pool.begin().await?;
    let seq = db::next_seq(&mut tx).await?;
    let row = match employees::soft_delete(&mut tx, &id, now, seq).await {
        Ok(row) => row,
        Err(sqlx::Error::RowNotFound) => return Err(AppError::NotFound("employee")),
        Err(err) => return Err(err.into()),
    };
    tx.commit().await?;
    Ok(Json(row))
}

#[derive(Debug, Deserialize)]
pub struct PinBody {
    pub pin: String,
}

/// Admin-only: report whether an employee has a login PIN set.
pub async fn pin_status(
    State(state): State<AppState>,
    Auth(auth): Auth,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    auth.require_admin()?;
    employees::get_live(&state.pool, &id)
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    Ok(Json(
        serde_json::json!({ "has_pin": employees::has_pin(&state.pool, &id).await? }),
    ))
}

/// Admin-only: set (or replace) an employee's 4-digit login PIN.
pub async fn set_pin(
    State(state): State<AppState>,
    Auth(auth): Auth,
    Path(id): Path<String>,
    AppJson(body): AppJson<PinBody>,
) -> AppResult<Json<Value>> {
    auth.require_admin()?;
    employees::get_live(&state.pool, &id)
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    auth::validate_pin_policy(&body.pin)
        .map_err(|message| AppError::Validation(vec![FieldError::new("pin", message)]))?;

    let pin = body.pin;
    let hash = tokio::task::spawn_blocking(move || auth::hash_password(&pin))
        .await
        .map_err(|err| AppError::internal(anyhow::anyhow!("PIN hashing task failed: {err}")))??;

    let now = now_millis();
    let _gate = state.write_gate.lock().await;
    let mut tx = state.pool.begin().await?;
    employees::set_pin(&mut tx, &id, &hash, now).await?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({ "updated": true })))
}

/// Admin-only: remove an employee's login PIN.
pub async fn clear_pin(
    State(state): State<AppState>,
    Auth(auth): Auth,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    auth.require_admin()?;
    employees::get_live(&state.pool, &id)
        .await?
        .ok_or(AppError::NotFound("employee"))?;

    let now = now_millis();
    let _gate = state.write_gate.lock().await;
    let mut tx = state.pool.begin().await?;
    employees::clear_pin(&mut tx, &id, now).await?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({ "updated": true })))
}
