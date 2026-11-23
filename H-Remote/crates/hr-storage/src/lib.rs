//! SQLite bootstrap, schema migrations and the repository facade.
//!
//! Phase 1 owns `Servers`, `Credentials`, `Configs` and `TableMeta`. Phase 0's
//! metadata table remains the migration ledger.

pub mod codec;
pub mod dialect;
pub mod import;
pub mod repository;
pub mod schema;
pub mod sources;

use std::path::{Path, PathBuf};
use std::time::Duration;

use hr_core::{AppError, AppResult};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

pub use codec::{
    SecretCodec, VaultCodec, seal_credential, seal_server, unseal_credential, unseal_server,
};
pub use dialect::SqlDialect;
pub use repository::Repository;
pub use schema::{
    CONFIGS, CREDENTIALS, CURRENT_SCHEMA_VERSION, ENCRYPTION_TEST_KEY, SERVERS, TABLE_META,
};
pub use sources::{DataSourceConfig, SourceKind, SourceManager, SourceStatus};

pub const META_TABLE: &str = "_hremote_meta";

#[derive(Debug)]
pub struct Database {
    pool: SqlitePool,
    path: PathBuf,
    read_only: bool,
}

impl Database {
    pub async fn open_sqlite(path: impl AsRef<Path>) -> AppResult<Self> {
        Self::open_sqlite_inner(path.as_ref(), false).await
    }

    /// Read-only connection used for secondary sources and tests.
    pub async fn open_sqlite_read_only(path: impl AsRef<Path>) -> AppResult<Self> {
        Self::open_sqlite_inner(path.as_ref(), true).await
    }

    async fn open_sqlite_inner(path: &Path, read_only: bool) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                AppError::permission_denied(format!(
                    "cannot create database directory {}: {error}",
                    parent.display()
                ))
            })?;
        }

        let mut options = SqliteConnectOptions::new()
            .filename(path)
            .read_only(read_only)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        if !read_only {
            options = options
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal);
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(if read_only { 2 } else { 4 })
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await
            .map_err(storage_error)?;

        let database = Self {
            pool,
            path: path.to_path_buf(),
            read_only,
        };
        database.health_check().await?;
        if !read_only {
            database.apply_migrations().await?;
        }
        Ok(database)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub async fn health_check(&self) -> AppResult<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(storage_error)
    }

    /// Probes real write capability, not just connection mode.
    pub async fn is_writable(&self) -> AppResult<bool> {
        if self.read_only {
            return Ok(false);
        }
        let mut connection = self.pool.acquire().await.map_err(storage_error)?;
        match sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *connection)
            .await
        {
            Ok(_) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
                Ok(true)
            }
            Err(error) => {
                tracing::warn!(%error, path = %self.path.display(), "database is not writable");
                Ok(false)
            }
        }
    }

    pub async fn schema_version(&self) -> AppResult<i64> {
        let value: Option<(String,)> = sqlx::query_as(&format!(
            "SELECT value FROM {META_TABLE} WHERE key = 'schema_version'"
        ))
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error)?;
        match value {
            Some((value,)) => value
                .parse::<i64>()
                .map_err(|error| AppError::storage(format!("invalid schema version: {error}"))),
            None => Ok(0),
        }
    }

    pub async fn close(self) {
        self.pool.close().await;
    }

    async fn apply_migrations(&self) -> AppResult<()> {
        let mut connection = self.pool.acquire().await.map_err(storage_error)?;
        sqlx::raw_sql(&format!(
            "CREATE TABLE IF NOT EXISTS {META_TABLE} (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );"
        ))
        .execute(&mut *connection)
        .await
        .map_err(storage_error)?;

        let current: Option<(String,)> = sqlx::query_as(&format!(
            "SELECT value FROM {META_TABLE} WHERE key = 'schema_version'"
        ))
        .fetch_optional(&mut *connection)
        .await
        .map_err(storage_error)?;
        let mut current = current
            .and_then(|(value,)| value.parse::<i64>().ok())
            .unwrap_or(0);
        if current > CURRENT_SCHEMA_VERSION {
            return Err(AppError::storage(format!(
                "database schema version {current} is newer than this build ({CURRENT_SCHEMA_VERSION})"
            )));
        }
        drop(connection);

        for (version, sql) in schema::MIGRATIONS {
            if *version <= current {
                continue;
            }
            let mut transaction = self.pool.begin().await.map_err(storage_error)?;
            sqlx::raw_sql(sql)
                .execute(&mut *transaction)
                .await
                .map_err(storage_error)?;
            sqlx::query(&format!(
                "INSERT INTO {META_TABLE} (key, value) VALUES ('schema_version', ?)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')"
            ))
            .bind(version.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
            transaction.commit().await.map_err(storage_error)?;
            current = *version;
            tracing::info!(schema_version = *version, "applied database migration");
        }
        Ok(())
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        tracing::debug!(path = %self.path.display(), "dropping database handle");
    }
}

