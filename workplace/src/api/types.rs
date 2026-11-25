use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::api::{AppJson, is_unique_violation};
use crate::auth::Auth;
use crate::domain::now_millis;
use crate::error::{AppError, AppResult};
use crate::repo::types::{self, EarningTypeInput, EarningTypeRow};

pub async fn list(
    State(state): State<AppState>,
    Auth(_auth): Auth,
) -> AppResult<Json<Vec<EarningTypeRow>>> {
    Ok(Json(types::list_live(&state.pool).await?))
}

pub async fn create(
    State(state): State<AppState>,
    Auth(auth): Auth,
    AppJson(input): AppJson<EarningTypeInput>,
) -> AppResult<(StatusCode, Json<EarningTypeRow>)> {
    auth.require_admin()?;
    let validated = types::validate_input(&input).map_err(AppError::Validation)?;
    let id = Uuid::now_v7().to_string();
    let now = now_millis();

    let _gate = state.write_gate.lock().await;
    let mut tx = state.pool.begin().await?;
    match types::insert(&mut tx, &id, &validated, now).await {
        Ok(row) => {
            tx.commit().await?;
            Ok((StatusCode::CREATED, Json(row)))
        }
        Err(err) if is_unique_violation(&err) => {
            drop(tx);
            let mut conn = state.pool.acquire().await?;
            let existing = types::find_live_by_name(&mut conn, &validated.name).await?;
            Err(AppError::conflict(
                "duplicate_type_name",
                "an earning type with this name already exists",
                existing.and_then(|row| serde_json::to_value(row).ok()),
            ))
        }
        Err(err) => Err(err.into()),
    }
}

pub async fn update(
    State(state): State<AppState>,
    Auth(auth): Auth,
    Path(id): Path<String>,
    AppJson(input): AppJson<EarningTypeInput>,
) -> AppResult<Json<EarningTypeRow>> {
    auth.require_admin()?;
    let validated = types::validate_input(&input).map_err(AppError::Validation)?;
    let now = now_millis();

    let _gate = state.write_gate.lock().await;
    let mut tx = state.pool.begin().await?;
    match types::update(&mut tx, &id, &validated, now).await {
        Ok(row) => {
            tx.commit().await?;
            Ok(Json(row))
        }
        Err(sqlx::Error::RowNotFound) => Err(AppError::NotFound("earning type")),
        Err(err) if is_unique_violation(&err) => {
            drop(tx);
            let mut conn = state.pool.acquire().await?;
            let existing = types::find_live_by_name(&mut conn, &validated.name).await?;
            Err(AppError::conflict(
                "duplicate_type_name",
                "an earning type with this name already exists",
                existing.and_then(|row| serde_json::to_value(row).ok()),
            ))
        }
        Err(err) => Err(err.into()),
    }
}

pub async fn delete(
    State(state): State<AppState>,
    Auth(auth): Auth,
    Path(id): Path<String>,
) -> AppResult<Json<EarningTypeRow>> {
    auth.require_admin()?;
    let now = now_millis();
    let _gate = state.write_gate.lock().await;
    let mut tx = state.pool.begin().await?;
    match types::soft_delete(&mut tx, &id, now).await {
        Ok(row) => {
            tx.commit().await?;
            Ok(Json(row))
        }
        Err(sqlx::Error::RowNotFound) => Err(AppError::NotFound("earning type")),
        Err(err) => Err(err.into()),
    }
}
