use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use hr_config::{
    Configuration, ConnectionRecords, JsonStore, LegacyProfile, ListViewLocality, Locality,
    PreflightReport, ProfilePaths, TagsLocality, TreeViewLocality, preflight,
};
use hr_core::{AppError, AppInfo, AppResult};
use hr_crypto::{Vault, VaultInit, VaultStatus};
use hr_domain::{Credential, Server};
use hr_files::FileManager;
use hr_i18n::I18n;
use hr_observability::{LogGuard, StartupTrace};
use hr_platform::{Capabilities, CliAction, detect_capabilities};
use hr_session::{
    BackendRegistry, IronRdpBackend, MstscBackend, ProcessBackend, SessionManager, VncBackend,
};
use hr_storage::{Database, Repository, SecretCodec, SourceManager, VaultCodec};
use hr_theme::ResolvedTheme;

pub const SAVE_DEBOUNCE: Duration = Duration::from_millis(300);

/// Composition root for the Tauri shell. Every service is created once here;
/// commands borrow from this state instead of constructing services ad hoc.
pub struct AppState {
    pub app_info: AppInfo,
    pub paths: ProfilePaths,
    pub config: Arc<JsonStore<Configuration>>,
    pub locality: Arc<JsonStore<Locality>>,
    pub tags: Arc<JsonStore<TagsLocality>>,
    pub connection_records: Arc<JsonStore<ConnectionRecords>>,
    pub list_view: Arc<JsonStore<ListViewLocality>>,
    pub tree_view: Arc<JsonStore<TreeViewLocality>>,
    pub i18n: Arc<I18n>,
    pub capabilities: Capabilities,
    pub startup: StartupTrace,
    pub preflight: PreflightReport,
    pub db: tokio::sync::Mutex<Option<Database>>,
    pub db_error: Mutex<Option<String>>,
    pub repository: Mutex<Option<Arc<Repository>>>,
    pub repository_error: Mutex<Option<String>>,
    pub assets: hr_config::AuxiliaryAssets,
    pub vault: RwLock<Option<Arc<Vault>>>,
    pub vault_status: VaultStatus,
    pub log_guard: Mutex<LogGuard>,
    pub pending_actions: Mutex<Vec<CliAction>>,
    pub hotkey_error: Mutex<Option<String>>,
    pub sessions: Arc<SessionManager>,
    pub files: Arc<FileManager>,
    /// Additional read-only data sources; `None` when the vault is unavailable.
    pub sources: Option<Arc<SourceManager>>,
    /// Last update-check result (P9-10).
    pub update_status: Mutex<Option<crate::updates::UpdateStatus>>,
    /// Catalog search cache; revision bumps on every local server mutation (P10-08).
    pub match_cache: Mutex<crate::match_cache::MatchCache>,
    pub catalog_revision: std::sync::atomic::AtomicU64,
    pub verified_until: Mutex<Option<std::time::Instant>>,
    pub data_ready: AtomicBool,
    pub exiting: AtomicBool,
    pub shutdown_started: AtomicBool,
    pub start_minimized: AtomicBool,
    pub first_run: bool,
    /// Computed once at start-up from usage thresholds (P9-11).
    pub rating_prompt: bool,
    pub legacy: Option<LegacyProfile>,
    pub recovered_files: Vec<PathBuf>,
}

