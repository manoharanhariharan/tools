use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use hr_core::{AppError, AppResult, new_id};
use hr_domain::{
    Credential, Folder, Server, is_descendant_path, normalize_tree_nodes, validate_tree_path,
};
use sqlx::sqlite::SqlitePool;
use sqlx::{Row, Sqlite, Transaction};

use crate::codec::{
    SecretCodec, VaultCodec, seal_credential, seal_server, unseal_credential_lenient,
    unseal_server_lenient,
};
use crate::dialect::SqlDialect;
use crate::schema::{CONFIGS, CREDENTIALS, ENCRYPTION_TEST_KEY, FOLDERS, SERVERS, TABLE_META};

const MAX_JSON_BYTES: usize = 16 * 1024 * 1024;
const MAX_CONFIG_BYTES: usize = 1024 * 1024;

struct CachedServers {
    stamp: i64,
    values: Arc<Vec<Server>>,
}

struct CachedCredentials {
    stamp: i64,
    values: Arc<Vec<Credential>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretRepairReport {
    pub servers: u32,
    pub credentials: u32,
    pub secrets_cleared: u32,
}

#[derive(Default)]
struct Cache {
    servers: Option<CachedServers>,
    credentials: Option<CachedCredentials>,
}

/// Persistence facade for one data source.
///
/// Phase 1 uses SQLite; the dialect field and SQL builders are the extension
/// point for MySQL/PostgreSQL in Phase 8.
#[derive(Clone)]
pub struct Repository {
    pool: SqlitePool,
    codec: Arc<dyn SecretCodec>,
    dialect: SqlDialect,
    cache: Arc<RwLock<Cache>>,
}

impl std::fmt::Debug for Repository {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Repository")
            .field("dialect", &self.dialect)
            .finish_non_exhaustive()
    }
}

impl Repository {
    /// Opens a repository and verifies (or writes) the encryption sentinel.
    /// A different vault key fails with `AppError::EncryptionKey`.
    pub async fn open(pool: SqlitePool, codec: Arc<dyn SecretCodec>) -> AppResult<Self> {
        let repository = Self {
            pool,
            codec,
            dialect: SqlDialect::Sqlite,
            cache: Arc::new(RwLock::new(Cache::default())),
        };
        match repository.get_config(ENCRYPTION_TEST_KEY).await? {
            Some(sentinel) => repository
                .codec
                .verify_sentinel(&sentinel)
                .map_err(|error| {
                    AppError::encryption_key(format!(
                        "the database was encrypted with a different key: {error}"
                    ))
                })?,
            None => {
                // Never stamp a new key onto an existing catalog: its rows may
                // still be encrypted with the previous key. Force the explicit
                // repair path instead of producing silent mixed-key storage.
                if repository.server_count().await? > 0 || repository.credential_count().await? > 0
                {
                    return Err(AppError::encryption_key(
                        "the database contains encrypted data but its sentinel is missing; run catalog repair",
                    ));
                }
                let sentinel = repository.codec.create_sentinel()?;
                repository
                    .set_config(ENCRYPTION_TEST_KEY, &sentinel)
                    .await?;
            }
        }
        Ok(repository)
    }

    pub async fn with_vault(pool: SqlitePool, vault: Arc<hr_crypto::Vault>) -> AppResult<Self> {
        Self::open(pool, Arc::new(VaultCodec::new(vault))).await
    }

    /// Repairs rows written with another key: readable secrets are re-encrypted
    /// with the current codec, unreadable ones are cleared, and the encryption
    /// sentinel is replaced. Usable both when `open` succeeded and when the
    /// sentinel itself failed verification.
    pub async fn repair_secrets_with_pool(
        pool: SqlitePool,
        codec: Arc<dyn SecretCodec>,
    ) -> AppResult<SecretRepairReport> {
        let repository = Self {
            pool,
            codec,
            dialect: SqlDialect::Sqlite,
            cache: Arc::new(RwLock::new(Cache::default())),
        };
        repository.repair_secrets().await
    }

    pub async fn repair_secrets(&self) -> AppResult<SecretRepairReport> {
        let now = now_ms();
        let mut report = SecretRepairReport::default();
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;

        let server_rows = sqlx::query(&format!(
            "SELECT Id, Protocol, ClassVersion, Json FROM {SERVERS}"
        ))
        .fetch_all(&mut *transaction)
        .await
        .map_err(storage_error)?;
        for row in server_rows {
            let id: String = row.try_get("Id").map_err(storage_error)?;
            let protocol: String = row.try_get("Protocol").map_err(storage_error)?;
            let class_version: i64 = row.try_get("ClassVersion").map_err(storage_error)?;
            let json: String = row.try_get("Json").map_err(storage_error)?;
            let (server, cleared) = unseal_server_lenient(
                &protocol,
                class_version.max(0) as u32,
                &json,
                self.codec.as_ref(),
            )
            .map_err(|error| AppError::storage(format!("cannot repair server {id}: {error}")))?;
            let sealed = seal_server(&server, self.codec.as_ref())?;
            sqlx::query(&format!(
                "UPDATE {SERVERS} SET Json = {}, UpdatedAt = {} WHERE Id = {}",
                self.dialect.placeholder(1),
                self.dialect.placeholder(2),
                self.dialect.placeholder(3)
            ))
            .bind(&sealed)
            .bind(now)
            .bind(&id)
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
            report.servers += 1;
            report.secrets_cleared += cleared;
        }

        let credential_rows = sqlx::query(&format!("SELECT Id, Json FROM {CREDENTIALS}"))
            .fetch_all(&mut *transaction)
            .await
            .map_err(storage_error)?;
        for row in credential_rows {
            let id: String = row.try_get("Id").map_err(storage_error)?;
            let json: String = row.try_get("Json").map_err(storage_error)?;
            let (credential, cleared) = unseal_credential_lenient(&json, self.codec.as_ref())
                .map_err(|error| {
                    AppError::storage(format!("cannot repair credential {id}: {error}"))
                })?;
            let sealed = seal_credential(&credential, self.codec.as_ref())?;
            sqlx::query(&format!(
                "UPDATE {CREDENTIALS} SET Json = {}, UpdatedAt = {} WHERE Id = {}",
                self.dialect.placeholder(1),
                self.dialect.placeholder(2),
                self.dialect.placeholder(3)
            ))
            .bind(&sealed)
            .bind(now)
            .bind(&id)
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
            report.credentials += 1;
            report.secrets_cleared += cleared;
        }

        if report.servers > 0 {
            bump_table(&mut transaction, SERVERS, 0, now).await?;
        }
        if report.credentials > 0 {
            bump_table(&mut transaction, CREDENTIALS, 0, now).await?;
        }
        transaction.commit().await.map_err(storage_error)?;

        self.set_config(ENCRYPTION_TEST_KEY, &self.codec.create_sentinel()?)
            .await?;
        self.invalidate_caches();
        Ok(report)
    }

