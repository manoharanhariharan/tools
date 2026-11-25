use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::api::{AppJson, resolve_range};
use crate::auth::Auth;
use crate::db;
use crate::domain::now_millis;
use crate::error::{AppError, AppResult};
use crate::repo::audit;
use crate::repo::employees;
use crate::repo::entries::{self, EntryInput, EntryRow};

/// Human-readable one-line summary for the audit trail.
fn entry_summary(action: &str, view: &EntryRow) -> String {
    let date = crate::domain::date::parse_iso(&view.work_date)
        .map(crate::domain::date::display)
        .unwrap_or_else(|| view.work_date.clone());
    format!(
        "Day sheet \"{}\" {} {} (business {})",
        view.employee_name,
        date,
        action,
        crate::domain::money::format_inr(view.business_minor)
    )
}

#[derive(Debug, Deserialize)]
pub struct EntriesQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub employee_id: Option<String>,
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    err.as_database_error()
        .is_some_and(|db_err| db_err.is_unique_violation())
}

fn duplicate_entry_error(existing: &EntryRow) -> AppError {
    AppError::conflict(
        "duplicate_entry_date",
        "an entry already exists for this employee and date",
        serde_json::to_value(existing).ok(),
    )
}

pub async fn list(
    State(state): State<AppState>,
    Auth(auth): Auth,
    Query(query): Query<EntriesQuery>,
) -> AppResult<Json<Vec<EntryRow>>> {
    let (from, to) = resolve_range(query.from.as_deref(), query.to.as_deref())?;
    // Employees can only read their own day sheets; admins may filter by any id.
    let employee_filter = if auth.is_admin() {
        query.employee_id.as_deref()
    } else {
        auth.employee_scope()
    };
    let rows = entries::list_live(&state.pool, &from, &to, employee_filter).await?;
    Ok(Json(rows))
}

pub async fn get_one(
    State(state): State<AppState>,
    Auth(auth): Auth,
    Path(id): Path<String>,
) -> AppResult<Json<EntryRow>> {
    let mut conn = state.pool.acquire().await?;
    let row = entries::find_live(&mut conn, &id)
        .await?
        .ok_or(AppError::NotFound("entry"))?;
    ensure_entry_visible(&auth, &row)?;
    Ok(Json(row))
}

/// Employees may only read or modify their own day sheets.
fn ensure_entry_visible(auth: &crate::auth::Identity, row: &EntryRow) -> AppResult<()> {
    if let Some(scope) = auth.employee_scope()
        && row.employee_id != scope
    {
        return Err(AppError::NotFound("entry"));
    }
    Ok(())
}

pub async fn create(
    State(state): State<AppState>,
    Auth(auth): Auth,
    AppJson(mut input): AppJson<EntryInput>,
) -> AppResult<(StatusCode, Json<EntryRow>)> {
    // Employees always create their own day sheets, regardless of the body.
    if let Some(scope) = auth.employee_scope() {
        input.employee_id = scope.to_string();
    }
    let (actor_role, actor_id, actor_name) = auth.actor();
    let id = Uuid::now_v7().to_string();
    let now = now_millis();
    let _gate = state.write_gate.lock().await;
    let mut tx = state.pool.begin().await?;

    let employee = employees::find(&mut tx, input.employee_id.trim())
        .await?
        .filter(|employee| employee.deleted_at.is_none())
        .ok_or(AppError::NotFound("employee"))?;

    let prepared = entries::prepare(&mut tx, &input).await?;
    if prepared.employee_id != employee.id {
        return Err(AppError::Validation(vec![crate::error::FieldError::new(
            "employee_id",
            "does not match the selected employee",
        )]));
    }

    if let Some(existing) =
        entries::find_live_by_employee_date(&mut tx, &prepared.employee_id, &prepared.work_date)
            .await?
    {
        return Err(duplicate_entry_error(&existing));
    }

    let seq = db::next_seq(&mut tx).await?;
    match entries::insert(&mut tx, &id, &prepared, now, seq).await {
        Ok(_) => {}
        Err(err) if is_unique_violation(&err) => {
            drop(tx);
            let mut conn = state.pool.acquire().await?;
            let existing = entries::find_live_by_employee_date(
                &mut conn,
                &prepared.employee_id,
                &prepared.work_date,
            )
            .await?;
            return Err(match existing {
                Some(row) => duplicate_entry_error(&row),
                None => AppError::conflict(
                    "duplicate_entry_date",
                    "an entry already exists for this employee and date",
                    None,
                ),
            });
        }
        Err(err) => return Err(err.into()),
    }

    let view = entries::find_any_view(&mut tx, &id)
        .await?
        .ok_or_else(|| AppError::internal(anyhow::anyhow!("inserted entry disappeared")))?;
    audit::record(
        &mut tx,
        now,
        actor_role,
        actor_id,
        actor_name,
        "entry",
        &id,
        "create",
        &entry_summary("created", &view),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(view)))
}