fn storage_error(error: sqlx::Error) -> AppError {
    AppError::storage(format!("database error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn creates_a_healthy_writable_database() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hremote.db");
        let database = Database::open_sqlite(&path).await.unwrap();
        assert!(path.is_file());
        assert!(!database.is_read_only());
        assert!(database.is_writable().await.unwrap());
        assert_eq!(
            database.schema_version().await.unwrap(),
            CURRENT_SCHEMA_VERSION
        );
        for table in [SERVERS, CREDENTIALS, CONFIGS, TABLE_META] {
            let count: i64 = sqlx::query_scalar(&format!(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '{table}'"
            ))
            .fetch_one(database.pool())
            .await
            .unwrap();
            assert_eq!(count, 1, "missing {table}");
        }
        database.close().await;
    }

    #[tokio::test]
    async fn reopening_preserves_the_schema_version() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hremote.db");
        {
            let database = Database::open_sqlite(&path).await.unwrap();
            database.close().await;
        }
        let database = Database::open_sqlite(&path).await.unwrap();
        assert_eq!(
            database.schema_version().await.unwrap(),
            CURRENT_SCHEMA_VERSION
        );
        database.close().await;
    }

    #[tokio::test]
    async fn read_only_connections_do_not_write() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hremote.db");
        {
            let database = Database::open_sqlite(&path).await.unwrap();
            database.close().await;
        }
        let read_only = Database::open_sqlite_read_only(&path).await.unwrap();
        assert!(read_only.is_read_only());
        assert!(!read_only.is_writable().await.unwrap());
        assert_eq!(
            read_only.schema_version().await.unwrap(),
            CURRENT_SCHEMA_VERSION
        );
        read_only.close().await;
    }

    #[tokio::test]
    async fn migrates_an_existing_phase_zero_database() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hremote.db");
        {
            let database = Database::open_sqlite(&path).await.unwrap();
            database.close().await;
        }
        // Simulate a Phase 0 database: drop Phase 1 tables and reset the version.
        let database = Database::open_sqlite(&path).await.unwrap();
        for table in [SERVERS, CREDENTIALS, CONFIGS, TABLE_META] {
            sqlx::query(&format!("DROP TABLE IF EXISTS {table}"))
                .execute(database.pool())
                .await
                .unwrap();
        }
        sqlx::query("UPDATE _hremote_meta SET value = '1' WHERE key = 'schema_version'")
            .execute(database.pool())
            .await
            .unwrap();
        database.close().await;

        let database = Database::open_sqlite(&path).await.unwrap();
        assert_eq!(
            database.schema_version().await.unwrap(),
            CURRENT_SCHEMA_VERSION
        );
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'Servers'",
        )
        .fetch_one(database.pool())
        .await
        .unwrap();
        assert_eq!(count, 1);
        database.close().await;
    }

    #[tokio::test]
    async fn newer_schema_is_rejected() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hremote.db");
        {
            let database = Database::open_sqlite(&path).await.unwrap();
            sqlx::query("UPDATE _hremote_meta SET value = '99' WHERE key = 'schema_version'")
                .execute(database.pool())
                .await
                .unwrap();
            database.close().await;
        }
        let error = Database::open_sqlite(&path).await.unwrap_err();
        assert!(matches!(error, AppError::Storage(_)));
    }

    #[tokio::test]
    async fn missing_read_only_database_is_reported() {
        let dir = TempDir::new().unwrap();
        let error = Database::open_sqlite_read_only(dir.path().join("missing.db"))
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Storage(_)));
    }

    #[tokio::test]
    async fn closes_without_leaving_file_locks_on_windows() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hremote.db");
        let database = Database::open_sqlite(&path).await.unwrap();
        database.close().await;
        // On Windows the SQLite worker thread can release its last handle a
        // few milliseconds after `close()` resolves. Retry briefly; a real leak
        // would still fail after the deadline.
        let renamed = dir.path().join("renamed.db");
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            match std::fs::rename(&path, &renamed) {
                Ok(()) => break,
                Err(error) if std::time::Instant::now() < deadline => {
                    tracing::debug!(%error, "database file still locked; retrying rename");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(error) => panic!("database file remained locked: {error}"),
            }
        }
        assert!(renamed.is_file());
    }
}