    pub fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn invalidate_caches(&self) {
        let mut cache = self.cache.write().expect("repository cache poisoned");
        cache.servers = None;
        cache.credentials = None;
    }

    // ----------------------------------------------------------------- servers

    pub async fn list_servers_shared(&self) -> AppResult<Arc<Vec<Server>>> {
        let stamp = self.table_stamp(SERVERS).await?;
        {
            let cache = self.cache.read().expect("repository cache poisoned");
            if let Some(cached) = &cache.servers
                && cached.stamp == stamp
            {
                return Ok(Arc::clone(&cached.values));
            }
        }
        let sql = format!("SELECT Id, Protocol, ClassVersion, Json FROM {SERVERS} ORDER BY Id");
        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(storage_error)?;
        let mut servers = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row.try_get("Id").map_err(storage_error)?;
            let protocol: String = row.try_get("Protocol").map_err(storage_error)?;
            let class_version: i64 = row.try_get("ClassVersion").map_err(storage_error)?;
            let json: String = row.try_get("Json").map_err(storage_error)?;
            let (server, cleared) = unseal_server_lenient(
                &protocol,
                class_version.max(0) as u32,
                &json,
                self.codec.as_ref(),
            )
            .map_err(|error| AppError::storage(format!("cannot read server {id}: {error}")))?;
            if cleared > 0 {
                tracing::warn!(
                    server_id = %id,
                    cleared,
                    "server has secrets encrypted with another key; run catalog repair"
                );
            }
            servers.push(server);
        }
        let values = Arc::new(servers);
        let mut cache = self.cache.write().expect("repository cache poisoned");
        cache.servers = Some(CachedServers {
            stamp,
            values: Arc::clone(&values),
        });
        Ok(values)
    }

    pub async fn list_servers(&self) -> AppResult<Vec<Server>> {
        Ok((*self.list_servers_shared().await?).clone())
    }

    pub async fn list_servers_page(&self, offset: u32, limit: u32) -> AppResult<Vec<Server>> {
        let limit = limit.clamp(1, 1000);
        let sql = format!(
            "SELECT Id, Protocol, ClassVersion, Json FROM {SERVERS} ORDER BY Id LIMIT {} OFFSET {}",
            self.dialect.placeholder(1),
            self.dialect.placeholder(2)
        );
        let rows = sqlx::query(&sql)
            .bind(i64::from(limit))
            .bind(i64::from(offset))
            .fetch_all(&self.pool)
            .await
            .map_err(storage_error)?;
        self.decode_server_rows(rows)
    }

    pub async fn get_server(&self, id: &str) -> AppResult<Option<Server>> {
        let sql = format!(
            "SELECT Id, Protocol, ClassVersion, Json FROM {SERVERS} WHERE Id = {}",
            self.dialect.placeholder(1)
        );
        let row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage_error)?;
        row.map(|row| self.decode_server_row(&row)).transpose()
    }

    pub async fn upsert_server(&self, mut server: Server) -> AppResult<Server> {
        server.normalize();
        ensure_valid_server(&server)?;
        let json = seal_server(&server, self.codec.as_ref())?;
        if json.len() > MAX_JSON_BYTES {
            return Err(AppError::validation("server JSON is too large"));
        }
        let now = now_ms();
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        self.write_server(&mut transaction, &server, &json, now)
            .await?;
        bump_table(&mut transaction, SERVERS, 1, now).await?;
        transaction.commit().await.map_err(storage_error)?;
        self.invalidate_caches();
        Ok(server)
    }

    pub async fn upsert_servers(&self, servers: Vec<Server>) -> AppResult<Vec<Server>> {
        let mut prepared = Vec::with_capacity(servers.len());
        for mut server in servers {
            server.normalize();
            ensure_valid_server(&server)?;
            let json = seal_server(&server, self.codec.as_ref())?;
            if json.len() > MAX_JSON_BYTES {
                return Err(AppError::validation(format!(
                    "server {} JSON is too large",
                    server.base().id
                )));
            }
            prepared.push((server, json));
        }
        if prepared.is_empty() {
            return Ok(Vec::new());
        }
        let now = now_ms();
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        for (server, json) in &prepared {
            self.write_server(&mut transaction, server, json, now)
                .await?;
        }
        bump_table(&mut transaction, SERVERS, prepared.len() as i64, now).await?;
        transaction.commit().await.map_err(storage_error)?;
        self.invalidate_caches();
        Ok(prepared.into_iter().map(|(server, _)| server).collect())
    }

    pub async fn delete_server(&self, id: &str) -> AppResult<bool> {
        let now = now_ms();
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        let result = sqlx::query(&format!(
            "DELETE FROM {SERVERS} WHERE Id = {}",
            self.dialect.placeholder(1)
        ))
        .bind(id)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if result.rows_affected() > 0 {
            bump_table(&mut transaction, SERVERS, -1, now).await?;
        }
        transaction.commit().await.map_err(storage_error)?;
        self.invalidate_caches();
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_servers(&self, ids: &[String]) -> AppResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let now = now_ms();
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        let mut deleted = 0;
        for id in ids {
            let result = sqlx::query(&format!(
                "DELETE FROM {SERVERS} WHERE Id = {}",
                self.dialect.placeholder(1)
            ))
            .bind(id)
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
            deleted += result.rows_affected() as i64;
        }
        if deleted > 0 {
            bump_table(&mut transaction, SERVERS, -deleted, now).await?;
        }
        transaction.commit().await.map_err(storage_error)?;
        self.invalidate_caches();
        Ok(deleted.max(0) as usize)
    }

    pub async fn server_count(&self) -> AppResult<i64> {
        sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {SERVERS}"))
            .fetch_one(&self.pool)
            .await
            .map_err(storage_error)
    }

    // ------------------------------------------------------------------ folders

    pub async fn list_folders(&self) -> AppResult<Vec<Folder>> {
        let sql = format!("SELECT Id, Path, ColorHex FROM {FOLDERS} ORDER BY Path");
        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(storage_error)?;
        let mut folders = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row.try_get("Id").map_err(storage_error)?;
            let path: String = row.try_get("Path").map_err(storage_error)?;
            let color_hex: String = row.try_get("ColorHex").map_err(storage_error)?;
            let segments: Vec<String> = serde_json::from_str(&path).map_err(|error| {
                AppError::storage(format!("folder {id} has an invalid path: {error}"))
            })?;
            folders.push(Folder::new(id, segments).with_color(color_hex));
        }
        Ok(folders)
    }

    pub async fn create_folder(&self, path: Vec<String>) -> AppResult<Folder> {
        let path = normalize_folder_path(path)?;
        if let Some(existing) = self
            .list_folders()
            .await?
            .into_iter()
            .find(|folder| folder.path == path)
        {
            return Ok(existing);
        }
        let folder = Folder::new(new_id(), path);
        let now = now_ms();
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        insert_folder(&mut transaction, &folder, now).await?;
        bump_table(&mut transaction, FOLDERS, 1, now).await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(folder)
    }

    /// Sets or clears a folder tint. Returns the updated folder.
    pub async fn set_folder_color(&self, folder_id: &str, color_hex: &str) -> AppResult<Folder> {
        let trimmed = color_hex.trim();
        let color = hr_domain::protocol::normalize_color(trimmed);
        if !trimmed.is_empty() && color.is_empty() {
            return Err(AppError::validation(
                "folder color must be #RRGGBB or #AARRGGBB",
            ));
        }
        let mut folder = self
            .list_folders()
            .await?
            .into_iter()
            .find(|folder| folder.id == folder_id)
            .ok_or_else(|| AppError::not_found("folder"))?;
        if folder.color_hex == color {
            return Ok(folder);
        }
        let now = now_ms();
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        sqlx::query(&format!(
            "UPDATE {FOLDERS} SET ColorHex = {}, UpdatedAt = {} WHERE Id = {}",
            self.dialect.placeholder(1),
            self.dialect.placeholder(2),
            self.dialect.placeholder(3)
        ))
        .bind(&color)
        .bind(now)
        .bind(folder_id)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        bump_table(&mut transaction, FOLDERS, 0, now).await?;
        transaction.commit().await.map_err(storage_error)?;
        folder.color_hex = color;
        Ok(folder)
    }

    /// Creates every ancestor of `path` and returns the leaf folder.
    pub async fn ensure_folder_path(&self, path: Vec<String>) -> AppResult<Folder> {
        let path = normalize_folder_path(path)?;
        let mut current = Vec::new();
        let mut leaf = None;
        for segment in path {
            current.push(segment);
            leaf = Some(self.create_folder(current.clone()).await?);
        }
        leaf.ok_or_else(|| AppError::validation("folder path is empty"))
    }

    pub async fn rename_folder(&self, folder_id: &str, new_name: &str) -> AppResult<usize> {
        let folders = self.list_folders().await?;
        let folder = folders
            .iter()
            .find(|folder| folder.id == folder_id)
            .ok_or_else(|| AppError::not_found("folder"))?;
        let mut new_path = folder.parent_path();
        let name = validate_folder_name(new_name)?;
        new_path.push(name);
        if new_path == folder.path {
            return Ok(0);
        }
        if folders
            .iter()
            .any(|candidate| candidate.id != folder.id && candidate.path == new_path)
        {
            return Err(AppError::conflict("a folder with that name already exists"));
        }
        let old_path = folder.path.clone();
        let (updated_folders, updated_servers) = self
            .rewrite_folder_prefix(&old_path, &new_path, &folders)
            .await?;
        Ok(updated_folders + updated_servers)
    }

    /// Deletes a folder subtree. Contained servers are moved to the parent
    /// folder (or root), never deleted.
    pub async fn delete_folder(&self, path: Vec<String>) -> AppResult<(usize, usize)> {
        let path = normalize_folder_path(path)?;
        let folders = self.list_folders().await?;
        let affected: Vec<Folder> = folders
            .iter()
            .filter(|folder| path_starts_with(&folder.path, &path))
            .cloned()
            .collect();
        if affected.is_empty() {
            return Ok((0, 0));
        }
        let servers = self.list_servers().await?;
        let mut moved = 0;
        let now = now_ms();
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        for folder in &affected {
            sqlx::query(&format!(
                "DELETE FROM {FOLDERS} WHERE Id = {}",
                self.dialect.placeholder(1)
            ))
            .bind(&folder.id)
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
        }
        for mut server in servers {
            if path_starts_with(&server.base().tree_nodes, &path) {
                server.base_mut().tree_nodes.drain(0..path.len());
                server.normalize();
                let json = seal_server(&server, self.codec.as_ref())?;
                self.write_server(&mut transaction, &server, &json, now)
                    .await?;
                moved += 1;
            }
        }
        bump_table(&mut transaction, FOLDERS, -(affected.len() as i64), now).await?;
        if moved > 0 {
            bump_table(&mut transaction, SERVERS, 0, now).await?;
        }
        transaction.commit().await.map_err(storage_error)?;
        self.invalidate_caches();
        Ok((affected.len(), moved))
    }

    /// Moves a folder and its subtree under `new_parent`.
    pub async fn move_folder(&self, folder_id: &str, new_parent: Vec<String>) -> AppResult<usize> {
        let folders = self.list_folders().await?;
        let folder = folders
            .iter()
            .find(|folder| folder.id == folder_id)
            .ok_or_else(|| AppError::not_found("folder"))?;
        let new_parent = if new_parent.is_empty() {
            Vec::new()
        } else {
            normalize_folder_path(new_parent)?
        };
        if new_parent == folder.path {
            return Ok(0);
        }
        if is_descendant_path(&new_parent, &folder.path) {
            return Err(AppError::conflict(
                "a folder cannot be moved inside itself or a descendant",
            ));
        }
        let mut new_path = new_parent;
        new_path.push(folder.name().to_string());
        if folders
            .iter()
            .any(|candidate| candidate.id != folder.id && candidate.path == new_path)
        {
            return Err(AppError::conflict("a folder with that name already exists"));
        }
        let (updated_folders, updated_servers) = self
            .rewrite_folder_prefix(&folder.path, &new_path, &folders)
            .await?;
        Ok(updated_folders + updated_servers)
    }

    /// Duplicates the folder subtree (folders only; servers keep their path).
    pub async fn duplicate_folder(&self, folder_id: &str) -> AppResult<Folder> {
        let folders = self.list_folders().await?;
        let folder = folders
            .iter()
            .find(|folder| folder.id == folder_id)
            .ok_or_else(|| AppError::not_found("folder"))?;
        let parent = folder.parent_path();
        let base_name = folder.name().to_string();
        let mut new_path;
        let mut suffix = 1;
        loop {
            let name = if suffix == 1 {
                format!("{base_name} (copy)")
            } else {
                format!("{base_name} (copy {suffix})")
            };
            new_path = parent.clone();
            new_path.push(name);
            if !folders.iter().any(|candidate| candidate.path == new_path) {
                break;
            }
            suffix += 1;
        }
        let subtree: Vec<Folder> = folders
            .iter()
            .filter(|candidate| path_starts_with(&candidate.path, &folder.path))
            .cloned()
            .collect();
        let subtree_len = subtree.len() as i64;
        let now = now_ms();
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        let mut root = None;
        for candidate in subtree {
            let mut path = new_path.clone();
            path.extend_from_slice(&candidate.path[folder.path.len()..]);
            let copy = Folder::new(new_id(), path).with_color(&candidate.color_hex);
            insert_folder(&mut transaction, &copy, now).await?;
            if candidate.path == folder.path {
                root = Some(copy);
            }
        }
        bump_table(&mut transaction, FOLDERS, subtree_len, now).await?;
        transaction.commit().await.map_err(storage_error)?;
        root.ok_or_else(|| AppError::not_found("folder"))
    }

    pub async fn folder_count(&self) -> AppResult<i64> {
        sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {FOLDERS}"))
            .fetch_one(&self.pool)
            .await
            .map_err(storage_error)
    }

    async fn rewrite_folder_prefix(
        &self,
        old_path: &[String],
        new_path: &[String],
        folders: &[Folder],
    ) -> AppResult<(usize, usize)> {
        let now = now_ms();
        let servers = self.list_servers().await?;
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        let mut updated_folders = 0;
        for folder in folders
            .iter()
            .filter(|folder| path_starts_with(&folder.path, old_path))
        {
            let mut path = new_path.to_vec();
            path.extend_from_slice(&folder.path[old_path.len()..]);
            sqlx::query(&format!(
                "UPDATE {FOLDERS} SET Path = {}, UpdatedAt = {} WHERE Id = {}",
                self.dialect.placeholder(1),
                self.dialect.placeholder(2),
                self.dialect.placeholder(3)
            ))
            .bind(folder_path_json(&path))
            .bind(now)
            .bind(&folder.id)
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
            updated_folders += 1;
        }
        let mut updated_servers = 0;
        for mut server in servers {
            if path_starts_with(&server.base().tree_nodes, old_path) {
                let mut path = new_path.to_vec();
                path.extend_from_slice(&server.base().tree_nodes[old_path.len()..]);
                server.base_mut().tree_nodes = path;
                server.normalize();
                let json = seal_server(&server, self.codec.as_ref())?;
                self.write_server(&mut transaction, &server, &json, now)
                    .await?;
                updated_servers += 1;
            }
        }
        bump_table(&mut transaction, FOLDERS, 0, now).await?;
        if updated_servers > 0 {
            bump_table(&mut transaction, SERVERS, 0, now).await?;
        }
        transaction.commit().await.map_err(storage_error)?;
        self.invalidate_caches();
        Ok((updated_folders, updated_servers))
    }

    // ------------------------------------------------------------- credentials

    pub async fn list_credentials_shared(&self) -> AppResult<Arc<Vec<Credential>>> {
        let stamp = self.table_stamp(CREDENTIALS).await?;
        {
            let cache = self.cache.read().expect("repository cache poisoned");
            if let Some(cached) = &cache.credentials
                && cached.stamp == stamp
            {
                return Ok(Arc::clone(&cached.values));
            }
        }
        let sql = format!("SELECT Id, Name, Hash, Json FROM {CREDENTIALS} ORDER BY Name");
        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(storage_error)?;
        let mut credentials = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row.try_get("Id").map_err(storage_error)?;
            let json: String = row.try_get("Json").map_err(storage_error)?;
            let (credential, cleared) = unseal_credential_lenient(&json, self.codec.as_ref())
                .map_err(|error| {
                    AppError::storage(format!("cannot read credential {id}: {error}"))
                })?;
            if cleared > 0 {
                tracing::warn!(
                    credential_id = %id,
                    cleared,
                    "credential has secrets encrypted with another key; run catalog repair"
                );
            }
            credentials.push(credential);
        }
        let values = Arc::new(credentials);
        let mut cache = self.cache.write().expect("repository cache poisoned");
        cache.credentials = Some(CachedCredentials {
            stamp,
            values: Arc::clone(&values),
        });
        Ok(values)
    }

    pub async fn list_credentials(&self) -> AppResult<Vec<Credential>> {
        Ok((*self.list_credentials_shared().await?).clone())
    }

    pub async fn get_credential(&self, id: &str) -> AppResult<Option<Credential>> {
        let sql = format!(
            "SELECT Id, Name, Hash, Json FROM {CREDENTIALS} WHERE Id = {}",
            self.dialect.placeholder(1)
        );
        let row = sqlx::query(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage_error)?;
        row.map(|row| {
            let json: String = row.try_get("Json").map_err(storage_error)?;
            unseal_credential_lenient(&json, self.codec.as_ref()).map(|(value, _)| value)
        })
        .transpose()
    }

    pub async fn find_credential_by_name(&self, name: &str) -> AppResult<Option<Credential>> {
        let sql = format!(
            "SELECT Id, Name, Hash, Json FROM {CREDENTIALS} WHERE Name = {} COLLATE NOCASE",
            self.dialect.placeholder(1)
        );
        let row = sqlx::query(&sql)
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage_error)?;
        row.map(|row| {
            let json: String = row.try_get("Json").map_err(storage_error)?;
            unseal_credential_lenient(&json, self.codec.as_ref()).map(|(value, _)| value)
        })
        .transpose()
    }

    pub async fn find_credential_by_hash(&self, hash: &str) -> AppResult<Option<Credential>> {
        let sql = format!(
            "SELECT Id, Name, Hash, Json FROM {CREDENTIALS} WHERE Hash = {}",
            self.dialect.placeholder(1)
        );
        let row = sqlx::query(&sql)
            .bind(hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage_error)?;
        row.map(|row| {
            let json: String = row.try_get("Json").map_err(storage_error)?;
            unseal_credential_lenient(&json, self.codec.as_ref()).map(|(value, _)| value)
        })
        .transpose()
    }

    pub async fn upsert_credential(&self, mut credential: Credential) -> AppResult<Credential> {
        if let Some(existing) = self.get_credential(&credential.id).await?
            && !existing.is_editable
        {
            return Err(AppError::permission_denied(format!(
                "credential '{}' is read-only",
                existing.name
            )));
        }
        credential.normalize();
        credential.rehash();
        let issues = credential.validate();
        if !issues.is_empty() {
            return Err(AppError::validation(issues.join("; ")));
        }
        let json = seal_credential(&credential, self.codec.as_ref())?;
        if json.len() > MAX_JSON_BYTES {
            return Err(AppError::validation("credential JSON is too large"));
        }
        let now = now_ms();
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        let sql = self.dialect.credentials_upsert_sql();
        let mut query = sqlx::query(&sql);
        query = query
            .bind(&credential.id)
            .bind(&credential.name)
            .bind(&credential.hash)
            .bind(&json)
            .bind(now);
        query.execute(&mut *transaction).await.map_err(|error| {
            if is_unique_violation(&error) {
                AppError::conflict(format!(
                    "a credential named '{}' already exists",
                    credential.name
                ))
            } else {
                storage_error(error)
            }
        })?;
        bump_table(&mut transaction, CREDENTIALS, 1, now).await?;
        transaction.commit().await.map_err(storage_error)?;
        self.invalidate_caches();
        Ok(credential)
    }

    pub async fn upsert_credentials(
        &self,
        credentials: Vec<Credential>,
    ) -> AppResult<Vec<Credential>> {
        let mut prepared = Vec::with_capacity(credentials.len());
        for mut credential in credentials {
            if let Some(existing) = self.get_credential(&credential.id).await?
                && !existing.is_editable
            {
                return Err(AppError::permission_denied(format!(
                    "credential '{}' is read-only",
                    existing.name
                )));
            }
            credential.normalize();
            credential.rehash();
            let issues = credential.validate();
            if !issues.is_empty() {
                return Err(AppError::validation(issues.join("; ")));
            }
            let json = seal_credential(&credential, self.codec.as_ref())?;
            prepared.push((credential, json));
        }
        if prepared.is_empty() {
            return Ok(Vec::new());
        }
        let now = now_ms();
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        for (credential, json) in &prepared {
            let sql = self.dialect.credentials_upsert_sql();
            let mut query = sqlx::query(&sql);
            query = query
                .bind(&credential.id)
                .bind(&credential.name)
                .bind(&credential.hash)
                .bind(json)
                .bind(now);
            query.execute(&mut *transaction).await.map_err(|error| {
                if is_unique_violation(&error) {
                    AppError::conflict(format!(
                        "a credential named '{}' already exists",
                        credential.name
                    ))
                } else {
                    storage_error(error)
                }
            })?;
        }
        bump_table(&mut transaction, CREDENTIALS, prepared.len() as i64, now).await?;
        transaction.commit().await.map_err(storage_error)?;
        self.invalidate_caches();
        Ok(prepared
            .into_iter()
            .map(|(credential, _)| credential)
            .collect())
    }

    pub async fn delete_credential(&self, id: &str) -> AppResult<bool> {
        let Some(existing) = self.get_credential(id).await? else {
            return Ok(false);
        };
        if !existing.is_editable {
            return Err(AppError::permission_denied(format!(
                "credential '{}' is read-only",
                existing.name
            )));
        }
        let name = existing.name.clone();
        let now = now_ms();
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        // Detach servers inheriting the credential so they remain usable.
        for mut server in self.list_servers().await? {
            let inherits = server
                .credentials()
                .and_then(|credentials| credentials.inherited_credential_name.as_deref())
                .is_some_and(|inherited| inherited.eq_ignore_ascii_case(&name));
            if inherits {
                if let Some(credentials) = server.credentials_mut() {
                    credentials.inherited_credential_name = None;
                }
                server.normalize();
                let json = seal_server(&server, self.codec.as_ref())?;
                self.write_server(&mut transaction, &server, &json, now)
                    .await?;
            }
        }
        let result = sqlx::query(&format!(
            "DELETE FROM {CREDENTIALS} WHERE Id = {}",
            self.dialect.placeholder(1)
        ))
        .bind(id)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if result.rows_affected() > 0 {
            bump_table(&mut transaction, CREDENTIALS, -1, now).await?;
        }
        transaction.commit().await.map_err(storage_error)?;
        self.invalidate_caches();
        Ok(result.rows_affected() > 0)
    }

    pub async fn credential_count(&self) -> AppResult<i64> {
        sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {CREDENTIALS}"))
            .fetch_one(&self.pool)
            .await
            .map_err(storage_error)
    }

    // ----------------------------------------------------------------- configs

    pub async fn get_config(&self, key: &str) -> AppResult<Option<String>> {
        let sql = format!(
            "SELECT Value FROM {CONFIGS} WHERE Key = {}",
            self.dialect.placeholder(1)
        );
        let row: Option<(String,)> = sqlx::query_as(&sql)
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage_error)?;
        Ok(row.map(|(value,)| value))
    }

    pub async fn set_config(&self, key: &str, value: &str) -> AppResult<()> {
        if value.len() > MAX_CONFIG_BYTES {
            return Err(AppError::validation("configuration value is too large"));
        }
        let now = now_ms();
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        let sql = self.dialect.configs_upsert_sql();
        let mut query = sqlx::query(&sql);
        query = query.bind(key).bind(value).bind(now);
        query
            .execute(&mut *transaction)
            .await
            .map_err(storage_error)?;
        bump_table(&mut transaction, CONFIGS, 1, now).await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(())
    }

    pub async fn delete_config(&self, key: &str) -> AppResult<bool> {
        let now = now_ms();
        let mut transaction = self.pool.begin().await.map_err(storage_error)?;
        let result = sqlx::query(&format!(
            "DELETE FROM {CONFIGS} WHERE Key = {}",
            self.dialect.placeholder(1)
        ))
        .bind(key)
        .execute(&mut *transaction)
        .await
        .map_err(storage_error)?;
        if result.rows_affected() > 0 {
            bump_table(&mut transaction, CONFIGS, -1, now).await?;
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn encryption_verified(&self) -> AppResult<bool> {
        Ok(self.get_config(ENCRYPTION_TEST_KEY).await?.is_some())
    }

    async fn table_stamp(&self, table: &str) -> AppResult<i64> {
        let sql = format!(
            "SELECT UpdatedAt FROM {TABLE_META} WHERE TableName = {}",
            self.dialect.placeholder(1)
        );
        let row: Option<(i64,)> = sqlx::query_as(&sql)
            .bind(table)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage_error)?;
        Ok(row.map(|(stamp,)| stamp).unwrap_or(0))
    }

    async fn write_server(
        &self,
        transaction: &mut Transaction<'_, Sqlite>,
        server: &Server,
        json: &str,
        now: i64,
    ) -> AppResult<()> {
        let sql = self.dialect.servers_upsert_sql();
        let mut query = sqlx::query(&sql);
        query = query
            .bind(&server.base().id)
            .bind(server.protocol().id())
            .bind(i64::from(server.base().class_version))
            .bind(json)
            .bind(now);
        query
            .execute(&mut **transaction)
            .await
            .map_err(storage_error)?;
        Ok(())
    }

    fn decode_server_rows(&self, rows: Vec<sqlx::sqlite::SqliteRow>) -> AppResult<Vec<Server>> {
        rows.iter().map(|row| self.decode_server_row(row)).collect()
    }

    fn decode_server_row(&self, row: &sqlx::sqlite::SqliteRow) -> AppResult<Server> {
        let id: String = row.try_get("Id").map_err(storage_error)?;
        let protocol: String = row.try_get("Protocol").map_err(storage_error)?;
        let class_version: i64 = row.try_get("ClassVersion").map_err(storage_error)?;
        let json: String = row.try_get("Json").map_err(storage_error)?;
        unseal_server_lenient(
            &protocol,
            class_version.max(0) as u32,
            &json,
            self.codec.as_ref(),
        )
        .map(|(server, _)| server)
        .map_err(|error| AppError::storage(format!("cannot read server {id}: {error}")))
    }
}

