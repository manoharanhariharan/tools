//! Local cache of record.
//!
//! The live tick stream never touches this layer. Everything here answers
//! questions the venue should not be asked twice:
//!
//! | Question | Collection |
//! |---|---|
//! | "have I already backfilled this underlying for this date?" | `DATA_STORED_TRACKER` |
//! | "what were the intraday OHLC/OI snapshots?" | `SNAPSHOT_INTRADAY` |
//! | "what did I do today?" | `FV_ORDERS` (new, owned by this app) |
//!
//! Two backends share one API: `MemoryStore` (tests, CI, no-database laptops) and
//! `MongoStore` (production). The app must keep working when Mongo is absent, so
//! [`Store::connect_or_memory`] degrades instead of failing.

pub mod memory;
pub mod model;

#[cfg(feature = "mongo")]
pub mod mongo;

pub use memory::MemoryStore;
pub use model::{OrderDoc, SnapshotDoc, TrackerEntry};

/// Errors raised by the cache layer.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("mongo: {0}")]
    Mongo(String),
    #[error("not supported by this backend: {0}")]
    Unsupported(&'static str),
    #[error("invalid document: {0}")]
    InvalidDocument(String),
}

/// Maximum consecutive "instrument not found" responses before backfill for that
/// key is abandoned. Prevents an endless retry loop against the venue for a
/// symbol that legitimately does not exist.
pub const MAX_NOT_FOUND_RETRIES: i32 = 3;

/// Should the app fetch snapshots for `target_date`?
///
/// This is the rule that replaced the legacy `Request sent for Get Scrips Info`
/// storm: ask Mongo first, fetch only when the tracker has no successful record
/// for that date.
pub fn needs_backfill(entry: Option<&TrackerEntry>, target_date: &str) -> bool {
    match entry {
        None => true,
        Some(e) => {
            if e.not_found_count >= MAX_NOT_FOUND_RETRIES {
                return false;
            }
            e.date_updated_on != target_date || !e.found_instrument
        }
    }
}

/// One cache handle.
pub enum Store {
    Memory(MemoryStore),
    #[cfg(feature = "mongo")]
    Mongo(mongo::MongoStore),
}

impl Store {
    pub fn memory() -> Self {
        Store::Memory(MemoryStore::default())
    }

    /// Connect to Mongo, or fall back to memory with a warning. Never fails —
    /// a missing database must degrade the app, not prevent it from starting.
    #[cfg(feature = "mongo")]
    pub async fn connect_or_memory(uri: &str, database: &str) -> Self {
        match mongo::MongoStore::connect(uri, database).await {
            Ok(store) => {
                tracing::info!(database, "cache store connected");
                Store::Mongo(store)
            }
            Err(err) => {
                tracing::warn!(
                    %err,
                    "MongoDB unavailable; running with in-memory cache (history disabled)"
                );
                Store::Memory(MemoryStore::default())
            }
        }
    }

    /// Build from environment. Without the `mongo` feature this is always memory.
    pub async fn from_env() -> Self {
        #[cfg(feature = "mongo")]
        {
            let uri = std::env::var("FV_MONGO_URI")
                .unwrap_or_else(|_| "mongodb://127.0.0.1:27017".to_string());
            let db = std::env::var("FV_MONGO_DB").unwrap_or_else(|_| "STOCK".to_string());
            Store::connect_or_memory(&uri, &db).await
        }
        #[cfg(not(feature = "mongo"))]
        {
            tracing::info!("built without the `mongo` feature; using in-memory cache");
            Store::Memory(MemoryStore::default())
        }
    }

    pub fn is_persistent(&self) -> bool {
        match self {
            Store::Memory(_) => false,
            #[cfg(feature = "mongo")]
            Store::Mongo(_) => true,
        }
    }

    /// Shut down cleanly, joining the driver's background workers.
    ///
    /// The Mongo driver panics at runtime teardown if its connection pool
    /// outlives the tokio runtime, so the app must call this before exiting.
    pub async fn shutdown(self) {
        match self {
            Store::Memory(_) => {}
            #[cfg(feature = "mongo")]
            Store::Mongo(s) => s.shutdown().await,
        }
    }

    pub async fn ping(&self) -> Result<(), StoreError> {
        match self {
            Store::Memory(_) => Ok(()),
            #[cfg(feature = "mongo")]
            Store::Mongo(s) => s.ping().await,
        }
    }

    pub async fn tracker_get(
        &self,
        underlying: &str,
        instrument: &str,
    ) -> Result<Option<TrackerEntry>, StoreError> {
        match self {
            Store::Memory(s) => s.tracker_get(underlying, instrument),
            #[cfg(feature = "mongo")]
            Store::Mongo(s) => s.tracker_get(underlying, instrument).await,
        }
    }

    pub async fn tracker_upsert(&self, entry: &TrackerEntry) -> Result<(), StoreError> {
        match self {
            Store::Memory(s) => s.tracker_upsert(entry),
            #[cfg(feature = "mongo")]
            Store::Mongo(s) => s.tracker_upsert(entry).await,
        }
    }

    pub async fn append_snapshot(&self, doc: &SnapshotDoc) -> Result<(), StoreError> {
        match self {
            Store::Memory(s) => s.append_snapshot(doc),
            #[cfg(feature = "mongo")]
            Store::Mongo(s) => s.append_snapshot(doc).await,
        }
    }

    pub async fn snapshots_for(
        &self,
        underlying: &str,
        date: &str,
    ) -> Result<Vec<SnapshotDoc>, StoreError> {
        match self {
            Store::Memory(s) => s.snapshots_for(underlying, date),
            #[cfg(feature = "mongo")]
            Store::Mongo(s) => s.snapshots_for(underlying, date).await,
        }
    }

    pub async fn journal_order(&self, doc: &OrderDoc) -> Result<(), StoreError> {
        match self {
            Store::Memory(s) => s.journal_order(doc),
            #[cfg(feature = "mongo")]
            Store::Mongo(s) => s.journal_order(doc).await,
        }
    }

    pub async fn recent_orders(&self, limit: u32) -> Result<Vec<OrderDoc>, StoreError> {
        match self {
            Store::Memory(s) => s.recent_orders(limit),
            #[cfg(feature = "mongo")]
            Store::Mongo(s) => s.recent_orders(limit).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(date: &str, found: bool, not_found: i32) -> TrackerEntry {
        TrackerEntry {
            underlying: "NIFTY".into(),
            instrument: "NIFTY:EQ:0:".into(),
            date_updated_on: date.into(),
            found_instrument: found,
            not_found_count: not_found,
            last_updated: Some("2026-11-26T10:31:39".into()),
        }
    }

    #[test]
    fn backfill_is_needed_without_a_tracker_entry() {
        assert!(needs_backfill(None, "2026-11-26"));
    }

    #[test]
    fn backfill_is_skipped_when_the_date_already_succeeded() {
        let e = entry("2026-11-26", true, 0);
        assert!(!needs_backfill(Some(&e), "2026-11-26"));
    }

    #[test]
    fn backfill_is_needed_for_a_new_date() {
        let e = entry("2026-11-25", true, 0);
        assert!(needs_backfill(Some(&e), "2026-11-26"));
    }

    #[test]
    fn backfill_retries_an_unsuccessful_lookup() {
        let e = entry("2026-11-26", false, 1);
        assert!(needs_backfill(Some(&e), "2026-11-26"));
    }

    #[test]
    fn backfill_gives_up_after_max_retries() {
        let e = entry("2026-11-26", false, MAX_NOT_FOUND_RETRIES);
        assert!(!needs_backfill(Some(&e), "2026-11-26"));
    }
}
