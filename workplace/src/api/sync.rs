//! Sync endpoints.
//!
//! Pull is cursor-based over the global `seq` counter; push reports a per-change
//! result and never silently loses a conflicting edit. See `repo::sync` for the
//! decision table.

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Sqlite, Transaction};
use uuid::Uuid;

use crate::AppState;
use crate::api::{AppJson, is_unique_violation};
use crate::auth::Auth;
use crate::db;
use crate::domain::now_millis;
use crate::error::{AppError, AppResult, FieldError};
use crate::repo::audit;
use crate::repo::employees::{self, EmployeeInput, EmployeeRow};
use crate::repo::entries::{self, EntryInput, EntryRow};
use crate::repo::sync::{self, Decision, ExistingState};

const DEFAULT_PULL_LIMIT: i64 = 200;
const MAX_PULL_LIMIT: i64 = 500;
const MAX_PUSH_CHANGES: usize = 200;

#[derive(Debug, Deserialize)]
pub struct PullQuery {
    pub since: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PullResponse {
    /// Highest sequence included in this page; pass as `since` next time.
    pub cursor: i64,
    /// Highest sequence that exists on the server right now.
    pub latest: i64,
    pub has_more: bool,
    pub employees: Vec<EmployeeRow>,
    pub entries: Vec<EntryRow>,
}

#[derive(Debug, Serialize)]
pub struct StateResponse {
    pub cursor: i64,
    pub server_time: i64,
}

pub async fn state(
    State(state): State<AppState>,
    Auth(auth): Auth,
) -> AppResult<Json<StateResponse>> {
    auth.require_admin()?;
    Ok(Json(StateResponse {
        cursor: db::current_seq(&state.pool).await?,
        server_time: now_millis(),
    }))
}

pub async fn pull(
    State(state): State<AppState>,
    Auth(auth): Auth,
    Query(query): Query<PullQuery>,
) -> AppResult<Json<PullResponse>> {
    auth.require_admin()?;
    let since = query.since.unwrap_or(0).max(0);
    let limit = query
        .limit
        .unwrap_or(DEFAULT_PULL_LIMIT)
        .clamp(1, MAX_PULL_LIMIT);

    // One read transaction = one consistent snapshot, even if a write lands
    // between the cursor query and hydration.
    let mut tx = state.pool.begin().await?;

    let seqs: Vec<i64> = sqlx::query_scalar(
        "SELECT seq FROM (
            SELECT seq FROM employees WHERE seq > ?
            UNION ALL
            SELECT seq FROM entries WHERE seq > ?
         ) ORDER BY seq ASC LIMIT ?",
    )
    .bind(since)
    .bind(since)
    .bind(limit)
    .fetch_all(&mut *tx)
    .await?;

    let cursor = seqs.last().copied().unwrap_or(since);

    let employees = if cursor > since {
        sqlx::query_as::<_, EmployeeRow>(
            "SELECT * FROM employees WHERE seq > ? AND seq <= ? ORDER BY seq ASC",
        )
        .bind(since)
        .bind(cursor)
        .fetch_all(&mut *tx)
        .await?
    } else {
        Vec::new()
    };

    let mut entries = if cursor > since {
        sqlx::query_as::<_, EntryRow>(
            "SELECT * FROM entries WHERE seq > ? AND seq <= ? ORDER BY seq ASC",
        )
        .bind(since)
        .bind(cursor)
        .fetch_all(&mut *tx)
        .await?
    } else {
        Vec::new()
    };
    entries::hydrate_lines(&mut tx, &mut entries).await?;

    let has_more: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM employees WHERE seq > ?
            UNION ALL
            SELECT 1 FROM entries WHERE seq > ?
         )",
    )
    .bind(cursor)
    .bind(cursor)
    .fetch_one(&mut *tx)
    .await?;

    let latest = db::current_seq_tx(&mut tx).await?;
    tx.commit().await?;

    Ok(Json(PullResponse {
        cursor,
        latest,
        has_more,
        employees,
        entries,
    }))
}

#[derive(Debug, Deserialize)]
pub struct PushBody {
    #[serde(default)]
    pub device_id: Option<String>,
    pub changes: Vec<Change>,
}

