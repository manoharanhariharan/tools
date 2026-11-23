use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::domain::money::MAX_AMOUNT_MINOR;
use crate::domain::pay::{
    DEFAULT_OT_MULTIPLIER_BP, DEFAULT_STANDARD_MINUTES, MAX_OT_MULTIPLIER_BP,
    MAX_STANDARD_MINUTES, MIN_OT_MULTIPLIER_BP, MIN_STANDARD_MINUTES, PayBasis, PayRule,
};
use crate::error::FieldError;

pub const MAX_NAME_LEN: usize = 120;
pub const MAX_NOTE_LEN: usize = 2_000;
pub const MAX_CODE_LEN: usize = 32;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct EmployeeRow {
    pub id: String,
    pub name: String,
    pub code: Option<String>,
    pub pay_basis: String,
    pub rate_minor: i64,
    pub ot_multiplier_bp: i64,
    pub standard_minutes_per_day: i64,
    pub note: String,
    pub active: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub seq: i64,
}

impl EmployeeRow {
    pub fn basis(&self) -> PayBasis {
        PayBasis::parse(&self.pay_basis).unwrap_or(PayBasis::Fixed)
    }

    pub fn rule(&self) -> PayRule {
        PayRule {
            basis: self.basis(),
            rate_minor: self.rate_minor,
            ot_multiplier_bp: self.ot_multiplier_bp,
            standard_minutes_per_day: self.standard_minutes_per_day,
        }
    }

    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmployeeInput {
    pub name: String,
    #[serde(default)]
    pub code: Option<String>,
    pub pay_basis: String,
    #[serde(default)]
    pub rate_minor: i64,
    #[serde(default)]
    pub ot_multiplier_bp: Option<i64>,
    #[serde(default)]
    pub standard_minutes_per_day: Option<i64>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default = "default_active")]
    pub active: bool,
}

fn default_active() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedEmployee {
    pub name: String,
    pub code: Option<String>,
    pub pay_basis: PayBasis,
    pub rate_minor: i64,
    pub ot_multiplier_bp: i64,
    pub standard_minutes_per_day: i64,
    pub note: String,
    pub active: bool,
}

pub fn validate_input(input: &EmployeeInput) -> Result<ValidatedEmployee, Vec<FieldError>> {
    let mut errors = Vec::new();

    let name = input.name.trim();
    if name.is_empty() {
        errors.push(FieldError::new("name", "required"));
    } else if name.chars().count() > MAX_NAME_LEN {
        errors.push(FieldError::new(
            "name",
            format!("must be at most {MAX_NAME_LEN} characters"),
        ));
    } else if name.chars().any(|c| c.is_control()) {
        errors.push(FieldError::new("name", "must not contain control characters"));
    }

    let code = input
        .code
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(str::to_string);
    if let Some(code) = &code {
        if code.chars().count() > MAX_CODE_LEN {
            errors.push(FieldError::new(
                "code",
                format!("must be at most {MAX_CODE_LEN} characters"),
            ));
        }
        if !code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
        {
            errors.push(FieldError::new(
                "code",
                "use letters, digits, '-', '_', '.' or '/' only",
            ));
        }
    }

    let pay_basis = PayBasis::parse(&input.pay_basis);
    if pay_basis.is_none() {
        errors.push(FieldError::new(
            "pay_basis",
            "must be one of day_rate, hourly, fixed",
        ));
    }

    if !(0..=MAX_AMOUNT_MINOR).contains(&input.rate_minor) {
        errors.push(FieldError::new(
            "rate_minor",
            format!("must be between 0 and {MAX_AMOUNT_MINOR} minor units"),
        ));
    }

    let ot_multiplier_bp = input.ot_multiplier_bp.unwrap_or(DEFAULT_OT_MULTIPLIER_BP);
    if !(MIN_OT_MULTIPLIER_BP..=MAX_OT_MULTIPLIER_BP).contains(&ot_multiplier_bp) {
        errors.push(FieldError::new(
            "ot_multiplier_bp",
            format!("must be between {MIN_OT_MULTIPLIER_BP} and {MAX_OT_MULTIPLIER_BP}"),
        ));
    }

    let standard_minutes_per_day = input
        .standard_minutes_per_day
        .unwrap_or(DEFAULT_STANDARD_MINUTES);
    if !(MIN_STANDARD_MINUTES..=MAX_STANDARD_MINUTES).contains(&standard_minutes_per_day) {
        errors.push(FieldError::new(
            "standard_minutes_per_day",
            format!("must be between {MIN_STANDARD_MINUTES} and {MAX_STANDARD_MINUTES}"),
        ));
    }

    let note = input.note.clone().unwrap_or_default();
    if note.chars().count() > MAX_NOTE_LEN {
        errors.push(FieldError::new(
            "note",
            format!("must be at most {MAX_NOTE_LEN} characters"),
        ));
    }
    if note
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        errors.push(FieldError::new("note", "must not contain control characters"));
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(ValidatedEmployee {
        name: name.to_string(),
        code,
        pay_basis: pay_basis.expect("checked above"),
        rate_minor: input.rate_minor,
        ot_multiplier_bp,
        standard_minutes_per_day,
        note,
        active: input.active,
    })
}

