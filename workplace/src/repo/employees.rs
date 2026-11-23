//! Employee records. Pay is no longer derived from a rate or hours: daily
//! earnings are entered as business lines. Legacy rate columns are retained in
//! the database for historical compatibility but are not exposed or edited.

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::error::FieldError;

pub const MAX_NAME_LEN: usize = 120;
pub const MAX_NOTE_LEN: usize = 2_000;
pub const MAX_CODE_LEN: usize = 32;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct EmployeeRow {
    pub id: String,
    pub name: String,
    pub code: Option<String>,
    /// Legacy columns kept for schema compatibility; hidden from the API.
    #[serde(skip_serializing)]
    pub pay_basis: String,
    #[serde(skip_serializing)]
    pub rate_minor: i64,
    #[serde(skip_serializing)]
    pub ot_multiplier_bp: i64,
    #[serde(skip_serializing)]
    pub standard_minutes_per_day: i64,
    pub note: String,
    pub active: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub seq: i64,
}

impl EmployeeRow {
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmployeeInput {
    pub name: String,
    #[serde(default)]
    pub code: Option<String>,
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
        errors.push(FieldError::new(
            "name",
            "must not contain control characters",
        ));
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
        errors.push(FieldError::new(
            "note",
            "must not contain control characters",
        ));
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(ValidatedEmployee {
        name: name.to_string(),
        code,
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

pub async fn get_live(pool: &SqlitePool, id: &str) -> Result<Option<EmployeeRow>, sqlx::Error> {
    sqlx::query_as::<_, EmployeeRow>("SELECT * FROM employees WHERE id = ? AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn get_live_by_code(
    pool: &SqlitePool,
    code: &str,
) -> Result<Option<EmployeeRow>, sqlx::Error> {
    sqlx::query_as::<_, EmployeeRow>(
        "SELECT * FROM employees WHERE code = ? COLLATE NOCASE AND deleted_at IS NULL",
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
        "SELECT * FROM employees WHERE code = ? COLLATE NOCASE AND deleted_at IS NULL",
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
         VALUES (?, ?, ?, 'fixed', 0, 15000, 480, ?, ?, ?, ?, NULL, ?)
         RETURNING *",
    )
    .bind(id)
    .bind(&v.name)
    .bind(&v.code)
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
            name = ?, code = ?, note = ?, active = ?, updated_at = ?, seq = ?
         WHERE id = ? AND deleted_at IS NULL
         RETURNING *",
    )
    .bind(&v.name)
    .bind(&v.code)
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

/// Insert-or-replace used by the sync engine; preserves legacy rate columns on
/// existing rows and preserves `created_at`.
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
         VALUES (?, ?, ?, 'fixed', 0, 15000, 480, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
            name = excluded.name,
            code = excluded.code,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> EmployeeInput {
        EmployeeInput {
            name: "  Asha  ".into(),
            code: Some(" EMP-01 ".into()),
            note: Some("hired 2024".into()),
            active: true,
        }
    }

    #[test]
    fn trims_and_defaults() {
        let v = validate_input(&input()).unwrap();
        assert_eq!(v.name, "Asha");
        assert_eq!(v.code.as_deref(), Some("EMP-01"));
        assert!(v.active);
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
        let errors = validate_input(&i).unwrap_err();
        assert!(errors.iter().any(|e| e.field == "name"));
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
