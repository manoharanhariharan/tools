//! Additional read-only data sources (MySQL/PostgreSQL, plus extra SQLite DBs).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use hr_core::{AppError, AppResult, new_id};
use hr_domain::{Credential, LOCAL_SOURCE_ID, Server, legacy_value_to_server};
use serde::{Deserialize, Serialize};
use sqlx::{MySqlPool, PgPool, Row, SqlitePool};
use tokio::sync::Mutex;

use crate::codec::{SecretCodec, unseal_credential, unseal_server};

const SOURCE_PURPOSE: &str = "datasource-password";
const SOURCES_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    Sqlite,
    MySql,
    Postgres,
}

impl SourceKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Sqlite => "sqlite",
            Self::MySql => "mysql",
            Self::Postgres => "postgres",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "sqlite" => Some(Self::Sqlite),
            "mysql" => Some(Self::MySql),
            "postgres" | "postgresql" => Some(Self::Postgres),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DataSourceConfig {
    pub id: String,
    pub name: String,
    pub kind: SourceKind,
    pub path: Option<String>,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    /// Vault-encrypted password; never exposed to the UI.
    pub password: String,
    pub read_only: bool,
    pub enabled: bool,
}

impl Default for DataSourceConfig {
    fn default() -> Self {
        Self {
            id: new_id(),
            name: "New source".to_string(),
            kind: SourceKind::Sqlite,
            path: None,
            host: "localhost".to_string(),
            port: 0,
            database: String::new(),
            username: String::new(),
            password: String::new(),
            read_only: true,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct SourcesFile {
    schema_version: u32,
    sources: Vec<DataSourceConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub state: String,
    pub writable: bool,
    pub server_count: usize,
    pub credential_count: usize,
    pub error: Option<String>,
    pub last_checked: i64,
}

struct CachedSource {
    config: DataSourceConfig,
    servers: Vec<Server>,
    credentials: Vec<Credential>,
    status: SourceStatus,
    backend: Option<Backend>,
}

#[derive(Clone)]
enum Backend {
    Sqlite(SqlitePool),
    MySql(MySqlPool),
    Postgres(PgPool),
}

pub struct SourceManager {
    path: PathBuf,
    codec: Arc<dyn SecretCodec>,
    sources: Mutex<HashMap<String, CachedSource>>,
}

impl SourceManager {
    pub async fn load(path: PathBuf, codec: Arc<dyn SecretCodec>) -> Self {
        let manager = Self {
            path,
            codec,
            sources: Mutex::new(HashMap::new()),
        };
        if let Ok(content) = tokio::fs::read_to_string(&manager.path).await {
            match serde_json::from_str::<SourcesFile>(&content) {
                Ok(file) => {
                    let mut sources = manager.sources.lock().await;
                    for config in file.sources {
                        let status = SourceStatus {
                            id: config.id.clone(),
                            name: config.name.clone(),
                            kind: config.kind.id().to_string(),
                            state: "notConnectedYet".to_string(),
                            writable: false,
                            server_count: 0,
                            credential_count: 0,
                            error: None,
                            last_checked: 0,
                        };
                        sources.insert(
                            config.id.clone(),
                            CachedSource {
                                config,
                                servers: Vec::new(),
                                credentials: Vec::new(),
                                status,
                                backend: None,
                            },
                        );
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        %error,
                        path = %manager.path.display(),
                        "data sources file is unreadable; no additional sources loaded"
                    );
                }
            }
        }
        manager
    }

    pub async fn statuses(&self) -> Vec<SourceStatus> {
        let sources = self.sources.lock().await;
        let mut statuses: Vec<SourceStatus> = sources
            .values()
            .map(|source| source.status.clone())
            .collect();
        statuses.sort_by(|left, right| left.name.cmp(&right.name));
        statuses
    }

    pub async fn configs(&self) -> Vec<DataSourceConfig> {
        let sources = self.sources.lock().await;
        let mut configs: Vec<DataSourceConfig> = sources
            .values()
            .map(|source| {
                let mut config = source.config.clone();
                config.password.clear();
                config
            })
            .collect();
        configs.sort_by(|left, right| left.name.cmp(&right.name));
        configs
    }

    pub async fn servers(&self) -> Vec<Server> {
        let sources = self.sources.lock().await;
        sources
            .values()
            .flat_map(|source| source.servers.clone())
            .collect()
    }

    pub async fn credentials(&self) -> Vec<Credential> {
        let sources = self.sources.lock().await;
        sources
            .values()
            .flat_map(|source| source.credentials.clone())
            .collect()
    }

    pub async fn source_for_server(&self, server_id: &str) -> Option<String> {
        let sources = self.sources.lock().await;
        sources.iter().find_map(|(id, source)| {
            source
                .servers
                .iter()
                .any(|server| server.base().id == server_id)
                .then(|| id.clone())
        })
    }

    pub async fn save(
        &self,
        mut config: DataSourceConfig,
        password: Option<String>,
    ) -> AppResult<DataSourceConfig> {
        if config.id.trim().is_empty() || config.id == LOCAL_SOURCE_ID {
            config.id = new_id();
        }
        if config.name.trim().is_empty() {
            return Err(AppError::validation("the source name is required"));
        }
        if config.kind == SourceKind::Sqlite {
            let path = config
                .path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
                .ok_or_else(|| AppError::validation("the SQLite source path is required"))?;
            if !Path::new(path).is_file() {
                return Err(AppError::validation(
                    "the SQLite source file does not exist",
                ));
            }
        } else if config.database.trim().is_empty() {
            return Err(AppError::validation("the database name is required"));
        }
        let existing = {
            let sources = self.sources.lock().await;
            sources.get(&config.id).map(|source| source.config.clone())
        };
        config.password = match (password, existing.as_ref()) {
            (Some(password), _) if !password.is_empty() => {
                self.codec.encrypt(SOURCE_PURPOSE, &password)?
            }
            (_, Some(existing)) => existing.password.clone(),
            _ => String::new(),
        };
        config.read_only = true;
        let mut sources = self.sources.lock().await;
        sources.insert(
            config.id.clone(),
            CachedSource {
                status: SourceStatus {
                    id: config.id.clone(),
                    name: config.name.clone(),
                    kind: config.kind.id().to_string(),
                    state: "notConnectedYet".to_string(),
                    writable: false,
                    server_count: 0,
                    credential_count: 0,
                    error: None,
                    last_checked: 0,
                },
                config: config.clone(),
                servers: Vec::new(),
                credentials: Vec::new(),
                backend: None,
            },
        );
        drop(sources);
        self.persist().await?;
        let mut config = config;
        config.password.clear();
        Ok(config)
    }

    pub async fn delete(&self, id: &str) -> AppResult<bool> {
        let mut sources = self.sources.lock().await;
        let removed = sources.remove(id).is_some();
        drop(sources);
        if removed {
            self.persist().await?;
        }
        Ok(removed)
    }

    pub async fn refresh(&self, id: &str) -> AppResult<SourceStatus> {
        let (config, decrypted) = {
            let sources = self.sources.lock().await;
            let source = sources
                .get(id)
                .ok_or_else(|| AppError::not_found("data source"))?;
            let decrypted = if source.config.password.is_empty() {
                Ok(String::new())
            } else {
                self.codec.decrypt(SOURCE_PURPOSE, &source.config.password)
            };
            (source.config.clone(), decrypted)
        };
        let password = match decrypted {
            Ok(password) => password,
            Err(error) => {
                // A key mismatch is a distinct state; keep the cache untouched.
                let mut sources = self.sources.lock().await;
                let source = sources
                    .get_mut(id)
                    .ok_or_else(|| AppError::not_found("data source"))?;
                source.status.last_checked = now_seconds();
                source.status.state = "encryptKeyError".to_string();
                source.status.error = Some(format!("cannot decrypt the stored password: {error}"));
                return Ok(source.status.clone());
            }
        };
        let outcome = self.connect_and_load(&config, &password).await;
        let mut sources = self.sources.lock().await;
        let source = sources
            .get_mut(id)
            .ok_or_else(|| AppError::not_found("data source"))?;
        source.status.last_checked = now_seconds();
        match outcome {
            Ok((backend, servers, credentials)) => {
                source.backend = Some(backend);
                source.servers = servers;
                source.credentials = credentials;
                source.status.state = "ok".to_string();
                source.status.error = None;
                source.status.server_count = source.servers.len();
                source.status.credential_count = source.credentials.len();
            }
            Err(error) => {
                let message = error.to_string();
                source.status.state =
                    classify_source_error(&message, source.status.server_count > 0).to_string();
                source.status.error = Some(message);
                // Keep the previous cache when the source goes down.
            }
        }
        Ok(source.status.clone())
    }

    pub async fn refresh_all(&self) -> Vec<SourceStatus> {
        let ids: Vec<String> = {
            let sources = self.sources.lock().await;
            sources
                .values()
                .filter(|source| source.config.enabled)
                .map(|source| source.config.id.clone())
                .collect()
        };
        for id in ids {
            let _ = self.refresh(&id).await;
        }
        self.statuses().await
    }

    pub async fn test(
        &self,
        config: &DataSourceConfig,
        password: Option<&str>,
    ) -> AppResult<SourceStatus> {
        let password = match password {
            Some(password) if !password.is_empty() => password.to_string(),
            _ => {
                let stored = {
                    let sources = self.sources.lock().await;
                    sources
                        .get(&config.id)
                        .map(|source| source.config.password.clone())
                        .unwrap_or_default()
                };
                if stored.is_empty() {
                    String::new()
                } else {
                    self.codec.decrypt(SOURCE_PURPOSE, &stored)?
                }
            }
        };
        let (_, servers, credentials) = self.connect_and_load(config, &password).await?;
        Ok(SourceStatus {
            id: config.id.clone(),
            name: config.name.clone(),
            kind: config.kind.id().to_string(),
            state: "ok".to_string(),
            writable: false,
            server_count: servers.len(),
            credential_count: credentials.len(),
            error: None,
            last_checked: now_seconds(),
        })
    }

    async fn persist(&self) -> AppResult<()> {
        let sources = self.sources.lock().await;
        let file = SourcesFile {
            schema_version: SOURCES_VERSION,
            sources: sources
                .values()
                .map(|source| source.config.clone())
                .collect(),
        };
        let content = serde_json::to_string_pretty(&file)?;
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        tokio::fs::write(&self.path, content)
            .await
            .map_err(|error| {
                AppError::permission_denied(format!(
                    "cannot save data sources to {}: {error}",
                    self.path.display()
                ))
            })
    }

    async fn connect_and_load(
        &self,
        config: &DataSourceConfig,
        password: &str,
    ) -> AppResult<(Backend, Vec<Server>, Vec<Credential>)> {
        match config.kind {
            SourceKind::Sqlite => {
                let path = PathBuf::from(
                    config
                        .path
                        .as_deref()
                        .ok_or_else(|| AppError::validation("SQLite path is required"))?,
                );
                let options = sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&path)
                    .read_only(true)
                    .busy_timeout(Duration::from_secs(5));
                let pool = sqlx::sqlite::SqlitePoolOptions::new()
                    .max_connections(1)
                    .acquire_timeout(Duration::from_secs(5))
                    .connect_with(options)
                    .await
                    .map_err(|error| {
                        AppError::storage(format!("cannot open the SQLite source: {error}"))
                    })?;
                let servers = self.sqlite_servers(&pool, &config.id).await?;
                let credentials = self.sqlite_credentials(&pool, &config.id).await?;
                Ok((Backend::Sqlite(pool), servers, credentials))
            }
            SourceKind::MySql => {
                let mut options = sqlx::mysql::MySqlConnectOptions::new()
                    .host(&config.host)
                    .port(config.port)
                    .database(&config.database)
                    .username(&config.username)
                    .password(password)
                    .ssl_mode(sqlx::mysql::MySqlSslMode::Disabled);
                if config.port == 0 {
                    options = options.port(3306);
                }
                let pool = sqlx::mysql::MySqlPoolOptions::new()
                    .max_connections(1)
                    .acquire_timeout(Duration::from_secs(5))
                    .connect_with(options)
                    .await
                    .map_err(|error| {
                        AppError::storage(format!("cannot connect to MySQL: {error}"))
                    })?;
                let servers = self.mysql_servers(&pool, &config.id).await?;
                let credentials = self.mysql_credentials(&pool, &config.id).await?;
                Ok((Backend::MySql(pool), servers, credentials))
            }
            SourceKind::Postgres => {
                let mut options = sqlx::postgres::PgConnectOptions::new()
                    .host(&config.host)
                    .port(config.port)
                    .database(&config.database)
                    .username(&config.username)
                    .password(password)
                    .ssl_mode(sqlx::postgres::PgSslMode::Prefer);
                if config.port == 0 {
                    options = options.port(5432);
                }
                let pool = sqlx::postgres::PgPoolOptions::new()
                    .max_connections(1)
                    .acquire_timeout(Duration::from_secs(5))
                    .connect_with(options)
                    .await
                    .map_err(|error| {
                        AppError::storage(format!("cannot connect to PostgreSQL: {error}"))
                    })?;
                let servers = self.postgres_servers(&pool, &config.id).await?;
                let credentials = self.postgres_credentials(&pool, &config.id).await?;
                Ok((Backend::Postgres(pool), servers, credentials))
            }
        }
    }

    async fn sqlite_servers(&self, pool: &SqlitePool, source_id: &str) -> AppResult<Vec<Server>> {
        let rows = sqlx::query("SELECT Protocol, ClassVersion, Json FROM Servers")
            .fetch_all(pool)
            .await
            .map_err(|error| AppError::storage(format!("cannot read source servers: {error}")))?;
        Ok(rows
            .iter()
            .filter_map(|row| {
                let protocol: String = row.try_get("Protocol").unwrap_or_default();
                let class_version: i64 = row.try_get("ClassVersion").unwrap_or(0);
                let json: String = row.try_get("Json").ok()?;
                parse_server(&protocol, class_version as u32, &json, self.codec.as_ref()).map(
                    |mut server| {
                        server.base_mut().source_id = source_id.to_string();
                        server
                    },
                )
            })
            .collect())
    }

    async fn sqlite_credentials(
        &self,
        pool: &SqlitePool,
        source_id: &str,
    ) -> AppResult<Vec<Credential>> {
        let rows = sqlx::query("SELECT Json FROM Credentials")
            .fetch_all(pool)
            .await
            .map_err(|error| {
                AppError::storage(format!("cannot read source credentials: {error}"))
            })?;
        Ok(rows
            .iter()
            .filter_map(|row| row.try_get::<String, _>("Json").ok())
            .filter_map(|json| {
                unseal_credential(&json, self.codec.as_ref())
                    .ok()
                    .or_else(|| serde_json::from_str(&json).ok())
            })
            .map(|mut credential| {
                credential.is_editable = false;
                credential.database_id = Some(source_id.to_string());
                credential
            })
            .collect())
    }

    async fn mysql_servers(&self, pool: &MySqlPool, source_id: &str) -> AppResult<Vec<Server>> {
        let rows = sqlx::query("SELECT Protocol, ClassVersion, Json FROM Servers")
            .fetch_all(pool)
            .await
            .map_err(|error| AppError::storage(format!("cannot read MySQL servers: {error}")))?;
        Ok(rows
            .iter()
            .filter_map(|row| {
                let protocol: String = row.try_get("Protocol").unwrap_or_default();
                let class_version: i64 = row.try_get("ClassVersion").unwrap_or(0);
                let json: String = row.try_get("Json").ok()?;
                parse_server(&protocol, class_version as u32, &json, self.codec.as_ref()).map(
                    |mut server| {
                        server.base_mut().source_id = source_id.to_string();
                        server
                    },
                )
            })
            .collect())
    }

    async fn mysql_credentials(
        &self,
        pool: &MySqlPool,
        source_id: &str,
    ) -> AppResult<Vec<Credential>> {
        let rows = sqlx::query("SELECT Json FROM Credentials")
            .fetch_all(pool)
            .await
            .map_err(|error| {
                AppError::storage(format!("cannot read MySQL credentials: {error}"))
            })?;
        Ok(rows
            .iter()
            .filter_map(|row| row.try_get::<String, _>("Json").ok())
            .filter_map(|json| serde_json::from_str::<Credential>(&json).ok())
            .map(|mut credential| {
                credential.is_editable = false;
                credential.database_id = Some(source_id.to_string());
                credential
            })
            .collect())
    }

    async fn postgres_servers(&self, pool: &PgPool, source_id: &str) -> AppResult<Vec<Server>> {
        let rows = sqlx::query("SELECT Protocol, ClassVersion, Json FROM Servers")
            .fetch_all(pool)
            .await
            .map_err(|error| {
                AppError::storage(format!("cannot read PostgreSQL servers: {error}"))
            })?;
        Ok(rows
            .iter()
            .filter_map(|row| {
                let protocol: String = row.try_get("Protocol").unwrap_or_default();
                let class_version: i64 = row.try_get("ClassVersion").unwrap_or(0);
                let json: String = row.try_get("Json").ok()?;
                parse_server(&protocol, class_version as u32, &json, self.codec.as_ref()).map(
                    |mut server| {
                        server.base_mut().source_id = source_id.to_string();
                        server
                    },
                )
            })
            .collect())
    }

    async fn postgres_credentials(
        &self,
        pool: &PgPool,
        source_id: &str,
    ) -> AppResult<Vec<Credential>> {
        let rows = sqlx::query("SELECT Json FROM Credentials")
            .fetch_all(pool)
            .await
            .map_err(|error| {
                AppError::storage(format!("cannot read PostgreSQL credentials: {error}"))
            })?;
        Ok(rows
            .iter()
            .filter_map(|row| row.try_get::<String, _>("Json").ok())
            .filter_map(|json| serde_json::from_str::<Credential>(&json).ok())
            .map(|mut credential| {
                credential.is_editable = false;
                credential.database_id = Some(source_id.to_string());
                credential
            })
            .collect())
    }

    /// Lightweight liveness probe used by the periodic status watcher
    /// (P8-07). The cached catalog is always retained on failure (P8-13).
    pub async fn check(&self, id: &str) -> AppResult<SourceStatus> {
        let backend = {
            let sources = self.sources.lock().await;
            sources
                .get(id)
                .ok_or_else(|| AppError::not_found("data source"))?
                .backend
                .clone()
        };
        let alive = match &backend {
            // Never connected (or already dropped after a failure): the
            // reconnect timer owns recovery, so leave the state untouched.
            None => None,
            Some(Backend::Sqlite(pool)) => Some(
                sqlx::query("SELECT 1 FROM Servers LIMIT 1")
                    .fetch_optional(pool)
                    .await
                    .is_ok(),
            ),
            Some(Backend::MySql(pool)) => {
                Some(sqlx::query("SELECT 1").fetch_optional(pool).await.is_ok())
            }
            Some(Backend::Postgres(pool)) => {
                Some(sqlx::query("SELECT 1").fetch_optional(pool).await.is_ok())
            }
        };
        let mut sources = self.sources.lock().await;
        let source = sources
            .get_mut(id)
            .ok_or_else(|| AppError::not_found("data source"))?;
        source.status.last_checked = now_seconds();
        match alive {
            Some(true) if source.status.state == "lostConnection" => {
                source.status.state = "ok".to_string();
                source.status.error = None;
            }
            Some(false) => {
                source.backend = None;
                source.status.state = "lostConnection".to_string();
                source.status.error =
                    Some("the data source stopped responding; cached data is shown".to_string());
            }
            _ => {}
        }
        Ok(source.status.clone())
    }

    /// Closes every pooled backend so shutdown is deterministic (P8).
    pub async fn shutdown(&self) {
        let mut sources = self.sources.lock().await;
        for source in sources.values_mut() {
            if let Some(backend) = source.backend.take() {
                match backend {
                    Backend::Sqlite(pool) => pool.close().await,
                    Backend::MySql(pool) => pool.close().await,
                    Backend::Postgres(pool) => pool.close().await,
                }
            }
        }
    }
}

fn parse_server(
    protocol: &str,
    class_version: u32,
    json: &str,
    codec: &dyn SecretCodec,
) -> Option<Server> {
    unseal_server(protocol, class_version, json, codec)
        .ok()
        .or_else(|| serde_json::from_str::<Server>(json).ok())
        .or_else(|| {
            serde_json::from_str::<serde_json::Value>(json)
                .ok()
                .and_then(|value| legacy_value_to_server(&value).ok())
        })
}

fn classify_source_error(message: &str, had_cache: bool) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("denied") || lower.contains("authentication") {
        "accessDenied"
    } else if lower.contains("encryption") || lower.contains("encrypted") {
        "encryptKeyError"
    } else if had_cache {
        "lostConnection"
    } else {
        "otherError"
    }
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Repository;
    use crate::codec::tests::test_vault;
    use hr_domain::Server;
    use tempfile::TempDir;

    #[tokio::test]
    async fn sqlite_source_loads_read_only_servers_and_keeps_the_cache_on_failure() {
        let dir = TempDir::new().unwrap();
        let source_path = dir.path().join("source.db");
        let database = crate::Database::open_sqlite(&source_path).await.unwrap();
        let vault = test_vault(dir.path());
        let codec = Arc::new(crate::VaultCodec::new(vault));
        let repository = Repository::open(database.pool().clone(), codec.clone())
            .await
            .unwrap();
        repository
            .upsert_server(Server::new_ssh("Imported", "host", "root"))
            .await
            .unwrap();
        database.close().await;

        let path = dir.path().join("hremote.dataSources.json");
        let manager = SourceManager::load(path.clone(), codec).await;
        let config = manager
            .save(
                DataSourceConfig {
                    name: "Secondary".into(),
                    kind: SourceKind::Sqlite,
                    path: Some(source_path.display().to_string()),
                    ..DataSourceConfig::default()
                },
                None,
            )
            .await
            .unwrap();
        manager.refresh(&config.id).await.unwrap();
        let status = manager.statuses().await;
        assert_eq!(status[0].state, "ok");
        assert_eq!(status[0].server_count, 1);
        assert_eq!(manager.servers().await[0].base().source_id, config.id);
        assert!(Path::new(&path).is_file());

        // Closing the pool then deleting the file must report a lost connection
        // while keeping the cached catalog (P8-06).
        manager.shutdown().await;
        std::fs::remove_file(&source_path).unwrap();
        manager.refresh(&config.id).await.unwrap();
        let status = manager.statuses().await;
        assert_eq!(status[0].state, "lostConnection");
        assert_eq!(manager.servers().await.len(), 1);
    }

    #[tokio::test]
    async fn missing_sqlite_file_is_rejected() {
        let dir = TempDir::new().unwrap();
        let manager = SourceManager::load(
            dir.path().join("sources.json"),
            Arc::new(crate::VaultCodec::new(test_vault(dir.path()))),
        )
        .await;
        let error = manager
            .save(
                DataSourceConfig {
                    name: "Missing".into(),
                    kind: SourceKind::Sqlite,
                    path: Some(dir.path().join("missing.db").display().to_string()),
                    ..DataSourceConfig::default()
                },
                None,
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn source_credentials_are_read_only_and_tagged_with_their_source() {
        let dir = TempDir::new().unwrap();
        let source_path = dir.path().join("source.db");
        let database = crate::Database::open_sqlite(&source_path).await.unwrap();
        let vault = test_vault(dir.path());
        let codec = Arc::new(crate::VaultCodec::new(vault));
        let repository = Repository::open(database.pool().clone(), codec.clone())
            .await
            .unwrap();
        repository
            .upsert_credential(hr_domain::Credential::new("shared", "root"))
            .await
            .unwrap();
        database.close().await;

        let manager = SourceManager::load(dir.path().join("sources.json"), codec).await;
        let config = manager
            .save(
                DataSourceConfig {
                    name: "Shared".into(),
                    kind: SourceKind::Sqlite,
                    path: Some(source_path.display().to_string()),
                    ..DataSourceConfig::default()
                },
                None,
            )
            .await
            .unwrap();
        manager.refresh(&config.id).await.unwrap();
        let credentials = manager.credentials().await;
        assert_eq!(credentials.len(), 1);
        assert!(!credentials[0].is_editable);
        assert_eq!(
            credentials[0].database_id.as_deref(),
            Some(config.id.as_str())
        );
        manager.shutdown().await;
    }

    #[tokio::test]
    async fn wrong_vault_key_reports_encrypt_key_error_without_losing_the_source() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sources.json");
        let manager = SourceManager::load(
            path.clone(),
            Arc::new(crate::VaultCodec::new(test_vault(dir.path()))),
        )
        .await;
        manager
            .save(
                DataSourceConfig {
                    name: "Encrypted".into(),
                    kind: SourceKind::MySql,
                    host: "127.0.0.1".into(),
                    port: 1,
                    database: "remote".into(),
                    ..DataSourceConfig::default()
                },
                Some("secret".into()),
            )
            .await
            .unwrap();

        let other = TempDir::new().unwrap();
        let reopened = SourceManager::load(
            path,
            Arc::new(crate::VaultCodec::new(test_vault(other.path()))),
        )
        .await;
        let id = reopened.configs().await[0].id.clone();
        let status = reopened.refresh(&id).await.unwrap();
        assert_eq!(status.state, "encryptKeyError");
        assert!(status.error.is_some());
        assert_eq!(reopened.configs().await.len(), 1);
    }

    #[test]
    fn source_errors_map_to_status_states() {
        assert_eq!(
            classify_source_error("access denied for user", false),
            "accessDenied"
        );
        assert_eq!(
            classify_source_error("authentication failed", false),
            "accessDenied"
        );
        assert_eq!(
            classify_source_error("the database was encrypted with another key", false),
            "encryptKeyError"
        );
        assert_eq!(
            classify_source_error("connection refused", true),
            "lostConnection"
        );
        assert_eq!(
            classify_source_error("connection refused", false),
            "otherError"
        );
    }

    #[tokio::test]
    async fn saved_sources_persist_encrypted_passwords_and_list_without_them() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hremote.dataSources.json");
        let vault = test_vault(dir.path());
        let manager =
            SourceManager::load(path.clone(), Arc::new(crate::VaultCodec::new(vault))).await;
        let saved = manager
            .save(
                DataSourceConfig {
                    name: "MySQL catalog".into(),
                    kind: SourceKind::MySql,
                    host: "db.internal".into(),
                    port: 3306,
                    database: "remote".into(),
                    username: "reader".into(),
                    ..DataSourceConfig::default()
                },
                Some("top-secret".into()),
            )
            .await
            .unwrap();
        assert!(saved.password.is_empty());
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("top-secret"));
        assert!(manager.statuses().await[0].state == "notConnectedYet");
        assert!(manager.configs().await[0].password.is_empty());

        // A fresh manager reloads the persisted definition (P8-12).
        let reopened = SourceManager::load(
            path,
            Arc::new(crate::VaultCodec::new(test_vault(dir.path()))),
        )
        .await;
        let configs = reopened.configs().await;
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].name, "MySQL catalog");
        assert_eq!(configs[0].kind, SourceKind::MySql);
        assert!(configs[0].read_only);
    }
}
