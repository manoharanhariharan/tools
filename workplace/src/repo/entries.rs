use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection, SqlitePool};
use time::Date;

use crate::domain::date;
use crate::domain::pay::{
    Computed, EntryAmounts, FULL_DAY_BP, compute,
};
use crate::error::{AppError, FieldError};
use crate::repo::employees::{EmployeeRow, MAX_NOTE_LEN};

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct EntryRow {
    pub id: String,
    pub employee_id: String,
    pub work_date: String,
    pub day_fraction_bp: i64,
    pub regular_minutes: i64,
    pub overtime_minutes: i64,
    pub ot_multiplier_bp: Option<i64>,
    pub override_amount_minor: Option<i64>,
    pub bonus_minor: i64,
    pub advance_minor: i64,
    pub deduction_minor: i64,
    pub base_minor: i64,
    pub ot_minor: i64,
    pub gross_minor: i64,
    pub net_minor: i64,
    pub note: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub seq: i64,
    /// Present on list/detail queries that join the employee; absent (empty)
    /// in sync payloads, which carry employees separately.
    #[sqlx(default)]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub employee_name: String,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub employee_code: Option<String>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub employee_pay_basis: String,
}

impl EntryRow {
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntryInput {
    pub employee_id: String,
    pub work_date: String,
    #[serde(default)]
    pub day_fraction_bp: Option<i64>,
    #[serde(default)]
    pub regular_minutes: Option<i64>,
    #[serde(default)]
    pub overtime_minutes: Option<i64>,
    #[serde(default)]
    pub ot_multiplier_bp: Option<i64>,
    #[serde(default)]
    pub override_amount_minor: Option<i64>,
    #[serde(default)]
    pub bonus_minor: Option<i64>,
    #[serde(default)]
    pub advance_minor: Option<i64>,
    #[serde(default)]
    pub deduction_minor: Option<i64>,
    #[serde(default)]
    pub note: Option<String>,
}

/// Normalized, validated inputs plus the server-authoritative amounts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedEntry {
    pub employee_id: String,
    pub work_date: String,
    pub day_fraction_bp: i64,
    pub regular_minutes: i64,
    pub overtime_minutes: i64,
    pub ot_multiplier_bp: Option<i64>,
    pub override_amount_minor: Option<i64>,
    pub bonus_minor: i64,
    pub advance_minor: i64,
    pub deduction_minor: i64,
    pub note: String,
    pub computed: Computed,
}

/// Validate raw inputs against the employee's rule and recompute all amounts.
pub fn prepare(employee: &EmployeeRow, input: &EntryInput) -> Result<PreparedEntry, AppError> {
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

    let raw = EntryAmounts {
        day_fraction_bp: input.day_fraction_bp.unwrap_or(FULL_DAY_BP),
        regular_minutes: input.regular_minutes.unwrap_or(0),
        overtime_minutes: input.overtime_minutes.unwrap_or(0),
        ot_multiplier_bp: input.ot_multiplier_bp,
        override_amount_minor: input.override_amount_minor,
        bonus_minor: input.bonus_minor.unwrap_or(0),
        advance_minor: input.advance_minor.unwrap_or(0),
        deduction_minor: input.deduction_minor.unwrap_or(0),
    };
    let amounts = raw.normalized_for(employee.basis());
    let computed = compute(&employee.rule(), &amounts).map_err(AppError::Validation)?;

    Ok(PreparedEntry {
        employee_id: employee.id.clone(),
        work_date: date::iso(work_date),
        day_fraction_bp: amounts.day_fraction_bp,
        regular_minutes: amounts.regular_minutes,
        overtime_minutes: amounts.overtime_minutes,
        ot_multiplier_bp: amounts.ot_multiplier_bp,
        override_amount_minor: amounts.override_amount_minor,
        bonus_minor: amounts.bonus_minor,
        advance_minor: amounts.advance_minor,
        deduction_minor: amounts.deduction_minor,
        note,
        computed,
    })
}

const VIEW_SELECT: &str = "SELECT e.*, emp.name AS employee_name, emp.code AS employee_code,
            emp.pay_basis AS employee_pay_basis
     FROM entries e JOIN employees emp ON emp.id = e.employee_id";

