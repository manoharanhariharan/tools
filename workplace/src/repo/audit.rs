//! Append-only audit trail for data changes. Handlers write an entry in the
//! same transaction as the mutation; the admin reads them back via `/api/audit`.

use serde::Serialize;
use sqlx::{FromRow, SqliteConnection, SqlitePool};

pub const DEFAULT_LIMIT: i64 = 100;
pub const MAX_LIMIT: i64 = 500;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AuditRow {
    pub id: i64,
    pub created_at: i64,
    pub actor_role: String,
    pub actor_id: Option<String>,
    pub actor_name: String,
    pub entity: String,
    pub entity_id: String,
    pub action: String,
    pub summary: String,
}

/// Insert one audit row. Call this inside the write transaction so the audit
/// entry commits atomically with the change it describes.
#[allow(clippy::too_many_arguments)]
pub async fn record(
    conn: &mut SqliteConnection,
    now: i64,
    actor_role: &str,
    actor_id: Option<&str>,
    actor_name: &str,
    entity: &str,
    entity_id: &str,
    action: &str,
    summary: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO audit_log
            (created_at, actor_role, actor_id, actor_name, entity, entity_id, action, summary)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(now)
    .bind(actor_role)
    .bind(actor_id)
    .bind(actor_name)
    .bind(entity)
    .bind(entity_id)
    .bind(action)
    .bind(summary)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Recent audit rows, newest first. `before_id` is an exclusive cursor for paging.
pub async fn list(
    pool: &SqlitePool,
    limit: i64,
    before_id: Option<i64>,
) -> Result<Vec<AuditRow>, sqlx::Error> {
    let limit = limit.clamp(1, MAX_LIMIT);
    sqlx::query_as::<_, AuditRow>(
        "SELECT * FROM audit_log
          WHERE (? IS NULL OR id < ?)
          ORDER BY id DESC
          LIMIT ?",
    )
    .bind(before_id)
    .bind(before_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Record an audit entry outside an existing transaction (used by handlers whose
/// write is performed in a helper that owns its own transaction).
#[allow(clippy::too_many_arguments)]
pub async fn record_standalone(
    pool: &SqlitePool,
    now: i64,
    actor_role: &str,
    actor_id: Option<&str>,
    actor_name: &str,
    entity: &str,
    entity_id: &str,
    action: &str,
    summary: &str,
) -> Result<(), sqlx::Error> {
    let mut conn = pool.acquire().await?;
    record(
        &mut conn, now, actor_role, actor_id, actor_name, entity, entity_id, action, summary,
    )
    .await
}

/// Trim audit history to the most recent `keep` rows. Used opportunistically so
/// the table stays bounded over years of use.
pub async fn trim(pool: &SqlitePool, keep: i64) -> Result<u64, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM audit_log WHERE id <= (SELECT MAX(id) FROM audit_log) - ?")
            .bind(keep.max(0))
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}