async fn bump_table(
    transaction: &mut Transaction<'_, Sqlite>,
    table: &str,
    delta: i64,
    now: i64,
) -> AppResult<()> {
    let sql = format!(
        "INSERT INTO {TABLE_META} (TableName, UpdatedAt, RowCount) VALUES (?1, ?2, ?3)
         ON CONFLICT(TableName) DO UPDATE SET
             UpdatedAt = excluded.UpdatedAt,
             RowCount = MAX(0, TableMeta.RowCount + excluded.RowCount)"
    );
    sqlx::query(&sql)
        .bind(table)
        .bind(now)
        .bind(delta)
        .execute(&mut **transaction)
        .await
        .map_err(storage_error)?;
    Ok(())
}

async fn insert_folder(
    transaction: &mut Transaction<'_, Sqlite>,
    folder: &Folder,
    now: i64,
) -> AppResult<()> {
    let sql =
        format!("INSERT INTO {FOLDERS} (Id, Path, ColorHex, UpdatedAt) VALUES (?1, ?2, ?3, ?4)");
    sqlx::query(&sql)
        .bind(&folder.id)
        .bind(folder_path_json(&folder.path))
        .bind(&folder.color_hex)
        .bind(now)
        .execute(&mut **transaction)
        .await
        .map_err(|error| {
            if is_unique_violation(&error) {
                AppError::conflict("a folder with that path already exists")
            } else {
                storage_error(error)
            }
        })?;
    Ok(())
}

