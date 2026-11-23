use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use rand::RngCore;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous,
};
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::auth;

pub struct DbInit {
    pub pool: SqlitePool,
    /// Set exactly once, when no admin password existed and one was generated.
    /// The caller prints it to the console; it is never stored in plain text.
    pub generated_password: Option<String>,
}

/// Open (creating if needed) the SQLite database, run migrations, and make
/// sure an admin credential exists.
pub async fn open(data_dir: &Path) -> anyhow::Result<DbInit> {
    std::fs::create_dir_all(data_dir)
        .with_context(|| format!("cannot create data directory {}", data_dir.display()))?;

    let options = SqliteConnectOptions::new()
        .filename(data_dir.join("daybook.db"))
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(options)
        .await
        .context("cannot open SQLite database")?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("database migration failed")?;

    let generated_password = ensure_admin_password(&pool)
        .await
        .context("cannot initialise admin credential")?;

    Ok(DbInit {
        pool,
        generated_password,
    })
}

/// Create an admin password only when none exists. Uses
/// `DAYBOOK_ADMIN_PASSWORD` when provided, otherwise generates a strong
/// password that the operator must record from the console.
async fn ensure_admin_password(pool: &SqlitePool) -> anyhow::Result<Option<String>> {
    let existing: Option<String> =
        sqlx::query_scalar("SELECT value FROM settings WHERE key = 'admin_password_hash'")
            .fetch_optional(pool)
            .await?;
    if existing.is_some() {
        return Ok(None);
    }

    let (password, generated) = match crate::config::Config::bootstrap_password() {
        Some(pw) => {
            auth::validate_password_policy(&pw)
                .map_err(|msg| anyhow::anyhow!("DAYBOOK_ADMIN_PASSWORD rejected: {msg}"))?;
            (pw, false)
        }
        None => (generate_password(), true),
    };

    let hash = auth::hash_password(&password)?;
    sqlx::query("INSERT INTO settings (key, value) VALUES ('admin_password_hash', ?)")
        .bind(hash)
        .execute(pool)
        .await?;

    Ok(generated.then_some(password))
}

pub async fn set_admin_password(pool: &SqlitePool, password: &str) -> anyhow::Result<()> {
    auth::validate_password_policy(password).map_err(|msg| anyhow::anyhow!(msg))?;
    let hash = auth::hash_password(password)?;
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ('admin_password_hash', ?)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
    )
    .bind(hash)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM sessions").execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Reserve the next globally monotonic change sequence inside `tx`.
///
/// Every synced mutation takes a fresh value so pull cursors are gapless and
/// clients can resume exactly where they stopped.
pub async fn next_seq(tx: &mut Transaction<'_, Sqlite>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE meta SET int_value = int_value + 1 WHERE key = 'seq' RETURNING int_value",
    )
    .fetch_one(&mut **tx)
    .await
}

pub async fn current_seq(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT int_value FROM meta WHERE key = 'seq'")
        .fetch_one(pool)
        .await
}

pub async fn current_seq_tx(tx: &mut Transaction<'_, Sqlite>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT int_value FROM meta WHERE key = 'seq'")
        .fetch_one(&mut **tx)
        .await
}

/// Bounded-interval cleanup task. Aborted by the caller on shutdown.
pub fn spawn_session_cleanup(pool: SqlitePool) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(3_600));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        ticker.tick().await;
        loop {
            ticker.tick().await;
            match auth::cleanup_expired_sessions(&pool).await {
                Ok(removed) if removed > 0 => {
                    tracing::debug!(removed, "expired sessions removed");
                }
                Ok(_) => {}
                Err(err) => tracing::warn!(error = %err, "session cleanup failed"),
            }
        }
    })
}

fn generate_password() -> String {
    // Ambiguous characters removed so the one-time console password is easy to
    // transcribe.
    const ALPHABET: &[u8] = b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut bytes = [0u8; 20];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes
        .iter()
        .map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_passwords_are_long_and_mixed() {
        let a = generate_password();
        let b = generate_password();
        assert_eq!(a.len(), 20);
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric()));
        assert!(a.chars().any(|c| c.is_ascii_lowercase()));
        assert!(a.chars().any(|c| c.is_ascii_uppercase()));
    }
}