pub async fn list_live(pool: &SqlitePool) -> Result<Vec<EmployeeRow>, sqlx::Error> {
    sqlx::query_as::<_, EmployeeRow>(
        "SELECT * FROM employees WHERE deleted_at IS NULL
         ORDER BY active DESC, name COLLATE NOCASE ASC, created_at ASC",
    )
    .fetch_all(pool)
    .await
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<EmployeeRow>, sqlx::Error> {
    sqlx::query_as::<_, EmployeeRow>("SELECT * FROM employees WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn find(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<Option<EmployeeRow>, sqlx::Error> {
    sqlx::query_as::<_, EmployeeRow>("SELECT * FROM employees WHERE id = ?")
        .bind(id)
        .fetch_optional(conn)
        .await
}

/// Insert-or-replace used by the sync engine. Unlike [`update`], this can also
/// resurrect a tombstoned row (when the client explicitly forces it) and
/// preserves the original `created_at`.
pub async fn upsert(
    conn: &mut SqliteConnection,
    id: &str,
    v: &ValidatedEmployee,
    created_at: i64,
    updated_at: i64,
    deleted_at: Option<i64>,
    seq: i64,
) -> Result<EmployeeRow, sqlx::Error> {
    sqlx::query_as::<_, EmployeeRow>(
        "INSERT INTO employees
            (id, name, code, pay_basis, rate_minor, ot_multiplier_bp,
             standard_minutes_per_day, note, active, created_at, updated_at,
             deleted_at, seq)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
            name = excluded.name,
            code = excluded.code,
            pay_basis = excluded.pay_basis,
            rate_minor = excluded.rate_minor,
            ot_multiplier_bp = excluded.ot_multiplier_bp,
            standard_minutes_per_day = excluded.standard_minutes_per_day,
            note = excluded.note,
            active = excluded.active,
            updated_at = excluded.updated_at,
            deleted_at = excluded.deleted_at,
            seq = excluded.seq
         RETURNING *",
    )
    .bind(id)
    .bind(&v.name)
    .bind(&v.code)
    .bind(v.pay_basis.as_str())
    .bind(v.rate_minor)
    .bind(v.ot_multiplier_bp)
    .bind(v.standard_minutes_per_day)
    .bind(&v.note)
    .bind(v.active)
    .bind(created_at)
    .bind(updated_at)
    .bind(deleted_at)
    .bind(seq)
    .fetch_one(conn)
    .await
}

/// Tombstone an employee. Returns `None` when it was already deleted.
pub async fn mark_deleted(
    conn: &mut SqliteConnection,
    id: &str,
    now: i64,
    seq: i64,
) -> Result<Option<EmployeeRow>, sqlx::Error> {
    sqlx::query_as::<_, EmployeeRow>(
        "UPDATE employees SET deleted_at = ?, updated_at = ?, seq = ?
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

pub async fn get_live(pool: &SqlitePool, id: &str) -> Result<Option<EmployeeRow>, sqlx::Error> {
    sqlx::query_as::<_, EmployeeRow>(
        "SELECT * FROM employees WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn get_live_by_code(
    pool: &SqlitePool,
    code: &str,
) -> Result<Option<EmployeeRow>, sqlx::Error> {
    sqlx::query_as::<_, EmployeeRow>(
        "SELECT * FROM employees WHERE code = ? AND deleted_at IS NULL",
    )
    .bind(code)
    .fetch_optional(pool)
    .await
}

pub async fn find_live_by_code(
    conn: &mut SqliteConnection,
    code: &str,
) -> Result<Option<EmployeeRow>, sqlx::Error> {
    sqlx::query_as::<_, EmployeeRow>(
        "SELECT * FROM employees WHERE code = ? AND deleted_at IS NULL",
    )
    .bind(code)
    .fetch_optional(conn)
    .await
}

pub async fn insert(
    conn: &mut SqliteConnection,
    id: &str,
    v: &ValidatedEmployee,
    now: i64,
    seq: i64,
) -> Result<EmployeeRow, sqlx::Error> {
    sqlx::query_as::<_, EmployeeRow>(
        "INSERT INTO employees
            (id, name, code, pay_basis, rate_minor, ot_multiplier_bp,
             standard_minutes_per_day, note, active, created_at, updated_at,
             deleted_at, seq)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?)
         RETURNING *",
    )
    .bind(id)
    .bind(&v.name)
    .bind(&v.code)
    .bind(v.pay_basis.as_str())
    .bind(v.rate_minor)
    .bind(v.ot_multiplier_bp)
    .bind(v.standard_minutes_per_day)
    .bind(&v.note)
    .bind(v.active)
    .bind(now)
    .bind(now)
    .bind(seq)
    .fetch_one(conn)
    .await
}

pub async fn update(
    conn: &mut SqliteConnection,
    id: &str,
    v: &ValidatedEmployee,
    now: i64,
    seq: i64,
) -> Result<EmployeeRow, sqlx::Error> {
    sqlx::query_as::<_, EmployeeRow>(
        "UPDATE employees SET
            name = ?, code = ?, pay_basis = ?, rate_minor = ?,
            ot_multiplier_bp = ?, standard_minutes_per_day = ?, note = ?,
            active = ?, updated_at = ?, seq = ?
         WHERE id = ? AND deleted_at IS NULL
         RETURNING *",
    )
    .bind(&v.name)
    .bind(&v.code)
    .bind(v.pay_basis.as_str())
    .bind(v.rate_minor)
    .bind(v.ot_multiplier_bp)
    .bind(v.standard_minutes_per_day)
    .bind(&v.note)
    .bind(v.active)
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
) -> Result<EmployeeRow, sqlx::Error> {
    sqlx::query_as::<_, EmployeeRow>(
        "UPDATE employees SET deleted_at = ?, updated_at = ?, seq = ?
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

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> EmployeeInput {
        EmployeeInput {
            name: "  Asha  ".into(),
            code: Some(" EMP-01 ".into()),
            pay_basis: "day_rate".into(),
            rate_minor: 80_000,
            ot_multiplier_bp: None,
            standard_minutes_per_day: None,
            note: Some("hired 2024".into()),
            active: true,
        }
    }

    #[test]
    fn trims_and_defaults() {
        let v = validate_input(&input()).unwrap();
        assert_eq!(v.name, "Asha");
        assert_eq!(v.code.as_deref(), Some("EMP-01"));
        assert_eq!(v.pay_basis, PayBasis::DayRate);
        assert_eq!(v.ot_multiplier_bp, DEFAULT_OT_MULTIPLIER_BP);
        assert_eq!(v.standard_minutes_per_day, DEFAULT_STANDARD_MINUTES);
    }

    #[test]
    fn blank_code_becomes_none() {
        let mut i = input();
        i.code = Some("   ".into());
        assert_eq!(validate_input(&i).unwrap().code, None);
    }

    #[test]
    fn rejects_bad_fields() {
        let mut i = input();
        i.name = "   ".into();
        i.pay_basis = "weekly".into();
        i.rate_minor = -1;
        i.ot_multiplier_bp = Some(5_000);
        i.standard_minutes_per_day = Some(0);
        let errors = validate_input(&i).unwrap_err();
        let fields: Vec<_> = errors.iter().map(|e| e.field.as_str()).collect();
        assert!(fields.contains(&"name"));
        assert!(fields.contains(&"pay_basis"));
        assert!(fields.contains(&"rate_minor"));
        assert!(fields.contains(&"ot_multiplier_bp"));
        assert!(fields.contains(&"standard_minutes_per_day"));
    }

    #[test]
    fn rejects_bad_code_characters() {
        let mut i = input();
        i.code = Some("BAD CODE".into());
        assert!(
            validate_input(&i)
                .unwrap_err()
                .iter()
                .any(|e| e.field == "code")
        );
    }
}