impl AppState {
    pub async fn initialize(
        paths: ProfilePaths,
        startup: StartupTrace,
        log_guard: LogGuard,
        vault_init: VaultInit,
    ) -> AppResult<Self> {
        let preflight = preflight(&paths, true);
        if !preflight.ok {
            return Err(AppError::permission_denied(
                preflight
                    .first_error()
                    .unwrap_or_else(|| "profile paths are not writable".to_string()),
            ));
        }
        startup.mark("configuration");
        let config = Arc::new(hr_config::open_config_store(&paths, SAVE_DEBOUNCE)?);
        let locality = Arc::new(hr_config::open_locality_store(&paths, SAVE_DEBOUNCE)?);
        let tags = Arc::new(hr_config::open_tags_store(&paths, SAVE_DEBOUNCE)?);
        let connection_records = Arc::new(hr_config::open_connection_records_store(
            &paths,
            SAVE_DEBOUNCE,
        )?);
        let list_view = Arc::new(hr_config::open_list_view_store(&paths, SAVE_DEBOUNCE)?);
        let tree_view = Arc::new(hr_config::open_tree_view_store(&paths, SAVE_DEBOUNCE)?);

        let now_seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or_default();
        // Usage counters drive the rating prompt (P9-11); they are incremented
        // once per process, not per window.
        config.update(|config| {
            config.engagement.launches = config.engagement.launches.saturating_add(1);
            if config.engagement.first_run_at.is_none() {
                config.engagement.first_run_at = Some(now_seconds);
            }
        });
        if let Err(error) = config.flush() {
            tracing::warn!(%error, "cannot persist engagement counters");
        }
        let snapshot = config.get();
        let rating_prompt = hr_config::rating_prompt_due(&snapshot.engagement, now_seconds);
        let first_run = !snapshot.general.guidance_completed;
        let i18n = Arc::new(I18n::new(snapshot.language())?);
        startup.mark("localisation");

        let capabilities = detect_capabilities();
        let startup_trace = startup;

        let db_path = snapshot.database_path(&paths.database_file);
        let (db, db_error) = match Database::open_sqlite(&db_path).await {
            Ok(database) => (Some(database), None),
            Err(error) => {
                tracing::error!(%error, path = %db_path.display(), "local database unavailable");
                (None, Some(error.to_string()))
            }
        };
        startup_trace.mark("database");

        let salt_path = paths.locality_dir.join(hr_crypto::SALT_FILE_NAME);
        let (vault, vault_status) = match vault_init {
            VaultInit::Ready(vault) => {
                let vault = Arc::new(vault);
                let status = VaultStatus {
                    ready: true,
                    key_store: Some(vault.key_store_kind()),
                    salt_path: salt_path.clone(),
                    message: None,
                };
                (Some(vault), status)
            }
            VaultInit::Unavailable(status) => (None, status),
        };
        if !vault_status.ready {
            tracing::warn!(
                message = ?vault_status.message,
                "encrypted storage is unavailable; secret features are disabled"
            );
        }
        startup_trace.mark("vault");

        let (repository, repository_error) = match (&vault, &db) {
            (Some(vault), Some(database)) => {
                let codec = Arc::new(VaultCodec::new(Arc::clone(vault)));
                match Repository::open(database.pool().clone(), codec).await {
                    Ok(repository) => (Some(Arc::new(repository)), None),
                    Err(error) => {
                        tracing::error!(%error, "catalog repository unavailable");
                        (None, Some(error.to_string()))
                    }
                }
            }
            (None, _) => (
                None,
                Some(
                    vault_status
                        .message
                        .clone()
                        .unwrap_or_else(|| "encrypted storage is unavailable".to_string()),
                ),
            ),
            (_, None) => (None, db_error.clone()),
        };
        startup_trace.mark("repository");

        // Additional sources share the vault codec so their passwords stay
        // encrypted at rest in `hremote.dataSources.json` (P8-12).
        let sources = match &vault {
            Some(vault) => {
                let codec: Arc<dyn SecretCodec> = Arc::new(VaultCodec::new(Arc::clone(vault)));
                Some(Arc::new(
                    SourceManager::load(paths.data_sources_file.clone(), codec).await,
                ))
            }
            None => None,
        };
        startup_trace.mark("sources");

        let mut recovered_files = Vec::new();
        for store in [
            config.recovered_from(),
            locality.recovered_from(),
            tags.recovered_from(),
            connection_records.recovered_from(),
            list_view.recovered_from(),
            tree_view.recovered_from(),
        ]
        .into_iter()
        .flatten()
        {
            recovered_files.push(store.to_path_buf());
        }

        let legacy = if snapshot.general.skip_legacy_migration {
            None
        } else {
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(PathBuf::from))
                .unwrap_or_else(|| paths.base_dir.clone());
            hr_config::detect_legacy_profiles(&hr_config::LegacyScanInput {
                exe_dir,
                config_dir: dirs::config_dir(),
                data_local_dir: dirs::data_local_dir(),
            })
            .into_iter()
            .find(|profile| profile.config_file.is_some())
        };