pub async fn list_live(
    pool: &SqlitePool,
    from: &str,
    to: &str,
    employee_id: Option<&str>,
) -> Result<Vec<EntryRow>, sqlx::Error> {
    sqlx::query_as::<_, EntryRow>(&format!(
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
    .fetch_all(pool)
    .await
}

pub async fn find(conn: &mut SqliteConnection, id: &str) -> Result<Option<EntryRow>, sqlx::Error> {
    sqlx::query_as::<_, EntryRow>("SELECT * FROM entries WHERE id = ?")
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn find_live(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<Option<EntryRow>, sqlx::Error> {
    sqlx::query_as::<_, EntryRow>(&format!(
        "{VIEW_SELECT} WHERE e.id = ? AND e.deleted_at IS NULL"
    ))
    .bind(id)
    .fetch_optional(conn)
    .await
}

pub async fn find_any_view(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<Option<EntryRow>, sqlx::Error> {
    sqlx::query_as::<_, EntryRow>(&format!("{VIEW_SELECT} WHERE e.id = ?"))
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn find_live_by_employee_date(
    conn: &mut SqliteConnection,
    employee_id: &str,
    work_date: &str,
) -> Result<Option<EntryRow>, sqlx::Error> {
    sqlx::query_as::<_, EntryRow>(&format!(
        "{VIEW_SELECT}
         WHERE e.employee_id = ? AND e.work_date = ? AND e.deleted_at IS NULL"
    ))
    .bind(employee_id)
    .bind(work_date)
    .fetch_optional(conn)
    .await
}

pub async fn insert(
    conn: &mut SqliteConnection,
    id: &str,
    p: &PreparedEntry,
    now: i64,
    seq: i64,
) -> Result<EntryRow, sqlx::Error> {
    sqlx::query_as::<_, EntryRow>(
        "INSERT INTO entries
            (id, employee_id, work_date, day_fraction_bp, regular_minutes,
             overtime_minutes, ot_multiplier_bp, override_amount_minor,
             bonus_minor, advance_minor, deduction_minor, base_minor, ot_minor,
             gross_minor, net_minor, note, created_at, updated_at, deleted_at, seq)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?)
         RETURNING *",
    )
    .bind(id)
    .bind(&p.employee_id)
    .bind(&p.work_date)
    .bind(p.day_fraction_bp)
    .bind(p.regular_minutes)
    .bind(p.overtime_minutes)
    .bind(p.ot_multiplier_bp)
    .bind(p.override_amount_minor)
    .bind(p.bonus_minor)
    .bind(p.advance_minor)
    .bind(p.deduction_minor)
    .bind(p.computed.base_minor)
    .bind(p.computed.ot_minor)
    .bind(p.computed.gross_minor)
    .bind(p.computed.net_minor)
    .bind(&p.note)
    .bind(now)
    .bind(now)
    .bind(seq)
    .fetch_one(conn)
    .await
}

pub async fn update(
    conn: &mut SqliteConnection,
    id: &str,
    p: &PreparedEntry,
    now: i64,
    seq: i64,
) -> Result<EntryRow, sqlx::Error> {
    sqlx::query_as::<_, EntryRow>(
        "UPDATE entries SET
            employee_id = ?, work_date = ?, day_fraction_bp = ?,
            regular_minutes = ?, overtime_minutes = ?, ot_multiplier_bp = ?,
            override_amount_minor = ?, bonus_minor = ?, advance_minor = ?,
            deduction_minor = ?, base_minor = ?, ot_minor = ?, gross_minor = ?,
            net_minor = ?, note = ?, updated_at = ?, seq = ?
         WHERE id = ? AND deleted_at IS NULL
         RETURNING *",
    )
    .bind(&p.employee_id)
    .bind(&p.work_date)
    .bind(p.day_fraction_bp)
    .bind(p.regular_minutes)
    .bind(p.overtime_minutes)
    .bind(p.ot_multiplier_bp)
    .bind(p.override_amount_minor)
    .bind(p.bonus_minor)
    .bind(p.advance_minor)
    .bind(p.deduction_minor)
    .bind(p.computed.base_minor)
    .bind(p.computed.ot_minor)
    .bind(p.computed.gross_minor)
    .bind(p.computed.net_minor)
    .bind(&p.note)
    .bind(now)
    .bind(seq)
    .bind(id)
    .fetch_one(conn)
    .await
}

pub async fn soft_delete(
    conn: &mut SqliteConnection,
    id: &str,
    now: i64,
    seq: i64,
) -> Result<EntryRow, sqlx::Error> {
    sqlx::query_as::<_, EntryRow>(
        "UPDATE entries SET deleted_at = ?, updated_at = ?, seq = ?
         WHERE id = ? AND deleted_at IS NULL
         RETURNING *",
    )
    .bind(now)
    .bind(now)
    .bind(seq)
    .bind(id)
    .fetch_one(conn)
    .await
}

/// Insert-or-replace used by the sync engine; can also resurrect a tombstoned
/// entry and preserves the original `created_at`.
pub async fn upsert(
    conn: &mut SqliteConnection,
    id: &str,
    p: &PreparedEntry,
    created_at: i64,
    updated_at: i64,
    deleted_at: Option<i64>,
    seq: i64,
) -> Result<EntryRow, sqlx::Error> {
    sqlx::query_as::<_, EntryRow>(
        "INSERT INTO entries
            (id, employee_id, work_date, day_fraction_bp, regular_minutes,
             overtime_minutes, ot_multiplier_bp, override_amount_minor,
             bonus_minor, advance_minor, deduction_minor, base_minor, ot_minor,
             gross_minor, net_minor, note, created_at, updated_at, deleted_at, seq)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
            employee_id = excluded.employee_id,
            work_date = excluded.work_date,
            day_fraction_bp = excluded.day_fraction_bp,
            regular_minutes = excluded.regular_minutes,
            overtime_minutes = excluded.overtime_minutes,
            ot_multiplier_bp = excluded.ot_multiplier_bp,
            override_amount_minor = excluded.override_amount_minor,
            bonus_minor = excluded.bonus_minor,
            advance_minor = excluded.advance_minor,
            deduction_minor = excluded.deduction_minor,
            base_minor = excluded.base_minor,
            ot_minor = excluded.ot_minor,
            gross_minor = excluded.gross_minor,
            net_minor = excluded.net_minor,
            note = excluded.note,
            updated_at = excluded.updated_at,
            deleted_at = excluded.deleted_at,
            seq = excluded.seq
         RETURNING *",
    )
    .bind(id)
    .bind(&p.employee_id)
    .bind(&p.work_date)
    .bind(p.day_fraction_bp)
    .bind(p.regular_minutes)
    .bind(p.overtime_minutes)
    .bind(p.ot_multiplier_bp)
    .bind(p.override_amount_minor)
    .bind(p.bonus_minor)
    .bind(p.advance_minor)
    .bind(p.deduction_minor)
    .bind(p.computed.base_minor)
    .bind(p.computed.ot_minor)
    .bind(p.computed.gross_minor)
    .bind(p.computed.net_minor)
    .bind(&p.note)
    .bind(created_at)
    .bind(updated_at)
    .bind(deleted_at)
    .bind(seq)
    .fetch_one(conn)
    .await
}

/// Tombstone an entry. Returns `None` when it was already deleted.
pub async fn mark_deleted(
    conn: &mut SqliteConnection,
    id: &str,
    now: i64,
    seq: i64,
) -> Result<Option<EntryRow>, sqlx::Error> {
    sqlx::query_as::<_, EntryRow>(
        "UPDATE entries SET deleted_at = ?, updated_at = ?, seq = ?
         WHERE id = ? AND deleted_at IS NULL
         RETURNING *",
    )
    .bind(now)
    .bind(now)
    .bind(seq)
    .bind(id)
    .fetch_optional(conn)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pay::PayBasis;

    fn employee(basis: &str, rate: i64) -> EmployeeRow {
        EmployeeRow {
            id: "emp-1".into(),
            name: "Asha".into(),
            code: Some("E1".into()),
            pay_basis: basis.into(),
            rate_minor: rate,
            ot_multiplier_bp: 15_000,
            standard_minutes_per_day: 480,
            note: String::new(),
            active: true,
            created_at: 0,
            updated_at: 0,
            deleted_at: None,
            seq: 0,
        }
    }

    fn input() -> EntryInput {
        EntryInput {
            employee_id: "emp-1".into(),
            work_date: "2026-11-23".into(),
            day_fraction_bp: None,
            regular_minutes: None,
            overtime_minutes: None,
            ot_multiplier_bp: None,
            override_amount_minor: None,
            bonus_minor: None,
            advance_minor: None,
            deduction_minor: None,
            note: None,
        }
    }

    #[test]
    fn prepares_day_rate_entry() {
        let p = prepare(&employee("day_rate", 80_000), &input()).unwrap();
        assert_eq!(p.computed.base_minor, 80_000);
        assert_eq!(p.work_date, "2026-11-23");
        assert_eq!(p.day_fraction_bp, FULL_DAY_BP);
    }

    #[test]
    fn normalizes_hourly_inputs() {
        let mut i = input();
        i.day_fraction_bp = Some(5_000);
        i.regular_minutes = Some(480);
        let p = prepare(&employee("hourly", 15_000), &i).unwrap();
        assert_eq!(p.day_fraction_bp, FULL_DAY_BP);
        assert_eq!(p.computed.base_minor, 120_000);
    }

    #[test]
    fn rejects_bad_dates_and_notes() {
        let mut i = input();
        i.work_date = "23/11/2026".into();
        assert!(matches!(prepare(&employee("day_rate", 1), &i), Err(AppError::Validation(_))));

        let mut i = input();
        i.note = Some("x".repeat(MAX_NOTE_LEN + 1));
        assert!(matches!(prepare(&employee("day_rate", 1), &i), Err(AppError::Validation(_))));
    }

    #[test]
    fn rejects_irrelevant_inputs_after_normalization_only_when_invalid() {
        let mut i = input();
        i.day_fraction_bp = Some(30_000);
        let errors = match prepare(&employee("day_rate", 80_000), &i) {
            Err(AppError::Validation(errors)) => errors,
            other => panic!("expected validation error, got {other:?}"),
        };
        assert!(errors.iter().any(|e| e.field == "day_fraction_bp"));
    }

    #[test]
    fn basis_parsing_is_total() {
        assert_eq!(employee("day_rate", 1).basis(), PayBasis::DayRate);
        assert_eq!(employee("bogus", 1).basis(), PayBasis::Fixed);
    }
}