#[derive(Debug, Deserialize)]
pub struct Change {
    pub entity: String,
    pub id: String,
    #[serde(default)]
    pub base_seq: i64,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub data: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct ChangeResult {
    pub entity: String,
    pub id: String,
    /// "applied" | "noop" | "conflict" | "invalid"
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct PushResponse {
    pub cursor: i64,
    pub results: Vec<ChangeResult>,
}

fn applied(entity: &str, id: &str, server: Value) -> ChangeResult {
    ChangeResult {
        entity: entity.to_string(),
        id: id.to_string(),
        status: "applied",
        conflict: None,
        server: Some(server),
        error: None,
    }
}

fn noop(entity: &str, id: &str, server: Option<Value>) -> ChangeResult {
    ChangeResult {
        entity: entity.to_string(),
        id: id.to_string(),
        status: "noop",
        conflict: None,
        server,
        error: None,
    }
}

fn conflict(entity: &str, id: &str, code: &'static str, server: Option<Value>) -> ChangeResult {
    ChangeResult {
        entity: entity.to_string(),
        id: id.to_string(),
        status: "conflict",
        conflict: Some(code),
        server,
        error: None,
    }
}

fn invalid(entity: &str, id: &str, message: &str, fields: Vec<FieldError>) -> ChangeResult {
    ChangeResult {
        entity: entity.to_string(),
        id: id.to_string(),
        status: "invalid",
        conflict: None,
        server: None,
        error: Some(json!({
            "code": "validation_failed",
            "message": message,
            "fields": fields,
        })),
    }
}

fn row_json<T: Serialize>(row: &T) -> Value {
    serde_json::to_value(row).unwrap_or(Value::Null)
}

fn entry_audit_summary(action: &str, work_date: &str, name: Option<&str>) -> String {
    let date = crate::domain::date::parse_iso(work_date)
        .map(crate::domain::date::display)
        .unwrap_or_else(|| work_date.to_string());
    let who = name.unwrap_or("unknown employee");
    format!("Day sheet \"{who}\" {date} {action}")
}

pub async fn push(
    State(state): State<AppState>,
    Auth(auth): Auth,
    AppJson(body): AppJson<PushBody>,
) -> AppResult<Json<PushResponse>> {
    auth.require_admin()?;
    if body.changes.len() > MAX_PUSH_CHANGES {
        return Err(AppError::BadRequest(
            "too many changes in one push (max 200)",
        ));
    }
    if let Some(device) = &body.device_id
        && device.chars().count() > 64
    {
        return Err(AppError::BadRequest("device_id is too long"));
    }

    let now = now_millis();
    let _gate = state.write_gate.lock().await;
    let mut tx = state.pool.begin().await?;
    let mut results = Vec::with_capacity(body.changes.len());

    // Employees are applied first so an offline bundle can contain an employee
    // and their first entry in any order.
    for change in body.changes.iter().filter(|c| c.entity == "employee") {
        results.push(apply_employee(&mut tx, change, now, &auth).await?);
    }
    for change in body.changes.iter().filter(|c| c.entity == "entry") {
        results.push(apply_entry(&mut tx, change, now, &auth).await?);
    }
    for change in body
        .changes
        .iter()
        .filter(|c| c.entity != "employee" && c.entity != "entry")
    {
        results.push(invalid(
            &change.entity,
            &change.id,
            "unknown entity (expected employee or entry)",
            vec![FieldError::new("entity", "must be employee or entry")],
        ));
    }

    let cursor = db::current_seq_tx(&mut tx).await?;
    tx.commit().await?;

    Ok(Json(PushResponse { cursor, results }))
}

async fn apply_employee(
    tx: &mut Transaction<'_, Sqlite>,
    change: &Change,
    now: i64,
    actor: &crate::auth::Identity,
) -> AppResult<ChangeResult> {
    let (actor_role, actor_id, actor_name) = actor.actor();
    let id = change.id.trim();
    if Uuid::parse_str(id).is_err() {
        return Ok(invalid(
            "employee",
            id,
            "id must be a UUID",
            vec![FieldError::new("id", "must be a UUID")],
        ));
    }

    let existing = employees::find(tx, id).await?;

    if change.deleted {
        let decision = sync::decide(
            existing.as_ref().map(ExistingState::from),
            change.base_seq,
            change.force,
            true,
            false,
        );
        return match decision {
            Decision::Noop => Ok(noop("employee", id, existing.as_ref().map(row_json))),
            Decision::Conflict(code) => Ok(conflict(
                "employee",
                id,
                code,
                existing.as_ref().map(row_json),
            )),
            Decision::Apply => {
                let seq = db::next_seq(tx).await?;
                match employees::mark_deleted(tx, id, now, seq).await? {
                    Some(row) => {
                        audit::record(
                            tx,
                            now,
                            actor_role,
                            actor_id,
                            actor_name,
                            "employee",
                            id,
                            "delete",
                            &format!("Employee \"{}\" deleted", row.name),
                        )
                        .await?;
                        Ok(applied("employee", id, row_json(&row)))
                    }
                    None => Ok(noop("employee", id, existing.as_ref().map(row_json))),
                }
            }
        };
    }

    let Some(data) = change.data.as_ref() else {
        return Ok(invalid(
            "employee",
            id,
            "employee data is required",
            vec![FieldError::new("data", "required")],
        ));
    };
    let input: EmployeeInput = match serde_json::from_value(data.clone()) {
        Ok(input) => input,
        Err(err) => {
            return Ok(invalid(
                "employee",
                id,
                "malformed employee data",
                vec![FieldError::new("data", err.to_string())],
            ));
        }
    };
    let validated = match employees::validate_input(&input) {
        Ok(validated) => validated,
        Err(fields) => return Ok(invalid("employee", id, "some fields are invalid", fields)),
    };

    let content_equal = existing
        .as_ref()
        .is_some_and(|row| sync::employee_content_equal(row, &validated));
    let decision = sync::decide(
        existing.as_ref().map(ExistingState::from),
        change.base_seq,
        change.force,
        false,
        content_equal,
    );

    match decision {
        Decision::Noop => Ok(noop("employee", id, existing.as_ref().map(row_json))),
        Decision::Conflict(code) => Ok(conflict(
            "employee",
            id,
            code,
            existing.as_ref().map(row_json),
        )),
        Decision::Apply => {
            let created_at = existing.as_ref().map(|row| row.created_at).unwrap_or(now);
            let seq = db::next_seq(tx).await?;
            match employees::upsert(tx, id, &validated, created_at, now, None, seq).await {
                Ok(row) => {
                    let (action, summary) = if existing.is_some() {
                        ("update", format!("Employee \"{}\" updated", row.name))
                    } else {
                        ("create", format!("Employee \"{}\" created", row.name))
                    };
                    audit::record(
                        tx, now, actor_role, actor_id, actor_name, "employee", id, action, &summary,
                    )
                    .await?;
                    Ok(applied("employee", id, row_json(&row)))
                }
                Err(err) if is_unique_violation(&err) => {
                    let existing = match &validated.code {
                        Some(code) => employees::find_live_by_code(tx, code).await?,
                        None => None,
                    };
                    Ok(conflict(
                        "employee",
                        id,
                        "duplicate_code",
                        existing.as_ref().map(row_json),
                    ))
                }
                Err(err) => Err(err.into()),
            }
        }
    }
}

async fn apply_entry(
    tx: &mut Transaction<'_, Sqlite>,
    change: &Change,
    now: i64,
    actor: &crate::auth::Identity,
) -> AppResult<ChangeResult> {
    let (actor_role, actor_id, actor_name) = actor.actor();
    let id = change.id.trim();
    if Uuid::parse_str(id).is_err() {
        return Ok(invalid(
            "entry",
            id,
            "id must be a UUID",
            vec![FieldError::new("id", "must be a UUID")],
        ));
    }

    let existing = entries::find(tx, id).await?;

    if change.deleted {
        let decision = sync::decide(
            existing.as_ref().map(ExistingState::from),
            change.base_seq,
            change.force,
            true,
            false,
        );
        return match decision {
            Decision::Noop => Ok(noop("entry", id, existing.as_ref().map(row_json))),
            Decision::Conflict(code) => {
                Ok(conflict("entry", id, code, existing.as_ref().map(row_json)))
            }
            Decision::Apply => {
                let seq = db::next_seq(tx).await?;
                match entries::mark_deleted(tx, id, now, seq).await? {
                    Some(row) => {
                        let existing_row = existing.as_ref().expect("delete apply implies a row");
                        let name = employees::find(tx, &existing_row.employee_id)
                            .await?
                            .map(|employee| employee.name);
                        audit::record(
                            tx,
                            now,
                            actor_role,
                            actor_id,
                            actor_name,
                            "entry",
                            id,
                            "delete",
                            &entry_audit_summary(
                                "deleted",
                                &existing_row.work_date,
                                name.as_deref(),
                            ),
                        )
                        .await?;
                        Ok(applied("entry", id, row_json(&row)))
                    }
                    None => Ok(noop("entry", id, existing.as_ref().map(row_json))),
                }
            }
        };
    }

    let Some(data) = change.data.as_ref() else {
        return Ok(invalid(
            "entry",
            id,
            "entry data is required",
            vec![FieldError::new("data", "required")],
        ));
    };
    let input: EntryInput = match serde_json::from_value(data.clone()) {
        Ok(input) => input,
        Err(err) => {
            return Ok(invalid(
                "entry",
                id,
                "malformed entry data",
                vec![FieldError::new("data", err.to_string())],
            ));
        }
    };

    let employee = employees::find(tx, input.employee_id.trim()).await?;
    if employee
        .as_ref()
        .is_none_or(|employee| employee.deleted_at.is_some())
    {
        return Ok(invalid(
            "entry",
            id,
            "employee does not exist or was deleted",
            vec![FieldError::new(
                "employee_id",
                "unknown or deleted employee",
            )],
        ));
    }
    let entry_employee_name = employee.as_ref().map(|employee| employee.name.as_str());

    let prepared = match entries::prepare(tx, &input).await {
        Ok(prepared) => prepared,
        Err(AppError::Validation(fields)) => {
            return Ok(invalid("entry", id, "some fields are invalid", fields));
        }
        Err(err) => return Err(err),
    };

    let content_equal = existing
        .as_ref()
        .is_some_and(|row| sync::entry_content_equal(row, &prepared));
    let decision = sync::decide(
        existing.as_ref().map(ExistingState::from),
        change.base_seq,
        change.force,
        false,
        content_equal,
    );

    match decision {
        Decision::Noop => Ok(noop("entry", id, existing.as_ref().map(row_json))),
        Decision::Conflict(code) => {
            Ok(conflict("entry", id, code, existing.as_ref().map(row_json)))
        }
        Decision::Apply => {
            // A different row already occupies this employee/day.
            if let Some(other) =
                entries::find_live_by_employee_date(tx, &prepared.employee_id, &prepared.work_date)
                    .await?
                && other.id != id
            {
                return Ok(conflict(
                    "entry",
                    id,
                    "duplicate_entry_date",
                    Some(row_json(&other)),
                ));
            }

            let created_at = existing.as_ref().map(|row| row.created_at).unwrap_or(now);
            let seq = db::next_seq(tx).await?;
            match entries::upsert(tx, id, &prepared, created_at, now, None, seq).await {
                Ok(row) => {
                    let (action, verb) = if existing.is_some() {
                        ("update", "updated")
                    } else {
                        ("create", "created")
                    };
                    audit::record(
                        tx,
                        now,
                        actor_role,
                        actor_id,
                        actor_name,
                        "entry",
                        id,
                        action,
                        &entry_audit_summary(verb, &prepared.work_date, entry_employee_name),
                    )
                    .await?;
                    Ok(applied("entry", id, row_json(&row)))
                }
                Err(err) if is_unique_violation(&err) => {
                    let other = entries::find_live_by_employee_date(
                        tx,
                        &prepared.employee_id,
                        &prepared.work_date,
                    )
                    .await?;
                    Ok(conflict(
                        "entry",
                        id,
                        "duplicate_entry_date",
                        other.as_ref().map(row_json),
                    ))
                }
                Err(err) => Err(err.into()),
            }
        }
    }
}