fn normalize_folder_path(path: Vec<String>) -> AppResult<Vec<String>> {
    let path = normalize_tree_nodes(path);
    if path.is_empty() {
        return Err(AppError::validation("folder path is empty"));
    }
    let issues = validate_tree_path(&path);
    if !issues.is_empty() {
        return Err(AppError::validation(issues.join("; ")));
    }
    Ok(path)
}

fn validate_folder_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::validation("folder name is required"));
    }
    if name.contains(['/', '\\']) {
        return Err(AppError::validation(
            "folder name cannot contain a path separator",
        ));
    }
    if name.chars().any(char::is_control) {
        return Err(AppError::validation(
            "folder name contains control characters",
        ));
    }
    if name.chars().count() > 255 {
        return Err(AppError::validation("folder name is too long"));
    }
    Ok(name.to_string())
}

fn folder_path_json(path: &[String]) -> String {
    serde_json::to_string(path).unwrap_or_else(|_| "[]".to_string())
}

fn path_starts_with(path: &[String], prefix: &[String]) -> bool {
    path.len() >= prefix.len()
        && path
            .iter()
            .zip(prefix.iter())
            .all(|(left, right)| left == right)
}

fn ensure_valid_server(server: &Server) -> AppResult<()> {
    let issues = server.validate();
    if issues.is_empty() {
        Ok(())
    } else {
        Err(AppError::validation(
            issues
                .iter()
                .map(|issue| format!("{}: {}", issue.field, issue.message))
                .collect::<Vec<_>>()
                .join("; "),
        ))
    }
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    error
        .to_string()
        .to_ascii_lowercase()
        .contains("unique constraint")
}

