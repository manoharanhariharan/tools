//! Employee-day sheets: business across earning types, plus bonus/advance/
//! deduction. The server recalculates every total and snapshots each line's
//! type name so edits to the type list never rewrite history.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, QueryBuilder, SqliteConnection, SqlitePool};
use time::Date;
use uuid::Uuid;

use crate::domain::date;
use crate::domain::totals::{self, DayTotals, LineAmount, MAX_LINES};
use crate::error::{AppError, FieldError};
use crate::repo::employees::MAX_NOTE_LEN;
use crate::repo::types;

#[derive(Debug, Clone, Serialize, FromRow, PartialEq, Eq)]
pub struct EntryLineRow {
    pub id: String,
    pub entry_id: String,
    pub type_id: String,
    pub type_name: String,
    pub description: String,
    pub amount_minor: i64,
    pub position: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct EntryRow {
    pub id: String,
    pub employee_id: String,
    pub work_date: String,
    #[sqlx(rename = "base_minor")]
    #[serde(rename = "business_minor")]
    pub business_minor: i64,
    pub bonus_minor: i64,
    pub advance_minor: i64,
    pub deduction_minor: i64,
    pub gross_minor: i64,
    pub net_minor: i64,
    pub note: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub seq: i64,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub employee_name: String,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub employee_code: Option<String>,
    #[sqlx(skip)]
    pub lines: Vec<EntryLineRow>,
}

impl EntryRow {
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}

/// Carried-forward net balance for one employee before a report range. The
/// Sunday ledger adds the range's Sunday earnings to this value.
#[derive(Debug, Clone, FromRow)]
pub struct PriorTotal {
    pub employee_id: String,
    pub name: String,
    pub net_minor: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntryLineInput {
    pub type_id: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub amount_minor: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntryInput {
    pub employee_id: String,
    pub work_date: String,
    #[serde(default)]
    pub lines: Vec<EntryLineInput>,
    #[serde(default)]
    pub bonus_minor: Option<i64>,
    #[serde(default)]
    pub advance_minor: Option<i64>,
    #[serde(default)]
    pub deduction_minor: Option<i64>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedLine {
    pub type_id: String,
    pub type_name: String,
    pub description: String,
    pub amount_minor: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedEntry {
    pub employee_id: String,
    pub work_date: String,
    pub lines: Vec<PreparedLine>,
    pub bonus_minor: i64,
    pub advance_minor: i64,
    pub deduction_minor: i64,
    pub note: String,
    pub totals: DayTotals,
}

/// Validate raw inputs against the earning-type catalogue and recompute the day
/// totals. Requires a connection to resolve type names (and to reject unknown
/// type ids).
pub async fn prepare(
    conn: &mut SqliteConnection,
    input: &EntryInput,
) -> Result<PreparedEntry, AppError> {
    let work_date: Date = match date::parse_iso(input.work_date.trim()) {
        Some(d) if (1970..=2100).contains(&d.year()) => d,
        _ => {
            return Err(AppError::Validation(vec![FieldError::new(
                "work_date",
                "expected a valid YYYY-MM-DD date",
            )]));
        }
    };

    let note = input.note.clone().unwrap_or_default();
    if note.chars().count() > MAX_NOTE_LEN {
        return Err(AppError::Validation(vec![FieldError::new(
            "note",
            format!("must be at most {MAX_NOTE_LEN} characters"),
        )]));
    }
    if note
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        return Err(AppError::Validation(vec![FieldError::new(
            "note",
            "must not contain control characters",
        )]));
    }

    if input.lines.len() > MAX_LINES {
        return Err(AppError::Validation(vec![FieldError::new(
            "lines",
            format!("at most {MAX_LINES} earning rows are allowed"),
        )]));
    }

    let raw: Vec<LineAmount> = input
        .lines
        .iter()
        .map(|line| LineAmount {
            type_id: line.type_id.trim().to_string(),
            description: line
                .description
                .clone()
                .unwrap_or_default()
                .trim()
                .to_string(),
            amount_minor: line.amount_minor,
        })
        .collect();

    let bonus_minor = input.bonus_minor.unwrap_or(0);
    let advance_minor = input.advance_minor.unwrap_or(0);
    let deduction_minor = input.deduction_minor.unwrap_or(0);
    let totals = totals::compute(&raw, bonus_minor, advance_minor, deduction_minor)
        .map_err(AppError::Validation)?;

    let type_ids: Vec<String> = raw
        .iter()
        .map(|line| line.type_id.clone())
        .filter(|id| !id.is_empty())
        .collect();
    let found = types::find_many(conn, &type_ids).await?;
    let names: HashMap<String, String> = found.into_iter().map(|row| (row.id, row.name)).collect();

    let mut lines = Vec::with_capacity(raw.len());
    for line in raw {
        let Some(type_name) = names.get(&line.type_id).cloned() else {
            return Err(AppError::Validation(vec![FieldError::new(
                "type_id",
                "unknown earning type",
            )]));
        };
        if line.amount_minor == 0 {
            // Zero-valued rows carry no information; keep the day sheet clean.
            continue;
        }
        lines.push(PreparedLine {
            type_id: line.type_id,
            type_name,
            description: line.description,
            amount_minor: line.amount_minor,
        });
    }

    Ok(PreparedEntry {
        employee_id: input.employee_id.trim().to_string(),
        work_date: date::iso(work_date),
        lines,
        bonus_minor,
        advance_minor,
        deduction_minor,
        note,
        totals,
    })
}

const VIEW_SELECT: &str = "SELECT e.*, emp.name AS employee_name, emp.code AS employee_code
     FROM entries e JOIN employees emp ON emp.id = e.employee_id";

async fn load_lines(
    conn: &mut SqliteConnection,
    entry_ids: &[String],
) -> Result<HashMap<String, Vec<EntryLineRow>>, sqlx::Error> {
    let mut grouped: HashMap<String, Vec<EntryLineRow>> = HashMap::new();
    if entry_ids.is_empty() {
        return Ok(grouped);
    }

    let mut builder = QueryBuilder::new("SELECT * FROM entry_lines WHERE entry_id IN (");
    let mut separated = builder.separated(", ");
    for id in entry_ids {
        separated.push_bind(id);
    }
    separated.push_unseparated(") ORDER BY position ASC, created_at ASC, id ASC");

    let rows: Vec<EntryLineRow> = builder.build_query_as().fetch_all(conn).await?;
    for row in rows {
        grouped.entry(row.entry_id.clone()).or_default().push(row);
    }
    Ok(grouped)
}

async fn attach_lines(
    conn: &mut SqliteConnection,
    rows: &mut [EntryRow],
) -> Result<(), sqlx::Error> {
    if rows.is_empty() {
        return Ok(());
    }
    let ids: Vec<String> = rows.iter().map(|row| row.id.clone()).collect();
    let mut grouped = load_lines(conn, &ids).await?;
    for row in rows {
        row.lines = grouped.remove(&row.id).unwrap_or_default();
    }
    Ok(())
}

/// Attach earning lines to rows fetched outside this module (sync pulls).
pub async fn hydrate_lines(
    conn: &mut SqliteConnection,
    rows: &mut [EntryRow],
) -> Result<(), sqlx::Error> {
    attach_lines(conn, rows).await
}

pub async fn list_live(
    pool: &SqlitePool,
    from: &str,
    to: &str,
    employee_id: Option<&str>,
) -> Result<Vec<EntryRow>, sqlx::Error> {
    let mut conn = pool.acquire().await?;
    let mut rows = sqlx::query_as::<_, EntryRow>(&format!(
        "{VIEW_SELECT}
         WHERE e.deleted_at IS NULL
           AND e.work_date BETWEEN ? AND ?
           AND (? IS NULL OR e.employee_id = ?)
         ORDER BY e.work_date ASC, emp.name COLLATE NOCASE ASC, e.created_at ASC"
    ))
    .bind(from)
    .bind(to)
    .bind(employee_id)
    .bind(employee_id)
    .fetch_all(&mut *conn)
    .await?;
    attach_lines(&mut conn, &mut rows).await?;
    Ok(rows)
}

/// Net money per employee for every live day sheet strictly before `before`.
/// This is the "previous total" carried into a Sunday ledger report.
pub async fn totals_before(
    pool: &SqlitePool,
    before: &str,
    employee_id: Option<&str>,
) -> Result<Vec<PriorTotal>, sqlx::Error> {
    sqlx::query_as::<_, PriorTotal>(
        "SELECT e.employee_id,
                emp.name AS name,
                COALESCE(SUM(e.net_minor), 0) AS net_minor
           FROM entries e
           JOIN employees emp ON emp.id = e.employee_id
          WHERE e.deleted_at IS NULL
            AND e.work_date < ?
            AND (? IS NULL OR e.employee_id = ?)
          GROUP BY e.employee_id, emp.name",
    )
    .bind(before)
    .bind(employee_id)
    .bind(employee_id)
    .fetch_all(pool)
    .await
}

pub async fn find(conn: &mut SqliteConnection, id: &str) -> Result<Option<EntryRow>, sqlx::Error> {
    let mut row = sqlx::query_as::<_, EntryRow>("SELECT * FROM entries WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
    if let Some(row) = row.as_mut() {
        attach_lines(conn, std::slice::from_mut(row)).await?;
    }
    Ok(row)
}

pub async fn find_live(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<Option<EntryRow>, sqlx::Error> {
    let mut row = sqlx::query_as::<_, EntryRow>(&format!(
        "{VIEW_SELECT} WHERE e.id = ? AND e.deleted_at IS NULL"
    ))
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(row) = row.as_mut() {
        attach_lines(conn, std::slice::from_mut(row)).await?;
    }
    Ok(row)
}

pub async fn find_any_view(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<Option<EntryRow>, sqlx::Error> {
    let mut row = sqlx::query_as::<_, EntryRow>(&format!("{VIEW_SELECT} WHERE e.id = ?"))
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
    if let Some(row) = row.as_mut() {
        attach_lines(conn, std::slice::from_mut(row)).await?;
    }
    Ok(row)
}

pub async fn find_live_by_employee_date(
    conn: &mut SqliteConnection,
    employee_id: &str,
    work_date: &str,
) -> Result<Option<EntryRow>, sqlx::Error> {
    let mut row = sqlx::query_as::<_, EntryRow>(&format!(
        "{VIEW_SELECT}
         WHERE e.employee_id = ? AND e.work_date = ? AND e.deleted_at IS NULL"
    ))
    .bind(employee_id)
    .bind(work_date)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(row) = row.as_mut() {
        attach_lines(conn, std::slice::from_mut(row)).await?;
    }
    Ok(row)
}

async fn replace_lines(
    conn: &mut SqliteConnection,
    entry_id: &str,
    lines: &[PreparedLine],
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM entry_lines WHERE entry_id = ?")
        .bind(entry_id)
        .execute(&mut *conn)
        .await?;
    for (position, line) in lines.iter().enumerate() {
        sqlx::query(
            "INSERT INTO entry_lines
                (id, entry_id, type_id, type_name, description, amount_minor, position, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(entry_id)
        .bind(&line.type_id)
        .bind(&line.type_name)
        .bind(&line.description)
        .bind(line.amount_minor)
        .bind(position as i64)
        .bind(now)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

pub async fn insert(
    conn: &mut SqliteConnection,
    id: &str,
    p: &PreparedEntry,
    now: i64,
    seq: i64,
) -> Result<EntryRow, sqlx::Error> {
    let mut row = sqlx::query_as::<_, EntryRow>(
        "INSERT INTO entries
            (id, employee_id, work_date, day_fraction_bp, regular_minutes,
             overtime_minutes, ot_multiplier_bp, override_amount_minor,
             bonus_minor, advance_minor, deduction_minor, base_minor, ot_minor,
             gross_minor, net_minor, note, created_at, updated_at, deleted_at, seq)
         VALUES (?, ?, ?, 10000, 0, 0, NULL, NULL, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?, NULL, ?)
         RETURNING *",
    )
    .bind(id)
    .bind(&p.employee_id)
    .bind(&p.work_date)
    .bind(p.bonus_minor)
    .bind(p.advance_minor)
    .bind(p.deduction_minor)
    .bind(p.totals.business_minor)
    .bind(p.totals.gross_minor)
    .bind(p.totals.net_minor)
    .bind(&p.note)
    .bind(now)
    .bind(now)
    .bind(seq)
    .fetch_one(&mut *conn)
    .await?;

    replace_lines(conn, id, &p.lines, now).await?;
    row.lines = load_lines(conn, &[id.to_string()])
        .await?
        .remove(id)
        .unwrap_or_default();
    Ok(row)
}

pub async fn update(
    conn: &mut SqliteConnection,
    id: &str,
    p: &PreparedEntry,
    now: i64,
    seq: i64,
) -> Result<EntryRow, sqlx::Error> {
    let mut row = sqlx::query_as::<_, EntryRow>(
        "UPDATE entries SET
            employee_id = ?, work_date = ?, bonus_minor = ?, advance_minor = ?,
            deduction_minor = ?, base_minor = ?, ot_minor = 0, gross_minor = ?,
            net_minor = ?, note = ?, updated_at = ?, seq = ?,
            day_fraction_bp = 10000, regular_minutes = 0, overtime_minutes = 0,
            ot_multiplier_bp = NULL, override_amount_minor = NULL
         WHERE id = ? AND deleted_at IS NULL
         RETURNING *",
    )
    .bind(&p.employee_id)
    .bind(&p.work_date)
    .bind(p.bonus_minor)
    .bind(p.advance_minor)
    .bind(p.deduction_minor)
    .bind(p.totals.business_minor)
    .bind(p.totals.gross_minor)
    .bind(p.totals.net_minor)
    .bind(&p.note)
    .bind(now)
    .bind(seq)
    .bind(id)
    .fetch_one(&mut *conn)
    .await?;

    replace_lines(conn, id, &p.lines, now).await?;
    row.lines = load_lines(conn, &[id.to_string()])
        .await?
        .remove(id)
        .unwrap_or_default();
    Ok(row)
}

/// Insert-or-replace for sync; can resurrect a tombstone and preserves the
/// original `created_at`.
pub async fn upsert(
    conn: &mut SqliteConnection,
    id: &str,
    p: &PreparedEntry,
    created_at: i64,
    updated_at: i64,
    deleted_at: Option<i64>,
    seq: i64,
) -> Result<EntryRow, sqlx::Error> {
    let mut row = sqlx::query_as::<_, EntryRow>(
        "INSERT INTO entries
            (id, employee_id, work_date, day_fraction_bp, regular_minutes,
             overtime_minutes, ot_multiplier_bp, override_amount_minor,
             bonus_minor, advance_minor, deduction_minor, base_minor, ot_minor,
             gross_minor, net_minor, note, created_at, updated_at, deleted_at, seq)
         VALUES (?, ?, ?, 10000, 0, 0, NULL, NULL, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
            employee_id = excluded.employee_id,
            work_date = excluded.work_date,
            bonus_minor = excluded.bonus_minor,
            advance_minor = excluded.advance_minor,
            deduction_minor = excluded.deduction_minor,
            base_minor = excluded.base_minor,
            ot_minor = 0,
            gross_minor = excluded.gross_minor,
            net_minor = excluded.net_minor,
            note = excluded.note,
            updated_at = excluded.updated_at,
            deleted_at = excluded.deleted_at,
            seq = excluded.seq,
            day_fraction_bp = 10000,
            regular_minutes = 0,
            overtime_minutes = 0,
            ot_multiplier_bp = NULL,
            override_amount_minor = NULL
         RETURNING *",
    )
    .bind(id)
    .bind(&p.employee_id)
    .bind(&p.work_date)
    .bind(p.bonus_minor)
    .bind(p.advance_minor)
    .bind(p.deduction_minor)
    .bind(p.totals.business_minor)
    .bind(p.totals.gross_minor)
    .bind(p.totals.net_minor)
    .bind(&p.note)
    .bind(created_at)
    .bind(updated_at)
    .bind(deleted_at)
    .bind(seq)
    .fetch_one(&mut *conn)
    .await?;

    replace_lines(conn, id, &p.lines, updated_at).await?;
    row.lines = load_lines(conn, &[id.to_string()])
        .await?
        .remove(id)
        .unwrap_or_default();
    Ok(row)
}

pub async fn soft_delete(
    conn: &mut SqliteConnection,
    id: &str,
    now: i64,
    seq: i64,
) -> Result<EntryRow, sqlx::Error> {
    let mut row = sqlx::query_as::<_, EntryRow>(
        "UPDATE entries SET deleted_at = ?, updated_at = ?, seq = ?
         WHERE id = ? AND deleted_at IS NULL
         RETURNING *",
    )
    .bind(now)
    .bind(now)
    .bind(seq)
    .bind(id)
    .fetch_one(&mut *conn)
    .await?;
    row.lines = load_lines(conn, &[id.to_string()])
        .await?
        .remove(id)
        .unwrap_or_default();
    Ok(row)
}

/// Tombstone an entry. Returns `None` when it was already deleted.
pub async fn mark_deleted(
    conn: &mut SqliteConnection,
    id: &str,
    now: i64,
    seq: i64,
) -> Result<Option<EntryRow>, sqlx::Error> {
    let mut row = sqlx::query_as::<_, EntryRow>(
        "UPDATE entries SET deleted_at = ?, updated_at = ?, seq = ?
         WHERE id = ? AND deleted_at IS NULL
         RETURNING *",
    )
    .bind(now)
    .bind(now)
    .bind(seq)
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(row) = row.as_mut() {
        row.lines = load_lines(conn, &[id.to_string()])
            .await?
            .remove(id)
            .unwrap_or_default();
    }
    Ok(row)
}
