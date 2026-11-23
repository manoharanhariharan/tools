//! Admin-managed earning types (Sales, Service, Tips, …).
//!
//! Types are configuration, not day data: they are managed online through the
//! REST API and cached on devices for offline entry forms. Historical entries
//! snapshot the type name, so renaming or deleting a type never rewrites pay.

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::error::FieldError;

pub const MAX_TYPE_NAME_LEN: usize = 60;
pub const MAX_SORT_ORDER: i64 = 1_000;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct EarningTypeRow {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EarningTypeInput {
    pub name: String,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub sort_order: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedEarningType {
    pub name: String,
    pub active: bool,
    pub sort_order: i64,
}

pub fn validate_input(input: &EarningTypeInput) -> Result<ValidatedEarningType, Vec<FieldError>> {
    let mut errors = Vec::new();

    let name = input.name.trim();
    if name.is_empty() {
        errors.push(FieldError::new("name", "required"));
    } else if name.chars().count() > MAX_TYPE_NAME_LEN {
        errors.push(FieldError::new(
            "name",
            format!("must be at most {MAX_TYPE_NAME_LEN} characters"),
        ));
    }
    if name.chars().any(|c| c.is_control()) {
        errors.push(FieldError::new(
            "name",
            "must not contain control characters",
        ));
    }

    let sort_order = input.sort_order.unwrap_or(50);
    if !(0..=MAX_SORT_ORDER).contains(&sort_order) {
        errors.push(FieldError::new(
            "sort_order",
            format!("must be between 0 and {MAX_SORT_ORDER}"),
        ));
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(ValidatedEarningType {
        name: name.to_string(),
        active: input.active.unwrap_or(true),
        sort_order,
    })
}

pub async fn list_live(pool: &SqlitePool) -> Result<Vec<EarningTypeRow>, sqlx::Error> {
    sqlx::query_as::<_, EarningTypeRow>(
        "SELECT * FROM earning_types WHERE deleted_at IS NULL
         ORDER BY sort_order ASC, name COLLATE NOCASE ASC",
    )
    .fetch_all(pool)
    .await
}

pub async fn find(
    conn: &mut SqliteConnection,
    id: &str,
) -> Result<Option<EarningTypeRow>, sqlx::Error> {
    sqlx::query_as::<_, EarningTypeRow>("SELECT * FROM earning_types WHERE id = ?")
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn find_many(
    conn: &mut SqliteConnection,
    ids: &[String],
) -> Result<Vec<EarningTypeRow>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder = sqlx::QueryBuilder::new("SELECT * FROM earning_types WHERE id IN (");
    let mut separated = builder.separated(", ");
    for id in ids {
        separated.push_bind(id);
    }
    separated.push_unseparated(")");
    builder
        .build_query_as::<EarningTypeRow>()
        .fetch_all(conn)
        .await
}

pub async fn find_live_by_name(
    conn: &mut SqliteConnection,
    name: &str,
) -> Result<Option<EarningTypeRow>, sqlx::Error> {
    sqlx::query_as::<_, EarningTypeRow>(
        "SELECT * FROM earning_types WHERE name = ? COLLATE NOCASE AND deleted_at IS NULL",
    )
    .bind(name)
    .fetch_optional(conn)
    .await
}

pub async fn insert(
    conn: &mut SqliteConnection,
    id: &str,
    v: &ValidatedEarningType,
    now: i64,
) -> Result<EarningTypeRow, sqlx::Error> {
    sqlx::query_as::<_, EarningTypeRow>(
        "INSERT INTO earning_types
            (id, name, active, sort_order, created_at, updated_at, deleted_at)
         VALUES (?, ?, ?, ?, ?, ?, NULL)
         RETURNING *",
    )
    .bind(id)
    .bind(&v.name)
    .bind(v.active)
    .bind(v.sort_order)
    .bind(now)
    .bind(now)
    .fetch_one(conn)
    .await
}

pub async fn update(
    conn: &mut SqliteConnection,
    id: &str,
    v: &ValidatedEarningType,
    now: i64,
) -> Result<EarningTypeRow, sqlx::Error> {
    sqlx::query_as::<_, EarningTypeRow>(
        "UPDATE earning_types SET name = ?, active = ?, sort_order = ?, updated_at = ?
         WHERE id = ? AND deleted_at IS NULL
         RETURNING *",
    )
    .bind(&v.name)
    .bind(v.active)
    .bind(v.sort_order)
    .bind(now)
    .bind(id)
    .fetch_one(conn)
    .await
}

pub async fn soft_delete(
    conn: &mut SqliteConnection,
    id: &str,
    now: i64,
) -> Result<EarningTypeRow, sqlx::Error> {
    sqlx::query_as::<_, EarningTypeRow>(
        "UPDATE earning_types SET deleted_at = ?, updated_at = ?
         WHERE id = ? AND deleted_at IS NULL
         RETURNING *",
    )
    .bind(now)
    .bind(now)
    .bind(id)
    .fetch_one(conn)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(name: &str) -> EarningTypeInput {
        EarningTypeInput {
            name: name.into(),
            active: None,
            sort_order: None,
        }
    }

    #[test]
    fn trims_names_and_defaults() {
        let v = validate_input(&input("  Sales  ")).unwrap();
        assert_eq!(v.name, "Sales");
        assert!(v.active);
        assert_eq!(v.sort_order, 50);
    }

    #[test]
    fn rejects_blank_long_and_control_names() {
        assert!(validate_input(&input("   ")).is_err());
        assert!(validate_input(&input(&"x".repeat(61))).is_err());
        assert!(validate_input(&input("bad\nname")).is_err());
    }

    #[test]
    fn rejects_bad_sort_order() {
        let mut i = input("Sales");
        i.sort_order = Some(MAX_SORT_ORDER + 1);
        assert!(
            validate_input(&i)
                .unwrap_err()
                .iter()
                .any(|e| e.field == "sort_order")
        );
    }
}