fn storage_error(error: sqlx::Error) -> AppError {
    AppError::storage(format!("database error: {error}"))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::CURRENT_SCHEMA_VERSION;
    use hr_crypto::{InstallationSalt, KeyStoreKind, Vault};
    use hr_domain::{AppArgument, AppArgumentType, Credential};
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn repository(dir: &TempDir) -> (Repository, Arc<Vault>, crate::Database) {
        let database = crate::Database::open_sqlite(dir.path().join("hremote.db"))
            .await
            .unwrap();
        let salt = InstallationSalt::load_or_create(dir.path().join("salt")).unwrap();
        let vault = Arc::new(Vault::from_parts([3u8; 32], &salt, KeyStoreKind::Memory));
        let codec = Arc::new(crate::codec::VaultCodec::new(Arc::clone(&vault)));
        let repository = Repository::open(database.pool().clone(), codec)
            .await
            .unwrap();
        (repository, vault, database)
    }

    #[tokio::test]
    async fn sentinel_is_written_and_verified() {
        let dir = TempDir::new().unwrap();
        let (repository, _, database) = repository(&dir).await;
        assert!(repository.encryption_verified().await.unwrap());
        assert_eq!(
            database.schema_version().await.unwrap(),
            CURRENT_SCHEMA_VERSION
        );
        database.close().await;
    }

    #[tokio::test]
    async fn wrong_vault_is_rejected_explicitly() {
        let dir = TempDir::new().unwrap();
        let (repository, _, database) = repository(&dir).await;
        drop(repository);
        let salt = InstallationSalt::load_or_create(dir.path().join("salt")).unwrap();
        let wrong = Arc::new(Vault::from_parts([4u8; 32], &salt, KeyStoreKind::Memory));
        let error = Repository::open(
            database.pool().clone(),
            Arc::new(crate::codec::VaultCodec::new(wrong)),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, AppError::EncryptionKey(_)), "{error}");
        database.close().await;
    }

    #[tokio::test]
    async fn repair_reencrypts_readable_rows_and_clears_foreign_secrets() {
        let dir = TempDir::new().unwrap();
        let database = crate::Database::open_sqlite(dir.path().join("hremote.db"))
            .await
            .unwrap();
        let salt = InstallationSalt::load_or_create(dir.path().join("salt")).unwrap();
        let first = Arc::new(Vault::from_parts([5u8; 32], &salt, KeyStoreKind::Memory));
        let first_codec = Arc::new(crate::codec::VaultCodec::new(Arc::clone(&first)));
        let repository = Repository::open(database.pool().clone(), first_codec)
            .await
            .unwrap();
        let mut server = Server::new_ssh("Host", "host.example", "root");
        if let Some(credentials) = server.credentials_mut() {
            credentials.password = "foreign-secret".into();
        }
        repository.upsert_server(server).await.unwrap();
        drop(repository);

        let second = Arc::new(Vault::from_parts([6u8; 32], &salt, KeyStoreKind::Memory));
        let second_codec: Arc<dyn SecretCodec> =
            Arc::new(crate::codec::VaultCodec::new(Arc::clone(&second)));
        let report = Repository::repair_secrets_with_pool(
            database.pool().clone(),
            Arc::clone(&second_codec),
        )
        .await
        .unwrap();
        assert_eq!(report.servers, 1);
        assert_eq!(report.secrets_cleared, 1);

        let repaired = Repository::open(database.pool().clone(), second_codec)
            .await
            .unwrap();
        let loaded = repaired.list_servers().await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].base().display_name, "Host");
        assert!(loaded[0].credentials().unwrap().password.is_empty());
        database.close().await;
    }

    #[tokio::test]
    async fn missing_sentinel_with_existing_rows_requires_repair() {
        let dir = TempDir::new().unwrap();
        let (repository, _, database) = repository(&dir).await;
        repository
            .upsert_server(Server::new_ssh("Host", "host.example", "root"))
            .await
            .unwrap();
        sqlx::query(&format!(
            "DELETE FROM {CONFIGS} WHERE Key = '{ENCRYPTION_TEST_KEY}'"
        ))
        .execute(database.pool())
        .await
        .unwrap();
        drop(repository);
        let salt = InstallationSalt::load_or_create(dir.path().join("salt")).unwrap();
        let vault = Arc::new(Vault::from_parts([3u8; 32], &salt, KeyStoreKind::Memory));
        let error = Repository::open(
            database.pool().clone(),
            Arc::new(crate::codec::VaultCodec::new(vault)),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, AppError::EncryptionKey(_)), "{error}");
        database.close().await;
    }

    #[tokio::test]
    async fn every_protocol_round_trips_through_storage() {
        let dir = TempDir::new().unwrap();
        let (repository, _, database) = repository(&dir).await;
        let servers = hr_domain::sample_servers();
        let mut expected = servers.clone();
        expected.sort_by_key(|server| server.base().id.clone());
        repository.upsert_servers(servers).await.unwrap();
        let mut loaded = repository.list_servers().await.unwrap();
        loaded.sort_by_key(|server| server.base().id.clone());
        assert_eq!(loaded, expected);

        let raw: String = sqlx::query_scalar("SELECT Json FROM Servers LIMIT 1")
            .fetch_one(database.pool())
            .await
            .unwrap();
        assert!(!raw.contains("ssh-secret"));
        database.close().await;
    }

    #[tokio::test]
    async fn crud_and_pagination_work() {
        let dir = TempDir::new().unwrap();
        let (repository, _, database) = repository(&dir).await;
        let server = repository
            .upsert_server(Server::new_ssh("one", "one.example", "root"))
            .await
            .unwrap();
        assert_eq!(
            repository.get_server(&server.base().id).await.unwrap(),
            Some(server.clone())
        );
        assert_eq!(repository.server_count().await.unwrap(), 1);
        assert_eq!(repository.list_servers_page(0, 10).await.unwrap().len(), 1);
        assert!(repository.delete_server(&server.base().id).await.unwrap());
        assert!(!repository.delete_server(&server.base().id).await.unwrap());
        assert_eq!(repository.server_count().await.unwrap(), 0);
        database.close().await;
    }

    #[tokio::test]
    async fn invalid_servers_are_rejected_before_writing() {
        let dir = TempDir::new().unwrap();
        let (repository, _, database) = repository(&dir).await;
        let mut bad = Server::new_ssh("bad", "", "root");
        bad.address_mut().unwrap().port = 0;
        let error = repository.upsert_server(bad).await.unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
        assert_eq!(repository.server_count().await.unwrap(), 0);
        database.close().await;
    }

    #[tokio::test]
    async fn caches_refresh_when_the_table_stamp_changes() {
        let dir = TempDir::new().unwrap();
        let (repository, _, database) = repository(&dir).await;
        let first = repository.list_servers_shared().await.unwrap();
        assert!(first.is_empty());
        repository
            .upsert_server(Server::new_ssh("one", "one", "root"))
            .await
            .unwrap();
        let after_write = repository.list_servers_shared().await.unwrap();
        assert_eq!(after_write.len(), 1);

        // External writer changes data and bumps the stamp.
        let now = now_ms() + 1;
        sqlx::query("UPDATE TableMeta SET UpdatedAt = ? WHERE TableName = 'Servers'")
            .bind(now)
            .execute(database.pool())
            .await
            .unwrap();
        let after_external = repository.list_servers_shared().await.unwrap();
        assert_eq!(after_external.len(), 1);
        database.close().await;
    }

    #[tokio::test]
    async fn credential_crud_detects_duplicate_names_and_hashes() {
        let dir = TempDir::new().unwrap();
        let (repository, _, database) = repository(&dir).await;
        let mut credential = Credential::new("prod", "root");
        credential.password = "secret".into();
        let stored = repository.upsert_credential(credential).await.unwrap();
        assert_eq!(repository.credential_count().await.unwrap(), 1);
        assert!(
            repository
                .find_credential_by_name("PROD")
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            repository
                .find_credential_by_hash(&stored.hash)
                .await
                .unwrap()
                .is_some()
        );

        let duplicate = Credential::new("prod", "other");
        let error = repository.upsert_credential(duplicate).await.unwrap_err();
        assert!(matches!(error, AppError::Conflict(_)), "{error}");

        assert!(repository.delete_credential(&stored.id).await.unwrap());
        assert_eq!(repository.credential_count().await.unwrap(), 0);
        database.close().await;
    }

    #[tokio::test]
    async fn folder_crud_updates_server_paths_and_prevents_cycles() {
        let dir = TempDir::new().unwrap();
        let (repository, _, database) = repository(&dir).await;
        let prod = repository.create_folder(vec!["Prod".into()]).await.unwrap();
        let eu = repository
            .ensure_folder_path(vec!["Prod".into(), "EU".into()])
            .await
            .unwrap();
        assert_eq!(repository.folder_count().await.unwrap(), 2);

        let mut server = Server::new_ssh("web", "10.0.0.1", "root");
        server.base_mut().tree_nodes = vec!["Prod".into(), "EU".into()];
        repository.upsert_server(server).await.unwrap();

        repository
            .rename_folder(&prod.id, "Production")
            .await
            .unwrap();
        let folders = repository.list_folders().await.unwrap();
        assert!(
            folders
                .iter()
                .any(|folder| folder.path == ["Production".to_string()])
        );
        let loaded = repository.list_servers().await.unwrap();
        assert_eq!(
            loaded[0].base().tree_nodes,
            vec!["Production".to_string(), "EU".to_string()]
        );

        let production = folders
            .iter()
            .find(|folder| folder.path == ["Production".to_string()])
            .unwrap();
        assert!(matches!(
            repository
                .move_folder(&production.id, vec!["Production".into(), "EU".into()])
                .await,
            Err(AppError::Conflict(_))
        ));

        repository.move_folder(&eu.id, vec![]).await.unwrap();
        let folders = repository.list_folders().await.unwrap();
        assert!(
            folders
                .iter()
                .any(|folder| folder.path == ["EU".to_string()])
        );

        let (deleted, moved) = repository
            .delete_folder(vec!["Production".into()])
            .await
            .unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(moved, 0);
        let loaded = repository.list_servers().await.unwrap();
        assert_eq!(loaded[0].base().tree_nodes, vec!["EU".to_string()]);

        let colored = repository
            .set_folder_color(&eu.id, "#ff8800")
            .await
            .unwrap();
        assert_eq!(colored.color_hex, "#FF8800");
        assert_eq!(
            repository
                .list_folders()
                .await
                .unwrap()
                .into_iter()
                .find(|folder| folder.id == eu.id)
                .unwrap()
                .color_hex,
            "#FF8800"
        );
        assert!(
            repository
                .set_folder_color(&eu.id, "not-a-color")
                .await
                .is_err()
        );

        let copy = repository.duplicate_folder(&eu.id).await.unwrap();
        assert_eq!(copy.path, vec!["EU (copy)".to_string()]);
        assert_eq!(copy.color_hex, "#FF8800");
        database.close().await;
    }

    #[tokio::test]
    async fn cached_snapshots_refresh_after_a_write() {
        let dir = TempDir::new().unwrap();
        let (repository, _, database) = repository(&dir).await;
        let before = repository.list_servers_shared().await.unwrap();
        assert!(before.is_empty());
        repository
            .upsert_server(Server::new_ssh("one", "one", "root"))
            .await
            .unwrap();
        let after = repository.list_servers_shared().await.unwrap();
        assert_eq!(after.len(), 1);
        database.close().await;
    }

    #[tokio::test]
    async fn deleting_a_credential_detaches_inheriting_servers() {
        let dir = TempDir::new().unwrap();
        let (repository, _, database) = repository(&dir).await;
        let credential = repository
            .upsert_credential(Credential::new("shared", "root"))
            .await
            .unwrap();
        let mut server = Server::new_ssh("Box", "host", "");
        if let Some(credentials) = server.credentials_mut() {
            credentials.inherited_credential_name = Some("shared".into());
        }
        repository.upsert_server(server).await.unwrap();

        assert!(repository.delete_credential(&credential.id).await.unwrap());
        let loaded = repository.list_servers().await.unwrap();
        assert_eq!(
            loaded[0]
                .credentials()
                .and_then(|credentials| credentials.inherited_credential_name.clone()),
            None
        );
        assert!(
            repository
                .get_credential(&credential.id)
                .await
                .unwrap()
                .is_none()
        );
        database.close().await;
    }

    #[tokio::test]
    async fn read_only_credentials_cannot_be_edited_or_deleted() {
        let dir = TempDir::new().unwrap();
        let (repository, _, database) = repository(&dir).await;
        let mut credential = Credential::new("locked", "root");
        credential.password = "secret".into();
        credential.is_editable = false;
        let stored = repository.upsert_credential(credential).await.unwrap();
        let mut changed = stored.clone();
        changed.username = "other".into();
        assert!(matches!(
            repository.upsert_credential(changed).await,
            Err(AppError::PermissionDenied(_))
        ));
        assert!(matches!(
            repository.delete_credential(&stored.id).await,
            Err(AppError::PermissionDenied(_))
        ));
        database.close().await;
    }

    #[tokio::test]
    async fn configs_round_trip_and_hide_sentinel() {
        let dir = TempDir::new().unwrap();
        let (repository, _, database) = repository(&dir).await;
        repository.set_config("theme", "Dark").await.unwrap();
        assert_eq!(
            repository.get_config("theme").await.unwrap().as_deref(),
            Some("Dark")
        );
        assert!(
            repository
                .get_config(ENCRYPTION_TEST_KEY)
                .await
                .unwrap()
                .is_some()
        );
        assert!(repository.delete_config("theme").await.unwrap());
        database.close().await;
    }

    #[tokio::test]
    async fn bulk_write_is_atomic_on_validation_failure() {
        let dir = TempDir::new().unwrap();
        let (repository, _, database) = repository(&dir).await;
        let mut bad = Server::new_ssh("bad", "", "root");
        bad.address_mut().unwrap().port = 0;
        let error = repository
            .upsert_servers(vec![Server::new_ssh("good", "good", "root"), bad])
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
        assert_eq!(repository.server_count().await.unwrap(), 0);
        database.close().await;
    }

    #[tokio::test]
    async fn argument_secrets_never_appear_in_the_database() {
        let dir = TempDir::new().unwrap();
        let (repository, _, database) = repository(&dir).await;
        let mut server = Server::new_local_app("App", "C:\\app.exe");
        if let Server::LocalApp(inner) = &mut server {
            inner.argument_list = vec![AppArgument {
                argument_type: AppArgumentType::Secret,
                name: "token".into(),
                key: "--token".into(),
                value: "super-secret".into(),
                ..AppArgument::default()
            }];
        }
        repository.upsert_server(server).await.unwrap();
        let raw: String = sqlx::query_scalar("SELECT Json FROM Servers")
            .fetch_one(database.pool())
            .await
            .unwrap();
        assert!(!raw.contains("super-secret"));
        database.close().await;
    }
}