pub async fn update(
    State(state): State<AppState>,
    Auth(auth): Auth,
    Path(id): Path<String>,
    AppJson(mut input): AppJson<EntryInput>,
) -> AppResult<Json<EntryRow>> {
    // Employees always update their own day sheets, regardless of the body.
    if let Some(scope) = auth.employee_scope() {
        input.employee_id = scope.to_string();
    }
    let (actor_role, actor_id, actor_name) = auth.actor();
    let now = now_millis();
    let _gate = state.write_gate.lock().await;
    let mut tx = state.pool.begin().await?;

    let existing = entries::find_live(&mut tx, &id)
        .await?
        .ok_or(AppError::NotFound("entry"))?;
    ensure_entry_visible(&auth, &existing)?;

    let employee = employees::find(&mut tx, input.employee_id.trim())
        .await?
        .filter(|employee| employee.deleted_at.is_none())
        .ok_or(AppError::NotFound("employee"))?;

    let prepared = entries::prepare(&mut tx, &input).await?;
    if prepared.employee_id != employee.id {
        return Err(AppError::Validation(vec![crate::error::FieldError::new(
            "employee_id",
            "does not match the selected employee",
        )]));
    }

    if let Some(other) =
        entries::find_live_by_employee_date(&mut tx, &prepared.employee_id, &prepared.work_date)
            .await?
        && other.id != id
    {
        return Err(duplicate_entry_error(&other));
    }

    let seq = db::next_seq(&mut tx).await?;
    match entries::update(&mut tx, &id, &prepared, now, seq).await {
        Ok(_) => {}
        Err(sqlx::Error::RowNotFound) => return Err(AppError::NotFound("entry")),
        Err(err) if is_unique_violation(&err) => {
            return Err(AppError::conflict(
                "duplicate_entry_date",
                "an entry already exists for this employee and date",
                None,
            ));
        }
        Err(err) => return Err(err.into()),
    }

    let view = entries::find_any_view(&mut tx, &id)
        .await?
        .ok_or_else(|| AppError::internal(anyhow::anyhow!("updated entry disappeared")))?;
    audit::record(
        &mut tx,
        now,
        actor_role,
        actor_id,
        actor_name,
        "entry",
        &id,
        "update",
        &entry_summary("updated", &view),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(view))
}

pub async fn delete(
    State(state): State<AppState>,
    Auth(auth): Auth,
    Path(id): Path<String>,
) -> AppResult<Json<EntryRow>> {
    let (actor_role, actor_id, actor_name) = auth.actor();
    let now = now_millis();
    let _gate = state.write_gate.lock().await;
    let mut tx = state.pool.begin().await?;
    let existing = entries::find_live(&mut tx, &id)
        .await?
        .ok_or(AppError::NotFound("entry"))?;
    ensure_entry_visible(&auth, &existing)?;

    let seq = db::next_seq(&mut tx).await?;
    let row = match entries::soft_delete(&mut tx, &id, now, seq).await {
        Ok(row) => row,
        Err(sqlx::Error::RowNotFound) => return Err(AppError::NotFound("entry")),
        Err(err) => return Err(err.into()),
    };
    audit::record(
        &mut tx,
        now,
        actor_role,
        actor_id,
        actor_name,
        "entry",
        &id,
        "delete",
        &entry_summary("deleted", &existing),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(row))
}