        let start_minimized = snapshot.general.start_minimized;
        let sessions = Arc::new(SessionManager::new(
            BackendRegistry::new(Arc::new(IronRdpBackend), Arc::new(MstscBackend))
                .with_vnc(Arc::new(VncBackend))
                .with_process(Arc::new(ProcessBackend)),
        ));
        let files = Arc::new(FileManager::new());
        let assets = hr_config::load_assets(&paths);
        for warning in &assets.warnings {
            tracing::warn!(%warning, "auxiliary profile asset problem");
        }
        let state = Self {
            app_info: AppInfo::current(),
            paths,
            config,
            locality,
            tags,
            connection_records,
            list_view,
            tree_view,
            i18n,
            capabilities,
            startup: startup_trace,
            preflight,
            db: tokio::sync::Mutex::new(db),
            db_error: Mutex::new(db_error),
            repository: Mutex::new(repository),
            repository_error: Mutex::new(repository_error),
            assets,
            vault: RwLock::new(vault),
            vault_status,
            log_guard: Mutex::new(log_guard),
            pending_actions: Mutex::new(Vec::new()),
            hotkey_error: Mutex::new(None),
            sessions,
            files,
            sources,
            update_status: Mutex::new(None),
            match_cache: Mutex::new(crate::match_cache::MatchCache::new()),
            catalog_revision: std::sync::atomic::AtomicU64::new(1),
            verified_until: Mutex::new(None),
            data_ready: AtomicBool::new(false),
            exiting: AtomicBool::new(false),
            shutdown_started: AtomicBool::new(false),
            start_minimized: AtomicBool::new(start_minimized),
            first_run,
            rating_prompt,
            legacy,
            recovered_files,
        };
        state.startup.mark("state");
        Ok(state)
    }

    pub fn config_snapshot(&self) -> Configuration {
        self.config.get()
    }

    pub fn repository(&self) -> AppResult<Arc<Repository>> {
        self.repository
            .lock()
            .expect("repository lock poisoned")
            .clone()
            .ok_or_else(|| {
                AppError::unavailable(
                    self.repository_error
                        .lock()
                        .expect("repository error lock poisoned")
                        .clone()
                        .unwrap_or_else(|| "catalog storage is unavailable".to_string()),
                )
            })
    }

    pub fn install_repository(&self, repository: Option<Arc<Repository>>, error: Option<String>) {
        *self.repository.lock().expect("repository lock poisoned") = repository;
        *self
            .repository_error
            .lock()
            .expect("repository error lock poisoned") = error;
    }

    pub fn sources(&self) -> AppResult<Arc<SourceManager>> {
        self.sources.clone().ok_or_else(|| {
            AppError::unavailable("additional data sources require the encrypted vault".to_string())
        })
    }

    /// Additional-source servers, empty when the vault is unavailable. Read
    /// failures are logged and degraded to an empty list so a broken secondary
    /// source never blocks the local catalog (P8-13).
    pub async fn extra_servers(&self) -> Vec<Server> {
        match &self.sources {
            Some(sources) => sources.servers().await,
            None => Vec::new(),
        }
    }

    pub async fn extra_credentials(&self) -> Vec<Credential> {
        match &self.sources {
            Some(sources) => sources.credentials().await,
            None => Vec::new(),
        }
    }

    /// Invalidates cached match results after any local catalog write.
    pub fn bump_catalog_revision(&self) {
        self.catalog_revision
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Finds a server in the local catalog first, then in read-only sources.
    pub async fn find_server(&self, id: &str) -> AppResult<Option<Server>> {
        if let Ok(repository) = self.repository()
            && let Some(server) = repository.get_server(id).await?
        {
            return Ok(Some(server));
        }
        Ok(self
            .extra_servers()
            .await
            .into_iter()
            .find(|server| server.base().id == id))
    }

    pub fn theme(&self) -> AppResult<ResolvedTheme> {
        hr_theme::resolve(&self.config_snapshot().theme)
            .map_err(|error| AppError::config(error.to_string()))
    }

    pub fn set_log_level(&self, level: hr_config::LogLevel) -> AppResult<()> {
        let guard = self.log_guard.lock().expect("log guard poisoned");
        guard.set_directive(level.directive())
    }

    pub fn queue_actions(&self, actions: Vec<CliAction>) {
        if actions.contains(&CliAction::StartMinimized) {
            self.start_minimized.store(true, Ordering::SeqCst);
        }
        self.pending_actions
            .lock()
            .expect("pending action lock poisoned")
            .extend(actions);
    }

    pub fn take_pending_actions(&self) -> Vec<CliAction> {
        std::mem::take(
            &mut *self
                .pending_actions
                .lock()
                .expect("pending action lock poisoned"),
        )
    }

    /// Deterministic shutdown: flush every store, close the DB pool, drop the
    /// vault key and log guard. Called once from the run-event loop.
    pub async fn shutdown(&self) {
        for (name, result) in [
            ("config", self.config.flush()),
            ("locality", self.locality.flush()),
            ("tags", self.tags.flush()),
            ("connection records", self.connection_records.flush()),
            ("list view", self.list_view.flush()),
            ("tree view", self.tree_view.flush()),
        ] {
            if let Err(error) = result {
                tracing::error!(%error, store = name, "failed to flush store during shutdown");
            }
        }
        self.sessions.shutdown().await;
        self.files.shutdown().await;
        if let Some(sources) = &self.sources {
            sources.shutdown().await;
        }
        if let Some(repository) = self
            .repository
            .lock()
            .expect("repository lock poisoned")
            .take()
        {
            drop(repository);
        }
        if let Some(database) = self.db.lock().await.take() {
            database.close().await;
        }
        if let Some(vault) = self.vault.write().expect("vault lock poisoned").take() {
            drop(vault); // zeroizes the key when the last Arc is gone
        }
        tracing::info!("H-Remote state shut down");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hr_config::{ProfileMode, ResolveInput, resolve_profile};
    use hr_observability::init_logging;
    use tempfile::TempDir;

    #[tokio::test]
    async fn state_initialises_from_a_temporary_profile() {
        let workspace = TempDir::new().unwrap();
        let config_dir = TempDir::new().unwrap();
        let data_dir = TempDir::new().unwrap();
        let paths = resolve_profile(&ResolveInput {
            exe_dir: workspace.path().to_path_buf(),
            config_dir: Some(config_dir.path().to_path_buf()),
            data_local_dir: Some(data_dir.path().to_path_buf()),
            exe_dir_writable: Some(true),
            ..ResolveInput::default()
        })
        .unwrap();
        assert_eq!(paths.mode, ProfileMode::Portable);
        paths.ensure_layout().unwrap();
        let log_guard = init_logging(&paths.logs_dir, "error").unwrap();
        let unavailable = VaultInit::Unavailable(VaultStatus {
            ready: false,
            key_store: None,
            salt_path: paths.locality_dir.join(hr_crypto::SALT_FILE_NAME),
            message: Some("disabled in test".to_string()),
        });
        let state = AppState::initialize(paths, StartupTrace::new(), log_guard, unavailable)
            .await
            .unwrap();
        assert!(state.first_run);
        assert!(state.config.get().general.language == "system");
        assert!(state.db.lock().await.is_some());
        assert!(state.startup.snapshot().len() >= 4);
        state.shutdown().await;
    }

    #[tokio::test]
    async fn state_wires_the_catalog_repository_when_a_vault_is_ready() {
        let workspace = TempDir::new().unwrap();
        let config_dir = TempDir::new().unwrap();
        let data_dir = TempDir::new().unwrap();
        let paths = resolve_profile(&ResolveInput {
            exe_dir: workspace.path().to_path_buf(),
            config_dir: Some(config_dir.path().to_path_buf()),
            data_local_dir: Some(data_dir.path().to_path_buf()),
            exe_dir_writable: Some(true),
            ..ResolveInput::default()
        })
        .unwrap();
        paths.ensure_layout().unwrap();
        let log_guard = init_logging(&paths.logs_dir, "error").unwrap();
        let salt =
            hr_crypto::InstallationSalt::load_or_create(paths.locality_dir.join("salt")).unwrap();
        let vault = hr_crypto::Vault::from_parts([5u8; 32], &salt, hr_crypto::KeyStoreKind::Memory);
        let state = AppState::initialize(
            paths,
            StartupTrace::new(),
            log_guard,
            VaultInit::Ready(vault),
        )
        .await
        .unwrap();
        assert!(state.repository.lock().unwrap().is_some());
        let repository = state.repository().unwrap();
        assert!(repository.encryption_verified().await.unwrap());
        assert_eq!(repository.server_count().await.unwrap(), 0);
        state.shutdown().await;
    }
}
