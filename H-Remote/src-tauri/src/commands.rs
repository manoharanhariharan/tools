use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hr_config::{
    Configuration, ConnectionRecords, ListViewLocality, Locality, LogLevel, ProfileMode,
    QuickConnectEntry, ServerView, TagsLocality, TreeViewLocality, WindowPlacement,
};
use hr_core::{AppError, AppInfo, AppResult, IpcError, is_valid_id};
use hr_domain::{
    CatalogSort, Credential, Folder, LOCAL_SOURCE_ID, MatchProvider, Server, build_catalog_tree,
    collect_tags, export_json, export_rdp, import_json, import_mremoteng_csv, import_rdp, search,
};
use hr_i18n::{LocaleInfo, supported_locales};
use hr_observability::Checkpoint;
use hr_platform::{Capabilities, ShortcutSpec, TrimOutcome};
use hr_session::{
    Frame, ProcessPlan, RdpPlan, ReconnectPolicy, SessionBackend, SessionCommand, SessionSnapshot,
    SessionState, SpecialKey, VncPlan,
};
use hr_storage::{DataSourceConfig, Database, SourceStatus};
use hr_theme::{ThemeConfig, ThemePreset};
use tauri::{AppHandle, Emitter, State};

use crate::dto::*;
use crate::integrations;
use crate::updates::UpdateStatus;
use crate::{AppState, activation, hotkey, session_scripts, window};

#[tauri::command]
pub async fn bootstrap(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BootstrapDto, IpcError> {
    let config = state.config_snapshot();
    let theme = state.theme()?;
    let database = database_info(&state).await;
    let pending_actions = state
        .pending_actions
        .lock()
        .expect("pending action lock poisoned")
        .clone();
    let bootstrap = BootstrapDto {
        app: state.app_info.clone(),
        paths: PathsDto::from(&state.paths),
        first_run: state.first_run,
        config: config.clone(),
        theme,
        themes: hr_theme::presets(),
        locales: supported_locales(),
        locale: state.i18n.locale(),
        system_locale: state.i18n.system_locale().to_string(),
        capabilities: state.capabilities.clone(),
        vault: state.vault_status.clone(),
        preflight: state.preflight.clone(),
        startup: state.startup.snapshot(),
        database,
        legacy: state.legacy.clone(),
        auxiliary: state.assets.clone(),
        protocols: hr_core::all_protocols(),
        recovered_files: state
            .recovered_files
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        marker_locked: state.paths.marker_file.is_some(),
        feedback_url: integrations::FEEDBACK_URL.to_string(),
        log_level: config.general.log_level,
        pending_actions,
        hotkey_error: state
            .hotkey_error
            .lock()
            .expect("hotkey error lock poisoned")
            .clone(),
        show_rating_prompt: state.rating_prompt,
    };
    let _ = app.emit("app:bootstrapped", ());
    Ok(bootstrap)
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Configuration {
    state.config_snapshot()
}

#[tauri::command]
pub fn update_config(
    app: AppHandle,
    state: State<'_, AppState>,
    mut config: Configuration,
) -> Result<Configuration, IpcError> {
    let old = state.config_snapshot();
    config.sanitize();
    hr_theme::resolve(&config.theme).map_err(|error| AppError::validation(error.to_string()))?;

    if config.launcher.enabled {
        config.launcher.hotkey = hr_platform::canonical_hotkey(&config.launcher.hotkey)
            .map_err(|error| AppError::validation(error.to_string()))?;
    }

    let canonical_locale = apply_locale(&state.i18n, &config)?;
    if config.language().is_some() {
        config.general.language = canonical_locale;
    } else {
        config.general.language = "system".to_string();
    }

    if old.general.run_at_startup != config.general.run_at_startup {
        if config.general.run_at_startup {
            integrations::install_startup(&app)?;
        } else {
            integrations::uninstall_startup(&app)?;
        }
    }

    if old.launcher.enabled != config.launcher.enabled
        || old.launcher.hotkey != config.launcher.hotkey
    {
        if let Err(error) = hotkey::apply(&app, config.launcher.enabled, &config.launcher.hotkey) {
            let _ = hotkey::apply(&app, old.launcher.enabled, &old.launcher.hotkey);
            return Err(error.into());
        }
    }

    let updated = state.config.replace(config);
    state.set_log_level(updated.general.log_level)?;
    state.config.flush()?;
    let _ = app.emit("config:changed", &updated);
    Ok(updated)
}

fn apply_locale(i18n: &hr_i18n::I18n, config: &Configuration) -> AppResult<String> {
    let requested = config
        .language()
        .map(str::to_string)
        .unwrap_or_else(|| i18n.system_locale().to_string());
    i18n.set_locale(&requested)
}

#[tauri::command]
pub fn get_themes() -> Vec<ThemePreset> {
    hr_theme::presets()
}

#[tauri::command]
pub fn resolve_theme(theme: ThemeConfig) -> Result<ThemeValidation, IpcError> {
    let resolved =
        hr_theme::resolve(&theme).map_err(|error| AppError::validation(error.to_string()))?;
    let css_variables = resolved.css_variables();
    Ok(ThemeValidation {
        theme: resolved,
        css_variables,
    })
}

#[tauri::command]
pub fn get_locales(state: State<'_, AppState>) -> (Vec<LocaleInfo>, String, String) {
    (
        supported_locales(),
        state.i18n.locale(),
        state.i18n.system_locale().to_string(),
    )
}

#[tauri::command]
pub fn get_locality(state: State<'_, AppState>) -> Locality {
    state.locality.get()
}

#[tauri::command]
pub fn save_window_placement(
    state: State<'_, AppState>,
    placement: WindowPlacement,
) -> Result<(), IpcError> {
    state
        .locality
        .update(|locality| locality.ui.main_window = Some(placement));
    state.locality.flush()?;
    Ok(())
}

#[tauri::command]
pub fn reset_main_window(app: AppHandle) -> Result<(), IpcError> {
    window::reset_main_window(&app)?;
    Ok(())
}

#[tauri::command]
pub fn set_server_view(state: State<'_, AppState>, view: ServerView) -> Result<(), IpcError> {
    state
        .locality
        .update(|locality| locality.ui.server_view = view);
    state.locality.flush()?;
    Ok(())
}

#[tauri::command]
pub fn get_connection_records(state: State<'_, AppState>) -> ConnectionRecords {
    state.connection_records.get()
}

#[tauri::command]
pub fn record_connection(state: State<'_, AppState>, server_id: String) -> Result<(), IpcError> {
    if !is_valid_id(&server_id) {
        return Err(AppError::validation("invalid server id").into());
    }
    let now = unix_seconds();
    state
        .connection_records
        .update(|records| records.record_connection(&server_id, now));
    state.connection_records.flush()?;
    Ok(())
}

#[tauri::command]
pub fn recent_sessions(state: State<'_, AppState>) -> Vec<RecentSessionInfo> {
    let records = state.connection_records.get();
    let limit = usize::from(state.config_snapshot().launcher.recent_limit);
    records
        .recent_server_ids(limit)
        .into_iter()
        .filter_map(|id| {
            records.records.get(&id).map(|record| RecentSessionInfo {
                server_id: id,
                last_connected_at: record.last_connected_at,
                connect_count: record.connect_count,
            })
        })
        .collect()
}

#[tauri::command]
pub fn get_tags(state: State<'_, AppState>) -> TagsLocality {
    state.tags.get()
}

#[tauri::command]
pub fn set_pinned_tags(
    state: State<'_, AppState>,
    tags: Vec<String>,
) -> Result<TagsLocality, IpcError> {
    let updated = state.tags.update(|locality| {
        locality.pinned = tags;
        locality.sanitize();
    });
    state.tags.flush()?;
    Ok(updated)
}

#[tauri::command]
pub fn set_tag_order(
    state: State<'_, AppState>,
    order: Vec<String>,
) -> Result<TagsLocality, IpcError> {
    let updated = state.tags.update(|locality| {
        locality.order = order;
        locality.sanitize();
    });
    state.tags.flush()?;
    Ok(updated)
}

#[tauri::command]
pub fn open_log_folder(state: State<'_, AppState>) -> Result<(), IpcError> {
    integrations::open_in_file_manager(&state.paths.logs_dir)?;
    Ok(())
}

#[tauri::command]
pub fn open_config_folder(state: State<'_, AppState>) -> Result<(), IpcError> {
    integrations::open_in_file_manager(&state.paths.base_dir)?;
    Ok(())
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), IpcError> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(AppError::validation("only http(s) URLs may be opened").into());
    }
    integrations::open_url(&url)?;
    Ok(())
}

#[tauri::command]
pub fn read_log_tail(state: State<'_, AppState>, lines: usize) -> Result<String, IpcError> {
    let path = state.paths.log_file();
    Ok(read_tail(&path, lines.clamp(1, 2000), 2 * 1024 * 1024)?)
}

#[tauri::command]
pub fn report_ui_error(
    state: State<'_, AppState>,
    message: String,
    details: String,
) -> Result<hr_observability::CrashReport, IpcError> {
    let report = hr_observability::write_crash_report(
        &state.paths.logs_dir,
        "ui",
        &message,
        &details,
        &state.app_info,
    )?;
    Ok(report)
}

#[tauri::command]
pub fn save_error_report(state: State<'_, AppState>, markdown: String) -> Result<String, IpcError> {
    if markdown.len() > 1024 * 1024 {
        return Err(AppError::validation("error report is too large").into());
    }
    std::fs::create_dir_all(&state.paths.logs_dir)?;
    let path = state
        .paths
        .logs_dir
        .join(format!("error-report-{}.md", unix_millis()));
    std::fs::write(&path, markdown.as_bytes())?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn list_crash_reports(state: State<'_, AppState>) -> Vec<CrashReportInfo> {
    hr_observability::list_crash_reports(&state.paths.logs_dir)
        .into_iter()
        .filter_map(|path| {
            let metadata = std::fs::metadata(&path).ok();
            Some(CrashReportInfo {
                file_name: path.file_name()?.to_string_lossy().to_string(),
                modified_at: metadata
                    .and_then(|metadata| metadata.modified().ok())
                    .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| duration.as_secs() as i64),
                path,
            })
        })
        .collect()
}

#[tauri::command]
pub fn read_crash_report(
    state: State<'_, AppState>,
    file_name: String,
) -> Result<String, IpcError> {
    if file_name.contains(['/', '\\', ':']) || file_name.contains("..") {
        return Err(AppError::validation("invalid report file name").into());
    }
    let path = state.paths.logs_dir.join(&file_name);
    if path.parent() != Some(state.paths.logs_dir.as_path()) {
        return Err(AppError::validation("invalid report path").into());
    }
    if !(file_name.starts_with("crash-") || file_name.starts_with("error-report-")) {
        return Err(AppError::validation("not an H-Remote report").into());
    }
    let text = read_tail(&path, usize::MAX, 2 * 1024 * 1024)?;
    Ok(text)
}

#[tauri::command]
pub fn trim_working_set() -> TrimOutcome {
    hr_platform::trim_working_set()
}

#[tauri::command]
pub fn flash_window(app: AppHandle) {
    window::flash(&app);
}

#[tauri::command]
pub fn show_main_window(app: AppHandle) {
    window::show_main(&app);
}

#[tauri::command]
pub fn validate_hotkey(hotkey: String) -> Result<String, IpcError> {
    hr_platform::canonical_hotkey(&hotkey)
        .map_err(|error| AppError::validation(error.to_string()).into())
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    state.app_info.clone()
}

#[tauri::command]
pub fn capabilities(state: State<'_, AppState>) -> Capabilities {
    state.capabilities.clone()
}

#[tauri::command]
pub fn startup_checkpoints(state: State<'_, AppState>) -> Vec<Checkpoint> {
    state.startup.snapshot()
}

#[tauri::command]
pub fn vault_status(state: State<'_, AppState>) -> Result<hr_crypto::VaultStatus, IpcError> {
    Ok(state.vault_status.clone())
}

#[tauri::command]
pub async fn database_status(state: State<'_, AppState>) -> Result<DatabaseInfo, IpcError> {
    Ok(database_info(&state).await)
}

#[tauri::command]
pub fn set_log_level(
    state: State<'_, AppState>,
    level: LogLevel,
) -> Result<Configuration, IpcError> {
    let updated = state
        .config
        .update(|config| config.general.log_level = level);
    state.set_log_level(level)?;
    state.config.flush()?;
    Ok(updated)
}

#[tauri::command]
pub fn complete_guidance(
    app: AppHandle,
    state: State<'_, AppState>,
    selection: GuidanceSelection,
) -> Result<GuidanceResult, IpcError> {
    let old = state.config_snapshot();
    let mut warnings = Vec::new();
    let mut migrated = None;

    let mut config = old.clone();
    if selection.import_legacy_settings
        && let Some(legacy) = state.legacy.as_ref()
    {
        match hr_config::import_legacy_settings(legacy, &mut config) {
            Ok(settings) => migrated = Some(settings),
            Err(error) => warnings.push(format!("legacy settings: {error}")),
        }
    }

    if let Some(language) = selection.language.as_deref() {
        config.general.language = language.to_string();
    }
    if let Some(custom) = selection.custom_theme {
        config.theme = custom;
    } else if let Some(name) = selection.theme_name.as_deref() {
        config
            .theme
            .select_preset(name)
            .map_err(|error| AppError::validation(error.to_string()))?;
    }
    config.general.confirm_before_closing_session = selection.confirm_before_closing_session;
    config.general.start_minimized = selection.start_minimized;
    config.general.run_at_startup = selection.run_at_startup;
    config.general.guidance_completed = true;
    config.sanitize();
    hr_theme::resolve(&config.theme).map_err(|error| AppError::validation(error.to_string()))?;
    let canonical_locale = apply_locale(&state.i18n, &config)?;
    config.general.language = if config.language().is_some() {
        canonical_locale
    } else {
        "system".to_string()
    };

    if selection.run_at_startup != old.general.run_at_startup {
        let result = if selection.run_at_startup {
            integrations::install_startup(&app)
        } else {
            integrations::uninstall_startup(&app)
        };
        if let Err(error) = result {
            config.general.run_at_startup = old.general.run_at_startup;
            warnings.push(error.to_string());
        }
    }

    let mut shortcut_path = None;
    if selection.create_shortcut {
        match hr_platform::default_desktop_dir()
            .and_then(|desktop| integrations::install_desktop_shortcut(&desktop))
        {
            Ok(path) => shortcut_path = Some(path.display().to_string()),
            Err(error) => warnings.push(error.to_string()),
        }
    }

    let mut restart_required = false;
    if let Some(mode) = selection.profile_mode
        && mode != state.paths.mode
    {
        match (
            integrations::executable_dir(),
            dirs::config_dir()
                .ok_or_else(|| AppError::platform("user configuration directory is unavailable")),
        ) {
            (Ok(executable_dir), Ok(config_dir)) => {
                match integrations::write_profile_marker(mode, &executable_dir, &config_dir) {
                    Ok(_) => restart_required = true,
                    Err(error) => warnings.push(error.to_string()),
                }
            }
            _ => warnings.push("cannot write profile marker".to_string()),
        }
    }

    if hotkey::apply(&app, config.launcher.enabled, &config.launcher.hotkey).is_err() {
        warnings.push("the launcher hotkey could not be registered".to_string());
    }

    let updated = state.config.replace(config);
    state.set_log_level(updated.general.log_level)?;
    state.config.flush()?;
    let _ = app.emit("config:changed", &updated);

    Ok(GuidanceResult {
        config: updated,
        restart_required,
        migrated,
        shortcut_path,
        warnings,
    })
}

#[tauri::command]
pub fn set_profile_mode(
    state: State<'_, AppState>,
    mode: ProfileMode,
) -> Result<ProfileChange, IpcError> {
    let executable_dir = integrations::executable_dir()?;
    let config_dir = dirs::config_dir()
        .ok_or_else(|| AppError::platform("user configuration directory is unavailable"))?;
    let marker = integrations::write_profile_marker(mode, &executable_dir, &config_dir)?;
    Ok(ProfileChange {
        mode,
        marker_path: Some(marker.display().to_string()),
        restart_required: mode != state.paths.mode,
    })
}

#[tauri::command]
pub fn detect_legacy_profile(state: State<'_, AppState>) -> Option<hr_config::LegacyProfile> {
    state.legacy.clone()
}

#[tauri::command]
pub fn migrate_legacy_settings(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<hr_config::LegacySettings, IpcError> {
    let legacy = state
        .legacy
        .as_ref()
        .ok_or_else(|| AppError::not_found("legacy profile"))?;
    let mut config = state.config_snapshot();
    let settings = hr_config::import_legacy_settings(legacy, &mut config)?;
    config.sanitize();
    let canonical_locale = apply_locale(&state.i18n, &config)?;
    if config.language().is_some() {
        config.general.language = canonical_locale;
    }
    let updated = state.config.replace(config);
    state.config.flush()?;
    let _ = app.emit("config:changed", &updated);
    Ok(settings)
}

#[tauri::command]
pub fn dismiss_legacy_migration(state: State<'_, AppState>) -> Result<Configuration, IpcError> {
    let updated = state
        .config
        .update(|config| config.general.skip_legacy_migration = true);
    state.config.flush()?;
    Ok(updated)
}

/// Path of the legacy SQLite catalog, if one was detected (P9-08).
#[tauri::command]
pub fn legacy_database(state: State<'_, AppState>) -> Option<String> {
    state
        .legacy
        .as_ref()
        .and_then(hr_config::find_legacy_database)
        .map(|path| path.display().to_string())
}

/// Imports servers from the detected PRemoteM/1Remote catalog into the writable
/// local source. Settings migration stays a separate, explicit step.
#[tauri::command]
pub fn import_legacy_servers(state: State<'_, AppState>) -> Result<ImportOutcome, IpcError> {
    let profile = state
        .legacy
        .as_ref()
        .ok_or_else(|| AppError::not_found("legacy profile"))?;
    let path = hr_config::find_legacy_database(profile)
        .ok_or_else(|| AppError::not_found("legacy catalog database"))?;
    let repository = state.repository()?;
    let result = tauri::async_runtime::block_on(hr_storage::import::import_database(&path))?;
    tauri::async_runtime::block_on(persist_import_with_state(&state, &repository, result))
}

#[tauri::command]
pub fn install_desktop_shortcut() -> Result<String, IpcError> {
    let desktop = hr_platform::default_desktop_dir()?;
    let path = integrations::install_desktop_shortcut(&desktop)?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn uninstall_desktop_shortcut() -> Result<bool, IpcError> {
    let desktop = hr_platform::default_desktop_dir()?;
    Ok(integrations::uninstall_desktop_shortcut(&desktop)?)
}

#[tauri::command]
pub fn install_startup(app: AppHandle) -> Result<(), IpcError> {
    integrations::install_startup(&app)?;
    Ok(())
}

#[tauri::command]
pub fn uninstall_startup(app: AppHandle) -> Result<(), IpcError> {
    integrations::uninstall_startup(&app)?;
    Ok(())
}

#[tauri::command]
pub fn startup_status(app: AppHandle) -> Result<bool, IpcError> {
    Ok(integrations::startup_enabled(&app)?)
}

#[tauri::command]
pub fn ui_ready(app: AppHandle) {
    activation::ui_ready(&app);
}

#[tauri::command]
pub fn handle_cli(app: AppHandle, args: Vec<String>) -> CliInvocationSummary {
    let invocation = hr_platform::parse_cli_args(args);
    let errors = invocation.errors.clone();
    let actions = invocation.actions.clone();
    activation::handle(&app, invocation);
    CliInvocationSummary { actions, errors }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliInvocationSummary {
    pub actions: Vec<hr_platform::CliAction>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct I18nReport {
    pub locale: String,
    pub missing: Vec<String>,
    pub locales: Vec<hr_i18n::LocaleInfo>,
}

#[tauri::command]
pub fn i18n_report(state: State<'_, AppState>) -> I18nReport {
    I18nReport {
        locale: state.i18n.locale(),
        missing: state.i18n.missing_keys(),
        locales: supported_locales(),
    }
}

#[tauri::command]
pub fn list_protocols() -> Vec<hr_core::ProtocolDescriptor> {
    hr_core::all_protocols()
}

#[tauri::command]
pub fn new_server(protocol: String) -> Result<Server, IpcError> {
    let kind = hr_domain::ProtocolKind::from_id(&protocol)
        .ok_or_else(|| AppError::validation(format!("unknown protocol '{protocol}'")))?;
    let server = match kind {
        hr_domain::ProtocolKind::Rdp => Server::new_rdp("New server", "", 3389),
        hr_domain::ProtocolKind::RemoteApp => Server::new_remote_app("New application", "", ""),
        hr_domain::ProtocolKind::Vnc => Server::new_vnc("New VNC server", "", 5900),
        hr_domain::ProtocolKind::Ssh => Server::new_ssh("New SSH server", "", ""),
        hr_domain::ProtocolKind::Telnet => Server::new_telnet("New Telnet server", "", 23),
        hr_domain::ProtocolKind::Serial => Server::new_serial("New serial session", "COM1"),
        hr_domain::ProtocolKind::Sftp => Server::new_sftp("New SFTP server", "", ""),
        hr_domain::ProtocolKind::Ftp => Server::new_ftp("New FTP server", "", ""),
        hr_domain::ProtocolKind::LocalApp => Server::new_local_app("New local app", ""),
        hr_domain::ProtocolKind::Custom => Server::new_custom("New custom server", ""),
        hr_domain::ProtocolKind::Dummy => Server::new_dummy("Placeholder"),
    };
    Ok(server)
}

/// A server owned by an extra (read-only) source must never be written back to
/// the local catalog (P8-05).
fn ensure_writable_server(server: &Server) -> AppResult<()> {
    if server.base().source_id == LOCAL_SOURCE_ID {
        Ok(())
    } else {
        Err(AppError::permission_denied(format!(
            "'{}' belongs to a read-only data source and cannot be modified",
            server.base().display_name_or("server")
        )))
    }
}

/// Local catalog first, then read-only sources (ids are stable across both).
async fn merged_servers(state: &AppState) -> AppResult<Vec<Server>> {
    let repository = state.repository()?;
    let mut servers = repository.list_servers().await?;
    let mut seen: HashSet<String> = servers
        .iter()
        .map(|server| server.base().id.clone())
        .collect();
    for server in state.extra_servers().await {
        if seen.insert(server.base().id.clone()) {
            servers.push(server);
        }
    }
    Ok(servers)
}

async fn merged_credentials(state: &AppState) -> AppResult<Vec<Credential>> {
    let repository = state.repository()?;
    let mut credentials = repository.list_credentials().await?;
    let mut seen: HashSet<String> = credentials
        .iter()
        .map(|credential| credential.id.clone())
        .collect();
    for credential in state.extra_credentials().await {
        if seen.insert(credential.id.clone()) {
            credentials.push(credential);
        }
    }
    Ok(credentials)
}

#[tauri::command]
pub async fn list_servers(state: State<'_, AppState>) -> Result<Vec<Server>, IpcError> {
    Ok(merged_servers(&state).await?)
}

#[tauri::command]
pub async fn list_servers_page(
    state: State<'_, AppState>,
    offset: u32,
    limit: u32,
) -> Result<Vec<Server>, IpcError> {
    Ok(state.repository()?.list_servers_page(offset, limit).await?)
}

#[tauri::command]
pub async fn get_server(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<Server>, IpcError> {
    Ok(state.find_server(&id).await?)
}

#[tauri::command]
pub async fn save_server(state: State<'_, AppState>, server: Server) -> Result<Server, IpcError> {
    ensure_writable_server(&server)?;
    let saved = state.repository()?.upsert_server(server).await?;
    state.bump_catalog_revision();
    Ok(saved)
}

#[tauri::command]
pub async fn save_servers(
    state: State<'_, AppState>,
    servers: Vec<Server>,
) -> Result<Vec<Server>, IpcError> {
    for server in &servers {
        ensure_writable_server(server)?;
    }
    let saved = state.repository()?.upsert_servers(servers).await?;
    state.bump_catalog_revision();
    Ok(saved)
}

#[tauri::command]
pub async fn delete_server(state: State<'_, AppState>, id: String) -> Result<bool, IpcError> {
    if let Some(server) = state.find_server(&id).await? {
        ensure_writable_server(&server)?;
    }
    let removed = state.repository()?.delete_server(&id).await?;
    if removed {
        state.bump_catalog_revision();
    }
    Ok(removed)
}

#[tauri::command]
pub async fn list_credentials(state: State<'_, AppState>) -> Result<Vec<Credential>, IpcError> {
    Ok(merged_credentials(&state).await?)
}

fn ensure_writable_credential(credential: &Credential) -> AppResult<()> {
    if credential.is_editable {
        Ok(())
    } else {
        Err(AppError::permission_denied(format!(
            "credential '{}' belongs to a read-only data source",
            credential.name
        )))
    }
}

#[tauri::command]
pub async fn save_credential(
    state: State<'_, AppState>,
    credential: Credential,
) -> Result<Credential, IpcError> {
    ensure_writable_credential(&credential)?;
    Ok(state.repository()?.upsert_credential(credential).await?)
}

#[tauri::command]
pub async fn save_credentials(
    state: State<'_, AppState>,
    credentials: Vec<Credential>,
) -> Result<Vec<Credential>, IpcError> {
    for credential in &credentials {
        ensure_writable_credential(credential)?;
    }
    Ok(state.repository()?.upsert_credentials(credentials).await?)
}

#[tauri::command]
pub async fn delete_credential(state: State<'_, AppState>, id: String) -> Result<bool, IpcError> {
    if state
        .extra_credentials()
        .await
        .iter()
        .any(|credential| credential.id == id)
    {
        return Err(AppError::permission_denied(
            "credentials in read-only data sources cannot be deleted",
        )
        .into());
    }
    Ok(state.repository()?.delete_credential(&id).await?)
}

#[tauri::command]
pub async fn create_credential(
    state: State<'_, AppState>,
    name: String,
    username: String,
    password: String,
) -> Result<Credential, IpcError> {
    let mut credential = Credential::new(name, username);
    credential.password = password.into();
    Ok(state.repository()?.upsert_credential(credential).await?)
}

#[tauri::command]
pub async fn find_credential(
    state: State<'_, AppState>,
    name: String,
) -> Result<Option<Credential>, IpcError> {
    Ok(merged_credentials(&state)
        .await?
        .into_iter()
        .find(|credential| credential.name.eq_ignore_ascii_case(name.trim())))
}

#[tauri::command]
pub async fn catalog_counts(state: State<'_, AppState>) -> Result<(i64, i64), IpcError> {
    let repository = state.repository()?;
    let local_servers = repository.server_count().await?;
    let local_credentials = repository.credential_count().await?;
    let extra_servers = state.extra_servers().await.len() as i64;
    let extra_credentials = state.extra_credentials().await.len() as i64;
    Ok((
        local_servers + extra_servers,
        local_credentials + extra_credentials,
    ))
}

#[tauri::command]
pub async fn catalog_snapshot(state: State<'_, AppState>) -> Result<CatalogSnapshot, IpcError> {
    let repository = state.repository()?;
    let servers = merged_servers(&state).await?;
    let folders = repository.list_folders().await?;
    let credentials = merged_credentials(&state).await?;
    let tags_locality = state.tags.get();
    let tags = collect_tags(&servers, &tags_locality.pinned, &tags_locality.order);
    Ok(CatalogSnapshot {
        servers,
        folders,
        credentials,
        protocols: hr_core::all_protocols(),
        tags,
        connection_records: state.connection_records.get(),
    })
}

#[tauri::command]
pub async fn catalog_view(
    state: State<'_, AppState>,
    query: String,
    providers: Vec<String>,
    sort: String,
    custom_order: Vec<String>,
    folder_order: Vec<String>,
    source: Option<String>,
) -> Result<CatalogView, IpcError> {
    let repository = state.repository()?;
    let servers = merged_servers(&state).await?;
    let folders = repository.list_folders().await?;
    let source = source.unwrap_or_default();
    let servers: Vec<Server> = if source.trim().is_empty() || source == "all" {
        servers
    } else {
        servers
            .into_iter()
            .filter(|server| server.base().source_id == source)
            .collect()
    };
    let parsed_query = hr_domain::SearchQuery::parse(&query);
    let providers: Vec<MatchProvider> = if providers.is_empty() {
        state
            .config_snapshot()
            .keyword_match
            .effective_matchers(&state.i18n.locale())
            .iter()
            .map(|matcher| match matcher {
                hr_config::MatcherKind::Direct => MatchProvider::Direct,
                hr_config::MatcherKind::Initials => MatchProvider::Initials,
                hr_config::MatcherKind::Discrete => MatchProvider::Discrete,
                hr_config::MatcherKind::Pinyin => MatchProvider::Pinyin,
                hr_config::MatcherKind::PinyinInitials => MatchProvider::PinyinInitials,
            })
            .collect()
    } else {
        providers
            .iter()
            .filter_map(|provider| MatchProvider::parse(provider))
            .collect()
    };
    let hits = {
        let revision = state
            .catalog_revision
            .load(std::sync::atomic::Ordering::Relaxed);
        let key = match_cache_key(&query, &providers, &source, revision);
        let cached = state
            .match_cache
            .lock()
            .expect("match cache lock poisoned")
            .get(key, Instant::now())
            .map(|entry| entry.hits.clone());
        match cached {
            Some(hits) => hits,
            None => {
                let hits = if servers.len() > 300 && state.config_snapshot().keyword_match.parallel
                {
                    // Large catalogs match off the async worker (P10-08).
                    let servers = servers.clone();
                    let parsed_query = parsed_query.clone();
                    let providers = providers.clone();
                    tokio::task::spawn_blocking(move || search(&servers, &parsed_query, &providers))
                        .await
                        .map_err(|error| {
                            AppError::internal(format!("search task failed: {error}"))
                        })?
                } else {
                    search(&servers, &parsed_query, &providers)
                };
                state
                    .match_cache
                    .lock()
                    .expect("match cache lock poisoned")
                    .insert(key, hits.clone(), Instant::now());
                hits
            }
        }
    };
    let hit_ids: HashSet<&str> = hits.iter().map(|hit| hit.id.as_str()).collect();
    let filtered: Vec<Server> = servers
        .into_iter()
        .filter(|server| hit_ids.contains(server.base().id.as_str()))
        .collect();
    let tree = build_catalog_tree(
        &filtered,
        &folders,
        parse_catalog_sort(&sort),
        &custom_order,
        &folder_order,
    );
    let tags_locality = state.tags.get();
    let tags = collect_tags(&filtered, &tags_locality.pinned, &tags_locality.order);
    Ok(CatalogView {
        tree,
        hits,
        tags,
        total: filtered.len(),
    })
}

/// Local source descriptor plus its health. Additional sources are listed by
/// `data_source_statuses` (P8-06).
async fn local_source_status(state: &AppState) -> SourceStatus {
    let path = state
        .config_snapshot()
        .database_path(&state.paths.database_file);
    let mut status = SourceStatus {
        id: LOCAL_SOURCE_ID.to_string(),
        name: "Local".to_string(),
        kind: "sqlite".to_string(),
        state: "otherError".to_string(),
        writable: true,
        server_count: 0,
        credential_count: 0,
        error: None,
        last_checked: unix_seconds(),
    };
    match state.repository() {
        Ok(repository) => match repository.server_count().await {
            Ok(servers) => match repository.credential_count().await {
                Ok(credentials) => {
                    status.state = "ok".to_string();
                    status.server_count = servers as usize;
                    status.credential_count = credentials as usize;
                }
                Err(error) => status.error = Some(error.to_string()),
            },
            Err(error) => status.error = Some(error.to_string()),
        },
        Err(error) => status.error = Some(error.to_string()),
    }
    if let Some(error) = &status.error {
        tracing::debug!(%error, path = %path.display(), "local source is unavailable");
    }
    status
}

async fn source_statuses(state: &AppState) -> Vec<SourceStatus> {
    let mut statuses = vec![local_source_status(state).await];
    if let Some(sources) = &state.sources {
        let mut extra = sources.statuses().await;
        extra.sort_by(|left, right| left.name.cmp(&right.name));
        statuses.append(&mut extra);
    }
    statuses
}

#[tauri::command]
pub async fn data_source_list(
    state: State<'_, AppState>,
) -> Result<Vec<DataSourceConfig>, IpcError> {
    Ok(state.sources()?.configs().await)
}

#[tauri::command]
pub async fn data_source_statuses(
    state: State<'_, AppState>,
) -> Result<Vec<SourceStatus>, IpcError> {
    Ok(source_statuses(&state).await)
}

#[tauri::command]
pub async fn data_source_save(
    state: State<'_, AppState>,
    config: DataSourceConfig,
    password: Option<String>,
) -> Result<DataSourceConfig, IpcError> {
    if config.id == LOCAL_SOURCE_ID {
        return Err(AppError::validation(
            "the local source is configured by the database path, not as an extra source",
        )
        .into());
    }
    Ok(state.sources()?.save(config, password).await?)
}

#[tauri::command]
pub async fn data_source_delete(state: State<'_, AppState>, id: String) -> Result<bool, IpcError> {
    if id == LOCAL_SOURCE_ID {
        return Err(AppError::permission_denied("the local source cannot be deleted").into());
    }
    Ok(state.sources()?.delete(&id).await?)
}

#[tauri::command]
pub async fn data_source_test(
    state: State<'_, AppState>,
    config: DataSourceConfig,
    password: Option<String>,
) -> Result<SourceStatus, IpcError> {
    Ok(state.sources()?.test(&config, password.as_deref()).await?)
}

#[tauri::command]
pub async fn data_source_refresh(
    state: State<'_, AppState>,
    id: String,
) -> Result<SourceStatus, IpcError> {
    if id == LOCAL_SOURCE_ID {
        return Ok(local_source_status(&state).await);
    }
    let status = state.sources()?.refresh(&id).await?;
    state.bump_catalog_revision();
    Ok(status)
}

#[tauri::command]
pub async fn data_source_refresh_all(
    state: State<'_, AppState>,
) -> Result<Vec<SourceStatus>, IpcError> {
    if let Some(sources) = &state.sources {
        sources.refresh_all().await;
        state.bump_catalog_revision();
    }
    Ok(source_statuses(&state).await)
}

/// Points the local catalog at another SQLite file (P8-01/P8-08). The new
/// database and repository are opened before the old pool is closed, so a bad
/// path or a foreign key leaves the running catalog untouched.
///
/// Synchronous on purpose: SQLx's executor futures are not provably `Send` for
/// every lifetime, which the async command macro requires. The work is a single
/// quick open (like `import_database_file`), so blocking the IPC thread is fine.
#[tauri::command]
pub fn change_local_database_path(
    state: State<'_, AppState>,
    path: Option<String>,
) -> Result<DatabaseInfo, IpcError> {
    let state = state.inner();
    tauri::async_runtime::block_on(async {
        let configured = path
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty());
        let resolved = match &configured {
            Some(path) => PathBuf::from(path),
            None => state.paths.database_file.clone(),
        };
        if resolved.is_dir() {
            return Err(AppError::validation("the database path must point to a file").into());
        }
        if let Some(parent) = resolved.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                AppError::permission_denied(format!("cannot create {}: {error}", parent.display()))
            })?;
        }
        let vault = state
            .vault
            .read()
            .expect("vault lock poisoned")
            .clone()
            .ok_or_else(|| AppError::unavailable("encrypted storage is unavailable"))?;
        let database = Database::open_sqlite(&resolved).await.map_err(|error| {
            AppError::permission_denied(format!(
                "cannot open {} for writing: {error}",
                resolved.display()
            ))
        })?;
        let codec = std::sync::Arc::new(hr_storage::VaultCodec::new(vault));
        let repository = match hr_storage::Repository::open(database.pool().clone(), codec).await {
            Ok(repository) => std::sync::Arc::new(repository),
            Err(error) => {
                database.close().await;
                return Err(error.into());
            }
        };
        state.config.update(|config| {
            config.sqlite_database_path = configured.clone();
        });
        state.config.flush()?;
        let previous = {
            let mut guard = state.db.lock().await;
            guard.replace(database)
        };
        let previous_repository = state
            .repository
            .lock()
            .expect("repository lock poisoned")
            .replace(repository);
        *state
            .repository_error
            .lock()
            .expect("repository error lock poisoned") = None;
        drop(previous_repository);
        if let Some(previous) = previous {
            previous.close().await;
        }
        tracing::info!(path = %resolved.display(), "local database path changed");
        state.bump_catalog_revision();
        Ok(database_info(state).await)
    })
}

fn match_cache_key(query: &str, providers: &[MatchProvider], source: &str, revision: u64) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    query.hash(&mut hasher);
    providers.hash(&mut hasher);
    source.hash(&mut hasher);
    revision.hash(&mut hasher);
    hasher.finish()
}

fn parse_catalog_sort(value: &str) -> CatalogSort {
    match value.trim().to_ascii_lowercase().as_str() {
        "protocol" => CatalogSort::Protocol,
        "name" => CatalogSort::Name,
        "address" => CatalogSort::Address,
        "custom" => CatalogSort::Custom,
        _ => CatalogSort::Insertion,
    }
}

#[tauri::command]
pub async fn list_folders(state: State<'_, AppState>) -> Result<Vec<Folder>, IpcError> {
    Ok(state.repository()?.list_folders().await?)
}

#[tauri::command]
pub async fn create_folder(
    state: State<'_, AppState>,
    path: Vec<String>,
) -> Result<Folder, IpcError> {
    Ok(state.repository()?.create_folder(path).await?)
}

#[tauri::command]
pub async fn ensure_folder(
    state: State<'_, AppState>,
    path: Vec<String>,
) -> Result<Folder, IpcError> {
    Ok(state.repository()?.ensure_folder_path(path).await?)
}

#[tauri::command]
pub async fn rename_folder(
    state: State<'_, AppState>,
    folder_id: String,
    new_name: String,
) -> Result<usize, IpcError> {
    Ok(state
        .repository()?
        .rename_folder(&folder_id, &new_name)
        .await?)
}

#[tauri::command]
pub async fn delete_folder(
    state: State<'_, AppState>,
    path: Vec<String>,
) -> Result<(usize, usize), IpcError> {
    Ok(state.repository()?.delete_folder(path).await?)
}

#[tauri::command]
pub async fn move_folder(
    state: State<'_, AppState>,
    folder_id: String,
    parent: Vec<String>,
) -> Result<usize, IpcError> {
    Ok(state.repository()?.move_folder(&folder_id, parent).await?)
}

#[tauri::command]
pub async fn duplicate_folder(
    state: State<'_, AppState>,
    folder_id: String,
) -> Result<Folder, IpcError> {
    Ok(state.repository()?.duplicate_folder(&folder_id).await?)
}

#[tauri::command]
pub async fn move_server_to_folder(
    state: State<'_, AppState>,
    server_id: String,
    path: Vec<String>,
) -> Result<Server, IpcError> {
    let repository = state.repository()?;
    let mut server = state
        .find_server(&server_id)
        .await?
        .ok_or_else(|| AppError::not_found("server"))?;
    ensure_writable_server(&server)?;
    server.base_mut().tree_nodes = path;
    let saved = repository.upsert_server(server).await?;
    state.bump_catalog_revision();
    Ok(saved)
}

#[tauri::command]
pub fn get_list_view(state: State<'_, AppState>) -> ListViewLocality {
    state.list_view.get()
}

#[tauri::command]
pub fn update_list_view(
    state: State<'_, AppState>,
    view: ListViewLocality,
) -> Result<ListViewLocality, IpcError> {
    let updated = state.list_view.replace(view);
    state.list_view.flush()?;
    Ok(updated)
}

#[tauri::command]
pub fn get_tree_view(state: State<'_, AppState>) -> TreeViewLocality {
    state.tree_view.get()
}

#[tauri::command]
pub fn update_tree_view(
    state: State<'_, AppState>,
    view: TreeViewLocality,
) -> Result<TreeViewLocality, IpcError> {
    let updated = state.tree_view.replace(view);
    state.tree_view.flush()?;
    Ok(updated)
}

#[tauri::command]
pub fn set_custom_order(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<ListViewLocality, IpcError> {
    let updated = state.list_view.update(|view| view.custom_order = ids);
    state.list_view.flush()?;
    Ok(updated)
}

#[tauri::command]
pub async fn export_servers_json(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<String, IpcError> {
    let servers = merged_servers(&state).await?;
    if ids.is_empty() {
        return Ok(export_json(&servers)?);
    }
    let wanted: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let selected: Vec<Server> = servers
        .into_iter()
        .filter(|server| wanted.contains(server.base().id.as_str()))
        .collect();
    Ok(export_json(&selected)?)
}

#[tauri::command]
pub async fn export_rdp_server(
    state: State<'_, AppState>,
    id: String,
) -> Result<RdpExport, IpcError> {
    let server = state
        .find_server(&id)
        .await?
        .ok_or_else(|| AppError::not_found("server"))?;
    let password = server
        .credentials()
        .map(|credentials| credentials.password.expose().to_string())
        .filter(|password| !password.is_empty());
    let (blob, password_protected, warning) = match password {
        Some(password) => match hr_platform::rdp_password_blob(&password) {
            Ok(blob) => (Some(blob), true, None),
            Err(_) => (
                None,
                false,
                Some(
                    "The password was omitted because DPAPI is unavailable on this platform."
                        .to_string(),
                ),
            ),
        },
        None => (None, true, None),
    };
    Ok(RdpExport {
        content: export_rdp(&server, blob.as_deref())?,
        password_protected,
        warning,
    })
}

#[tauri::command]
pub async fn import_json_content(
    state: State<'_, AppState>,
    content: String,
    target: Option<String>,
) -> Result<ImportOutcome, IpcError> {
    ensure_local_target(target.as_deref())?;
    let repository = state.repository()?;
    let result = import_json(&content);
    persist_import_with_state(&state, &repository, result).await
}

#[tauri::command]
pub async fn import_csv_content(
    state: State<'_, AppState>,
    content: String,
    target: Option<String>,
) -> Result<ImportOutcome, IpcError> {
    ensure_local_target(target.as_deref())?;
    let repository = state.repository()?;
    let result = import_mremoteng_csv(&content);
    persist_import_with_state(&state, &repository, result).await
}

#[tauri::command]
pub async fn import_rdp_content(
    state: State<'_, AppState>,
    content: String,
    display_name: String,
    password: Option<String>,
    target: Option<String>,
) -> Result<ImportOutcome, IpcError> {
    ensure_local_target(target.as_deref())?;
    let repository = state.repository()?;
    let result = import_rdp(&content, &display_name, password);
    persist_import_with_state(&state, &repository, result).await
}

#[tauri::command]
pub fn import_database_file(
    state: State<'_, AppState>,
    path: String,
    target: Option<String>,
) -> Result<ImportOutcome, IpcError> {
    ensure_local_target(target.as_deref())?;
    let repository = state.repository()?;
    let path = PathBuf::from(path);
    let result = tauri::async_runtime::block_on(hr_storage::import::import_database(&path))?;
    tauri::async_runtime::block_on(persist_import_with_state(&state, &repository, result))
}

async fn persist_import_with_state(
    state: &AppState,
    repository: &hr_storage::Repository,
    result: hr_domain::ImportResult,
) -> Result<ImportOutcome, IpcError> {
    let outcome = persist_import(repository, result).await?;
    if outcome.imported > 0 {
        state.bump_catalog_revision();
    }
    Ok(outcome)
}

fn ensure_local_target(target: Option<&str>) -> AppResult<()> {
    if target.is_none_or(|target| target.is_empty() || target.eq_ignore_ascii_case("local")) {
        Ok(())
    } else {
        Err(AppError::unsupported(
            "additional data sources are read-only; imports must target the local source",
        ))
    }
}

async fn persist_import(
    repository: &hr_storage::Repository,
    mut result: hr_domain::ImportResult,
) -> Result<ImportOutcome, IpcError> {
    let imported = result.servers.len();
    if imported > 0 {
        // Imports always land in the writable local catalog, even when the
        // exported payload carried a `sourceId` from a read-only source (P8-11).
        for server in &mut result.servers {
            server.base_mut().source_id = LOCAL_SOURCE_ID.to_string();
        }
        repository.upsert_servers(result.servers).await?;
    }
    Ok(ImportOutcome {
        imported,
        warnings: result.warnings,
    })
}

#[tauri::command]
pub async fn create_shortcut_for_servers(
    _app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<String>,
    name: String,
    start_minimized: bool,
) -> Result<ShortcutOutcome, IpcError> {
    let all = merged_servers(&state).await?;
    let selected: Vec<Server> = all
        .into_iter()
        .filter(|server| ids.contains(&server.base().id))
        .collect();
    if selected.is_empty() {
        return Err(AppError::validation("select at least one server").into());
    }
    let arguments = selected
        .iter()
        .map(|server| format!("ULID:{}", server.base().id))
        .chain(start_minimized.then(|| "--start-minimized".to_string()))
        .collect::<Vec<_>>();
    let icon = merged_shortcut_icon(&state, &selected)?;
    Ok(write_shortcut(&state, &name, arguments, icon)?)
}

#[tauri::command]
pub async fn create_shortcut_for_tag(
    _app: AppHandle,
    state: State<'_, AppState>,
    tag: String,
    name: String,
    start_minimized: bool,
) -> Result<ShortcutOutcome, IpcError> {
    let tag = hr_domain::normalize_tag(&tag).ok_or_else(|| AppError::validation("invalid tag"))?;
    let all = merged_servers(&state).await?;
    let selected: Vec<Server> = all
        .iter()
        .filter(|server| server.base().tags.contains(&tag))
        .cloned()
        .collect();
    if selected.is_empty() {
        return Err(AppError::validation("the tag has no servers").into());
    }
    let mut arguments = vec![format!("#{tag}")];
    if start_minimized {
        arguments.push("--start-minimized".to_string());
    }
    let icon = merged_shortcut_icon(&state, &selected)?;
    Ok(write_shortcut(&state, &name, arguments, icon)?)
}

fn merged_shortcut_icon(state: &AppState, servers: &[Server]) -> AppResult<Option<PathBuf>> {
    let icons: Vec<hr_domain::IconData> = servers
        .iter()
        .filter_map(|server| server.base().icon_base64.as_deref())
        .filter_map(|base64| hr_domain::IconData::decode(base64).ok())
        .take(4)
        .collect();
    if icons.is_empty() {
        return Ok(None);
    }
    let bytes = hr_domain::merge_icons_ico(&icons, 64)?;
    let path = state
        .paths
        .icons_dir
        .join(format!("shortcut-{}.ico", hr_core::new_id()));
    std::fs::write(&path, bytes)?;
    Ok(Some(path))
}

fn write_shortcut(
    state: &AppState,
    name: &str,
    arguments: Vec<String>,
    icon: Option<PathBuf>,
) -> AppResult<ShortcutOutcome> {
    let executable = std::env::current_exe()
        .map_err(|error| AppError::platform(format!("cannot locate executable: {error}")))?;
    let mut spec = ShortcutSpec::new(name, executable.clone())
        .with_arguments(arguments)
        .with_description("H-Remote session shortcut");
    if let Some(parent) = executable.parent() {
        spec = spec.with_working_dir(parent.to_path_buf());
    }
    if let Some(icon) = &icon {
        spec = spec.with_icon(icon.clone());
    }
    let desktop = hr_platform::default_desktop_dir()?;
    let path = hr_platform::write_desktop_shortcut(&spec, &desktop)?;
    let _ = state;
    Ok(ShortcutOutcome {
        path: path.display().to_string(),
        icon_path: icon.map(|path| path.display().to_string()),
    })
}

#[tauri::command]
pub fn save_icon_to_library(
    state: State<'_, AppState>,
    base64_value: String,
) -> Result<String, IpcError> {
    let icon = hr_domain::IconData::decode(&base64_value)?;
    let image = image::RgbaImage::from_raw(icon.width, icon.height, icon.rgba.as_ref().clone())
        .ok_or_else(|| AppError::validation("invalid icon buffer"))?;
    std::fs::create_dir_all(&state.paths.icon_library_dir)?;
    let path = state
        .paths
        .icon_library_dir
        .join(format!("{}.png", hr_core::new_id()));
    image
        .save(&path)
        .map_err(|error| AppError::internal(format!("cannot save icon: {error}")))?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn list_icon_library(state: State<'_, AppState>) -> Result<Vec<IconLibraryEntry>, IpcError> {
    let mut entries = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(&state.paths.icon_library_dir) {
        let mut paths: Vec<PathBuf> = read_dir
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect();
        paths.sort();
        for path in paths.into_iter().rev().take(64) {
            let bytes = std::fs::read(&path)?;
            if bytes.len() > 512 * 1024 {
                continue;
            }
            entries.push(IconLibraryEntry {
                name: path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("icon")
                    .to_string(),
                path: path.display().to_string(),
                base64: STANDARD.encode(bytes),
            });
        }
    }
    Ok(entries)
}

#[tauri::command]
pub fn delete_icon_from_library(
    state: State<'_, AppState>,
    path: String,
) -> Result<bool, IpcError> {
    let path = PathBuf::from(path);
    if path.parent() != Some(state.paths.icon_library_dir.as_path()) {
        return Err(AppError::validation("invalid icon path").into());
    }
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(AppError::from(error).into()),
    }
}

#[tauri::command]
pub async fn fetch_icon_from_url(url: String) -> Result<String, IpcError> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(AppError::validation("icon URL must use http(s)").into());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|error| AppError::platform(format!("cannot create HTTP client: {error}")))?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|error| AppError::platform(format!("cannot fetch icon: {error}")))?;
    if !response.status().is_success() {
        return Err(AppError::platform(format!(
            "icon download failed with status {}",
            response.status()
        ))
        .into());
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| AppError::platform(format!("cannot read icon: {error}")))?;
    if bytes.len() > 512 * 1024 {
        return Err(AppError::validation("icon is larger than 512 KiB").into());
    }
    hr_domain::IconData::decode(&STANDARD.encode(&bytes))?;
    let mime = match image::guess_format(&bytes) {
        Ok(image::ImageFormat::Jpeg) => "image/jpeg",
        _ => "image/png",
    };
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

#[tauri::command]
pub async fn test_script(
    state: State<'_, AppState>,
    script: String,
    hide_window: bool,
    server_id: Option<String>,
) -> Result<hr_platform::ScriptResult, IpcError> {
    let environment = match server_id.filter(|id| !id.trim().is_empty()) {
        Some(id) => match state.find_server(&id).await? {
            Some(server) => session_scripts::environment(&server, "test-script"),
            None => Default::default(),
        },
        None => Default::default(),
    };
    Ok(tauri::async_runtime::spawn_blocking(move || {
        hr_platform::run_script_with_env(&script, hide_window, &environment)
    })
    .await
    .map_err(|error| AppError::internal(format!("script task failed: {error}")))??)
}

async fn database_info(state: &AppState) -> DatabaseInfo {
    let repository = state
        .repository
        .lock()
        .expect("repository lock poisoned")
        .clone();
    let repository_error = state
        .repository_error
        .lock()
        .expect("repository error lock poisoned")
        .clone();
    let g = state.db.lock().await;
    let path = state
        .config_snapshot()
        .database_path(&state.paths.database_file)
        .display()
        .to_string();
    if let Some(database) = g.as_ref() {
        let writable = database.is_writable().await.ok();
        let schema_version = database.schema_version().await.ok();
        let (encryption_verified, server_count, credential_count, repository_error) =
            match repository {
                Some(repository) => (
                    repository.encryption_verified().await.ok(),
                    repository.server_count().await.ok(),
                    repository.credential_count().await.ok(),
                    None,
                ),
                None => (None, None, None, repository_error),
            };
        return DatabaseInfo {
            available: true,
            path,
            writable,
            schema_version,
            encryption_verified,
            server_count,
            credential_count,
            error: repository_error,
        };
    }
    DatabaseInfo {
        available: false,
        path,
        writable: None,
        schema_version: None,
        encryption_verified: None,
        server_count: None,
        credential_count: None,
        error: state
            .db_error
            .lock()
            .expect("database error lock poisoned")
            .clone(),
    }
}

fn read_tail(path: &Path, lines: usize, max_bytes: usize) -> AppResult<String> {
    let bytes = std::fs::read(path)?;
    let start = bytes.len().saturating_sub(max_bytes);
    let text = String::from_utf8_lossy(&bytes[start..]).to_string();
    if lines == usize::MAX {
        return Ok(text);
    }
    let collected: Vec<&str> = text.lines().rev().take(lines).collect();
    Ok(collected.into_iter().rev().collect::<Vec<_>>().join("\n"))
}

fn unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn file_session_snapshot(info: &hr_files::FileSessionInfo) -> SessionSnapshot {
    let now = unix_millis() as i64;
    SessionSnapshot {
        id: info.id.clone(),
        server_id: info.server_id.clone(),
        server_name: info.server_name.clone(),
        backend: SessionBackend::FileTransfer,
        state: SessionState::Connected,
        reason: None,
        message: None,
        attempt: 1,
        max_attempts: 1,
        width: 0,
        height: 0,
        frame_count: 0,
        started_at: now,
        connected_at: Some(now),
        ended_at: None,
        frame_ring: None,
    }
}

async fn start_file_session(state: &AppState, server: &Server) -> AppResult<SessionSnapshot> {
    let request = match server {
        Server::Sftp(sftp) => hr_files::FileConnectRequest {
            server_id: sftp.base.id.clone(),
            server_name: sftp.base.display_name.clone(),
            protocol: "SFTP".to_string(),
            address: sftp.address.address.clone(),
            port: sftp.address.port,
            username: sftp.credentials.username.clone(),
            password: sftp.credentials.password.expose().to_string(),
            private_key: sftp.credentials.private_key.clone(),
            startup_path: sftp.startup_path.clone(),
        },
        Server::Ftp(ftp) => hr_files::FileConnectRequest {
            server_id: ftp.base.id.clone(),
            server_name: ftp.base.display_name.clone(),
            protocol: "FTP".to_string(),
            address: ftp.address.address.clone(),
            port: ftp.address.port,
            username: ftp.credentials.username.clone(),
            password: ftp.credentials.password.expose().to_string(),
            private_key: None,
            startup_path: ftp.startup_path.clone(),
        },
        other => {
            return Err(AppError::unsupported(format!(
                "{} is not a file-transfer protocol",
                other.protocol().id()
            )));
        }
    };
    let info = state.files.connect(request).await?;
    Ok(file_session_snapshot(&info))
}

fn apply_alternate(server: &mut Server, name: &str) -> AppResult<()> {
    let alternate = server
        .address()
        .and_then(|address| {
            address
                .alternate_credentials
                .iter()
                .find(|candidate| candidate.name.eq_ignore_ascii_case(name))
        })
        .cloned()
        .ok_or_else(|| AppError::not_found("alternate credential"))?;
    if let Some(address) = server.address_mut() {
        if let Some(host) = alternate.address.filter(|host| !host.trim().is_empty()) {
            address.address = host;
        }
        if let Some(port) = alternate.port.filter(|port| *port != 0) {
            address.port = port;
        }
    }
    if let Some(credentials) = server.credentials_mut() {
        if !alternate.username.trim().is_empty() {
            credentials.username = alternate.username;
        }
        if !alternate.password.is_empty() {
            credentials.password = alternate.password;
            credentials.private_key = None;
        } else if let Some(key) = alternate.private_key.filter(|key| !key.is_empty()) {
            credentials.private_key = Some(key);
            credentials.password = "".into();
        }
    }
    Ok(())
}

async fn probe(host: &str, port: u16) -> Option<u128> {
    // Quick 100 ms test first so a healthy address does not flash a dialog.
    if let Some(latency) =
        hr_platform::tcp_latency(host, port, std::time::Duration::from_millis(100)).await
    {
        return Some(latency);
    }
    hr_platform::tcp_latency(host, port, std::time::Duration::from_secs(2)).await
}

/// Picks the first reachable alternate when the main address is down.
async fn auto_switch_address(server: &mut Server) {
    if let Server::Rdp(rdp) = server
        && rdp.gateway_mode != hr_domain::GatewayMode::DoNotUseGateway
    {
        return;
    }
    let Some(address) = server.address() else {
        return;
    };
    if !address.is_auto_alternate_address_switching {
        return;
    }
    if probe(&address.address, address.port).await.is_some() {
        return;
    }
    let alternates = address.alternate_credentials.clone();
    for alternate in alternates {
        let host = alternate
            .address
            .clone()
            .filter(|host| !host.trim().is_empty())
            .unwrap_or_else(|| address.address.clone());
        let port = alternate
            .port
            .filter(|port| *port != 0)
            .unwrap_or(address.port);
        if probe(&host, port).await.is_some() {
            if let Err(error) = apply_alternate(server, &alternate.name) {
                tracing::warn!(%error, "cannot apply alternate address");
            }
            return;
        }
    }
}

#[tauri::command]
pub async fn check_server_availability(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<AvailabilityEntry>, IpcError> {
    let server = state
        .find_server(&id)
        .await?
        .ok_or_else(|| AppError::not_found("server"))?;
    let Some(address) = server.address() else {
        return Ok(Vec::new());
    };
    let mut targets = vec![("Main".to_string(), address.address.clone(), address.port)];
    for alternate in &address.alternate_credentials {
        targets.push((
            alternate.name.clone(),
            alternate
                .address
                .clone()
                .filter(|host| !host.trim().is_empty())
                .unwrap_or_else(|| address.address.clone()),
            alternate
                .port
                .filter(|port| *port != 0)
                .unwrap_or(address.port),
        ));
    }
    let mut result = Vec::with_capacity(targets.len());
    for (name, host, port) in targets {
        let latency = probe(&host, port).await;
        result.push(AvailabilityEntry {
            name,
            address: host,
            port,
            reachable: latency.is_some(),
            latency_ms: latency,
        });
    }
    Ok(result)
}

#[tauri::command]
pub async fn connect_server(
    state: State<'_, AppState>,
    id: String,
    viewport: Option<(u32, u32)>,
    device_scale: Option<u16>,
    username: Option<String>,
    password: Option<String>,
    private_key: Option<String>,
    alternate_name: Option<String>,
) -> Result<SessionSnapshot, IpcError> {
    let mut server = state
        .find_server(&id)
        .await?
        .ok_or_else(|| AppError::not_found("server"))?;
    // Resolve a named vault credential before applying per-connection overrides
    // so inherited username/password/key actually reach the protocol plans.
    if let Some(credentials) = server.credentials() {
        if credentials.inherited_credential_name.is_some() {
            let catalog = merged_credentials(&state).await?;
            if let Some(effective) = server.effective_credentials(&catalog)
                && let Some(target) = server.credentials_mut()
            {
                target.username = effective.username;
                target.password = effective.password;
                target.private_key = effective.private_key;
            }
        }
    }
    if let Some(name) = alternate_name.filter(|name| !name.trim().is_empty()) {
        apply_alternate(&mut server, &name)?;
    }
    if let Some(username) = username {
        if let Some(credentials) = server.credentials_mut() {
            credentials.username = username;
        }
    }
    if let Some(password) = password {
        if let Some(credentials) = server.credentials_mut() {
            credentials.password = password.into();
            credentials.private_key = None;
        }
    }
    if let Some(key) = private_key.filter(|key| !key.trim().is_empty()) {
        if let Some(credentials) = server.credentials_mut() {
            credentials.private_key = Some(key);
            credentials.password = "".into();
        }
    }
    auto_switch_address(&mut server).await;
    let scripts = session_scripts::supports(&server);
    let requested_session_id = hr_core::new_id();
    if scripts {
        session_scripts::ensure_pre_connect(&server, &requested_session_id).await?;
    }
    let snapshot = match &server {
        Server::Rdp(_) => {
            let plan = RdpPlan::from_server(
                &server,
                viewport.unwrap_or((1920, 1080)),
                device_scale.unwrap_or(100),
                ReconnectPolicy::default(),
            )?;
            state.sessions.start_plan_with_id(
                hr_session::SessionPlan::Rdp(plan),
                Some(requested_session_id.clone()),
            )?
        }
        Server::Vnc(_) => state.sessions.start_plan_with_id(
            hr_session::SessionPlan::Vnc(VncPlan::from_server(&server)?),
            Some(requested_session_id.clone()),
        )?,
        Server::Sftp(_) | Server::Ftp(_) => start_file_session(&state, &server).await?,
        Server::RemoteApp(_)
        | Server::LocalApp(_)
        | Server::Ssh(_)
        | Server::Telnet(_)
        | Server::Serial(_)
        | Server::Custom(_) => {
            let plan = ProcessPlan::from_server(&server, &state.config_snapshot().runners)?;
            state.sessions.start_plan_with_id(
                hr_session::SessionPlan::Process(plan),
                Some(requested_session_id.clone()),
            )?
        }
        other => {
            return Err(AppError::unsupported(format!(
                "{} sessions are not available in this build",
                other.protocol().id()
            ))
            .into());
        }
    };
    if scripts {
        session_scripts::spawn_post_disconnect(
            std::sync::Arc::clone(&state.sessions),
            server.clone(),
            snapshot.id.clone(),
        );
    }
    let now = unix_seconds();
    state
        .connection_records
        .update(|records| records.record_connection(&id, now));
    state.connection_records.flush()?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn session_list(state: State<'_, AppState>) -> Result<Vec<SessionSnapshot>, IpcError> {
    let mut sessions = state.sessions.list();
    sessions.extend(state.files.infos().await.iter().map(file_session_snapshot));
    Ok(sessions)
}

#[tauri::command]
pub async fn session_snapshot(
    state: State<'_, AppState>,
    id: String,
) -> Result<SessionSnapshot, IpcError> {
    if let Ok(snapshot) = state.sessions.snapshot(&id) {
        return Ok(snapshot);
    }
    Ok(file_session_snapshot(&state.files.info(&id).await?))
}

#[tauri::command]
pub async fn session_cancel(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    if state.sessions.snapshot(&id).is_ok() {
        state.sessions.cancel(&id)?;
    } else {
        state.files.close(&id).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn session_reconnect(
    state: State<'_, AppState>,
    id: String,
) -> Result<SessionSnapshot, IpcError> {
    if state.sessions.snapshot(&id).is_ok() {
        return Ok(state.sessions.reconnect(&id)?);
    }
    let info = state.files.info(&id).await?;
    state.files.close(&id).await?;
    let server = state
        .find_server(&info.server_id)
        .await?
        .ok_or_else(|| AppError::not_found("server"))?;
    start_file_session(&state, &server)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn session_remove(state: State<'_, AppState>, id: String) -> Result<bool, IpcError> {
    if state.sessions.snapshot(&id).is_ok() {
        return Ok(state.sessions.remove(&id)?);
    }
    state.files.close(&id).await?;
    Ok(true)
}

#[tauri::command]
pub fn session_resize(
    state: State<'_, AppState>,
    id: String,
    width: u32,
    height: u32,
    scale_percent: Option<u16>,
) -> Result<(), IpcError> {
    Ok(state
        .sessions
        .resize(&id, width, height, scale_percent.unwrap_or(100))?)
}

#[tauri::command]
pub fn session_send_ctrl_alt_del(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    Ok(state.sessions.send_input(&id, SessionCommand::CtrlAltDel)?)
}

#[tauri::command]
pub fn session_frame_ring(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<String>, IpcError> {
    Ok(state.sessions.frame_ring(&id)?)
}

#[tauri::command]
pub async fn session_frame_image(
    state: State<'_, AppState>,
    id: String,
    since: Option<u64>,
) -> Result<Option<SessionFrameImage>, IpcError> {
    let Some(frame) = state.sessions.frame(&id)? else {
        return Ok(None);
    };
    // The poll loop can ask again before the desktop changed; skip the PNG
    // encode entirely in that case.
    if since == Some(frame.seq) {
        return Ok(None);
    }
    // PNG encoding is CPU-heavy. Keep it off the Tauri main thread so a large
    // or fast-moving remote desktop cannot make every window unresponsive.
    let encoded = tauri::async_runtime::spawn_blocking(move || encode_frame_image(frame))
        .await
        .map_err(|error| AppError::internal(format!("cannot encode session frame: {error}")))?;
    encoded.map(Some).map_err(Into::into)
}

fn encode_frame_image(frame: Frame) -> AppResult<SessionFrameImage> {
    let seq = frame.seq;
    let width = frame.width;
    let height = frame.height;
    let image = image::RgbaImage::from_raw(width, height, frame.rgba)
        .ok_or_else(|| AppError::validation("invalid session frame buffer"))?;
    let mut png = Vec::new();
    // Default PNG settings spend tens of milliseconds per 1080p frame on
    // filtering; frame delivery is latency-sensitive, so trade a little size for
    // a much faster encode.
    let encoder = image::codecs::png::PngEncoder::new_with_quality(
        std::io::Cursor::new(&mut png),
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::Sub,
    );
    image
        .write_with_encoder(encoder)
        .map_err(|error| AppError::internal(format!("cannot encode session frame: {error}")))?;
    Ok(SessionFrameImage {
        seq,
        width,
        height,
        data_url: format!("data:image/png;base64,{}", STANDARD.encode(&png)),
    })
}

#[tauri::command]
pub fn update_tab_workspace(
    state: State<'_, AppState>,
    layout: String,
    grid_mode: bool,
    show_icons: bool,
    show_close: bool,
    show_reconnect: bool,
) -> Result<Locality, IpcError> {
    if layout.len() > 64
        || !layout
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(AppError::validation("invalid tab layout id").into());
    }
    let updated = state.locality.update(|locality| {
        locality.ui.tab_layout = layout;
        locality.ui.grid_mode = grid_mode;
        locality.ui.show_tab_icons = show_icons;
        locality.ui.show_tab_close = show_close;
        locality.ui.show_tab_reconnect = show_reconnect;
    });
    state.locality.flush()?;
    Ok(updated)
}

#[tauri::command]
pub fn open_session_window(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    placement: Option<WindowPlacement>,
) -> Result<String, IpcError> {
    state.sessions.snapshot(&session_id)?;
    Ok(window::open_session_window(&app, &session_id, placement)?)
}

#[tauri::command]
pub fn set_window_title(window: tauri::Window, title: String) -> Result<(), IpcError> {
    window
        .set_title(&title)
        .map_err(|error| AppError::platform(format!("cannot set the window title: {error}")))?;
    Ok(())
}

#[tauri::command]
pub fn focus_server_in_catalog(app: AppHandle, server_id: String) -> Result<(), IpcError> {
    if !is_valid_id(&server_id) {
        return Err(AppError::validation("invalid server id").into());
    }
    window::show_main(&app);
    let _ = app.emit("catalog:edit-server", server_id);
    Ok(())
}

#[tauri::command]
pub fn show_launcher(app: AppHandle) -> Result<(), IpcError> {
    Ok(window::show_launcher(&app)?)
}

#[tauri::command]
pub fn hide_launcher(app: AppHandle) -> Result<(), IpcError> {
    Ok(window::hide_launcher(&app)?)
}

#[tauri::command]
pub fn resize_launcher(app: AppHandle, height: u32) -> Result<(), IpcError> {
    Ok(window::resize_launcher(&app, height)?)
}

#[tauri::command]
pub fn quick_connect_history(state: State<'_, AppState>) -> Vec<QuickConnectEntry> {
    state.connection_records.get().quick_connect
}

#[tauri::command]
pub async fn quick_connect(
    state: State<'_, AppState>,
    protocol: String,
    address: String,
    port: Option<u16>,
    username: String,
    password: Option<String>,
    save: bool,
    match_credential: bool,
) -> Result<QuickConnectOutcome, IpcError> {
    let protocol = protocol.trim().to_ascii_uppercase();
    let address = address.trim();
    if address.is_empty() {
        return Err(AppError::validation("an address is required").into());
    }
    let port = port.unwrap_or(match protocol.as_str() {
        "VNC" => 5900,
        "SSH" | "SFTP" => 22,
        "TELNET" => 23,
        "FTP" => 21,
        _ => 3389,
    });
    let display_name = if username.trim().is_empty() {
        address.to_string()
    } else {
        format!("{address} ({})", username.trim())
    };
    let mut server = match protocol.as_str() {
        "RDP" => Server::new_rdp(display_name.clone(), address, port),
        "VNC" => Server::new_vnc(display_name.clone(), address, port),
        "SSH" => Server::new_ssh(display_name.clone(), address, username.trim()),
        "SFTP" => Server::new_sftp(display_name.clone(), address, username.trim()),
        "FTP" => Server::new_ftp(display_name.clone(), address, username.trim()),
        "TELNET" => Server::new_telnet(display_name.clone(), address, port),
        "SERIAL" => Server::new_serial(display_name.clone(), address),
        other => {
            return Err(AppError::unsupported(format!(
                "{other} quick connect arrives with the file-transfer engine (Phase 6b)"
            ))
            .into());
        }
    };
    if let Some(credentials) = server.credentials_mut() {
        credentials.username = username.trim().to_string();
        if let Some(password) = password.filter(|value| !value.is_empty()) {
            credentials.password = password.into();
        }
    }
    if match_credential {
        let credentials = merged_credentials(&state).await?;
        if let Some(credential) = credentials.iter().find(|credential| {
            !credential.username.is_empty()
                && credential.username.eq_ignore_ascii_case(username.trim())
        }) {
            if let Some(client) = server.credentials_mut() {
                if client.username.is_empty() {
                    client.username = credential.username.clone();
                }
                if client.password.is_empty() {
                    client.password = credential.password.clone();
                }
            }
        }
    }
    server.normalize();
    let snapshot = match &server {
        Server::Rdp(_) => {
            let plan =
                RdpPlan::from_server(&server, (1920, 1080), 100, ReconnectPolicy::default())?;
            state.sessions.start(plan)?
        }
        Server::Vnc(_) => state.sessions.start_vnc(VncPlan::from_server(&server)?)?,
        Server::Sftp(_) | Server::Ftp(_) => start_file_session(&state, &server).await?,
        Server::Ssh(_) | Server::Telnet(_) | Server::Serial(_) => {
            let plan = ProcessPlan::from_server(&server, &state.config_snapshot().runners)?;
            state.sessions.start_process(plan)?
        }
        _ => {
            return Err(AppError::unsupported(
                "this protocol is not available for quick connect yet",
            )
            .into());
        }
    };
    let now = unix_seconds();
    if save {
        state.repository()?.upsert_server(server).await?;
        state.bump_catalog_revision();
    }
    let limit = usize::from(state.config_snapshot().launcher.recent_limit).max(1);
    state.connection_records.update(|records| {
        records.record_quick_connect(
            QuickConnectEntry {
                protocol: protocol.clone(),
                address: address.to_string(),
                port,
                username: username.trim().to_string(),
                last_used_at: now,
            },
            limit,
        );
        if save {
            records.record_connection(&snapshot.server_id, now);
        }
    });
    state.connection_records.flush()?;
    Ok(QuickConnectOutcome {
        session: snapshot,
        saved: save,
    })
}

#[tauri::command]
pub fn session_send_special(
    state: State<'_, AppState>,
    id: String,
    special: String,
) -> Result<(), IpcError> {
    let key = match special.as_str() {
        "ctrlAltDel" => SpecialKey::CtrlAltDel,
        "ctrlEsc" => SpecialKey::CtrlEsc,
        "altF4" => SpecialKey::AltF4,
        _ => return Err(AppError::validation("unknown special key").into()),
    };
    Ok(state
        .sessions
        .send_input(&id, SessionCommand::Special(key))?)
}

#[tauri::command]
pub fn session_vnc_key(
    state: State<'_, AppState>,
    id: String,
    keycode: u32,
    down: bool,
) -> Result<(), IpcError> {
    Ok(state
        .sessions
        .send_input(&id, SessionCommand::VncKey { keycode, down })?)
}

#[tauri::command]
pub fn session_vnc_pointer(
    state: State<'_, AppState>,
    id: String,
    x: u16,
    y: u16,
    buttons: u8,
) -> Result<(), IpcError> {
    Ok(state
        .sessions
        .send_input(&id, SessionCommand::VncPointer { x, y, buttons })?)
}

#[tauri::command]
pub fn session_vnc_refresh(state: State<'_, AppState>, id: String) -> Result<(), IpcError> {
    Ok(state.sessions.send_input(&id, SessionCommand::VncRefresh)?)
}

#[tauri::command]
pub fn session_vnc_text(
    state: State<'_, AppState>,
    id: String,
    text: String,
) -> Result<(), IpcError> {
    if text.len() > 64 * 1024 {
        return Err(AppError::validation("clipboard text is too large").into());
    }
    Ok(state
        .sessions
        .send_input(&id, SessionCommand::VncText(text))?)
}

#[tauri::command]
pub fn session_rdp_key(
    state: State<'_, AppState>,
    id: String,
    code: String,
    key: String,
    down: bool,
) -> Result<(), IpcError> {
    Ok(state.sessions.rdp_key(&id, &code, &key, down)?)
}

#[tauri::command]
pub fn session_rdp_pointer_move(
    state: State<'_, AppState>,
    id: String,
    x: u16,
    y: u16,
) -> Result<(), IpcError> {
    Ok(state.sessions.rdp_pointer_move(&id, x, y)?)
}

#[tauri::command]
pub fn session_rdp_pointer_button(
    state: State<'_, AppState>,
    id: String,
    x: u16,
    y: u16,
    button: u8,
    down: bool,
) -> Result<(), IpcError> {
    Ok(state.sessions.rdp_pointer_button(&id, x, y, button, down)?)
}

#[tauri::command]
pub fn session_rdp_wheel(
    state: State<'_, AppState>,
    id: String,
    x: u16,
    y: u16,
    vertical: bool,
    units: i16,
) -> Result<(), IpcError> {
    Ok(state.sessions.rdp_wheel(&id, x, y, vertical, units)?)
}

#[tauri::command]
pub fn get_runners(state: State<'_, AppState>) -> hr_config::RunnersConfig {
    state.config_snapshot().runners
}

#[tauri::command]
pub fn save_runner(
    state: State<'_, AppState>,
    runner: hr_config::Runner,
) -> Result<hr_config::RunnersConfig, IpcError> {
    let updated = state.config.update(|config| {
        if let Err(error) = config.runners.upsert(runner) {
            tracing::warn!(%error, "invalid runner");
        }
        config.runners.sanitize();
    });
    state.config.flush()?;
    Ok(updated.runners)
}

#[tauri::command]
pub fn delete_runner(
    state: State<'_, AppState>,
    id: String,
) -> Result<hr_config::RunnersConfig, IpcError> {
    let updated = state.config.update(|config| {
        config.runners.remove(&id);
        config.runners.sanitize();
    });
    state.config.flush()?;
    Ok(updated.runners)
}

#[tauri::command]
pub fn set_default_runner(
    state: State<'_, AppState>,
    protocol: String,
    runner_id: String,
) -> Result<hr_config::RunnersConfig, IpcError> {
    let updated = state.config.update(|config| {
        config.runners.set_default(&protocol, &runner_id);
        config.runners.sanitize();
    });
    state.config.flush()?;
    Ok(updated.runners)
}

#[tauri::command]
pub async fn file_list(
    state: State<'_, AppState>,
    session_id: String,
    path: Option<String>,
) -> Result<hr_files::FileListing, IpcError> {
    Ok(state.files.list(&session_id, path).await?)
}

#[tauri::command]
pub async fn file_rename(
    state: State<'_, AppState>,
    session_id: String,
    from: String,
    to: String,
) -> Result<(), IpcError> {
    if to.trim().is_empty() {
        return Err(AppError::validation("the new name is required").into());
    }
    Ok(state.files.rename(&session_id, &from, &to).await?)
}

#[tauri::command]
pub async fn file_delete(
    state: State<'_, AppState>,
    session_id: String,
    path: String,
    is_dir: bool,
) -> Result<(), IpcError> {
    Ok(state.files.delete(&session_id, &path, is_dir).await?)
}

#[tauri::command]
pub async fn file_mkdir(
    state: State<'_, AppState>,
    session_id: String,
    path: String,
) -> Result<(), IpcError> {
    if path.trim().is_empty() {
        return Err(AppError::validation("the folder path is required").into());
    }
    Ok(state.files.mkdir(&session_id, &path).await?)
}

#[tauri::command]
pub async fn file_upload(
    state: State<'_, AppState>,
    session_id: String,
    local_paths: Vec<String>,
    remote_dir: String,
) -> Result<Vec<hr_files::TransferSnapshot>, IpcError> {
    Ok(state
        .files
        .start_upload(&session_id, local_paths, remote_dir)
        .await?)
}

#[tauri::command]
pub async fn file_download(
    state: State<'_, AppState>,
    session_id: String,
    remote_paths: Vec<String>,
    local_dir: String,
) -> Result<Vec<hr_files::TransferSnapshot>, IpcError> {
    Ok(state
        .files
        .start_download(&session_id, remote_paths, local_dir)
        .await?)
}

#[tauri::command]
pub async fn file_upload_dialog(
    state: State<'_, AppState>,
    session_id: String,
    remote_dir: String,
) -> Result<Vec<hr_files::TransferSnapshot>, IpcError> {
    let files = tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new().pick_files().unwrap_or_default()
    })
    .await
    .map_err(|error| AppError::internal(format!("file picker failed: {error}")))?;
    if files.is_empty() {
        return Ok(Vec::new());
    }
    Ok(state
        .files
        .start_upload(
            &session_id,
            files
                .into_iter()
                .map(|path| path.display().to_string())
                .collect(),
            remote_dir,
        )
        .await?)
}

#[tauri::command]
pub async fn file_download_dialog(
    state: State<'_, AppState>,
    session_id: String,
    remote_paths: Vec<String>,
) -> Result<Vec<hr_files::TransferSnapshot>, IpcError> {
    let folder = tauri::async_runtime::spawn_blocking(|| rfd::FileDialog::new().pick_folder())
        .await
        .map_err(|error| AppError::internal(format!("folder picker failed: {error}")))?;
    let Some(folder) = folder else {
        return Ok(Vec::new());
    };
    Ok(state
        .files
        .start_download(&session_id, remote_paths, folder.display().to_string())
        .await?)
}

#[tauri::command]
pub async fn file_transfers(
    state: State<'_, AppState>,
) -> Result<Vec<hr_files::TransferSnapshot>, IpcError> {
    Ok(state.files.transfers().await)
}

#[tauri::command]
pub async fn file_cancel_transfer(
    state: State<'_, AppState>,
    transfer_id: String,
) -> Result<bool, IpcError> {
    Ok(state.files.cancel_transfer(&transfer_id).await)
}

#[tauri::command]
pub fn verify_sensitive_action(
    state: State<'_, AppState>,
    action: String,
    force: bool,
) -> Result<hr_platform::VerificationResult, IpcError> {
    let now = std::time::Instant::now();
    {
        let verified = state
            .verified_until
            .lock()
            .expect("verification lock poisoned");
        if !force && verified.is_some_and(|until| until > now) {
            return Ok(hr_platform::VerificationResult {
                verified: true,
                supported: true,
                message: None,
            });
        }
    }
    let label = if action.trim().is_empty() {
        "Confirm your Windows account to continue."
    } else {
        action.as_str()
    };
    let result = hr_platform::verify_user(label)?;
    if result.verified {
        *state
            .verified_until
            .lock()
            .expect("verification lock poisoned") = Some(now + std::time::Duration::from_secs(30));
    }
    Ok(result)
}

#[tauri::command]
pub fn verification_status(state: State<'_, AppState>) -> i64 {
    let now = std::time::Instant::now();
    state
        .verified_until
        .lock()
        .expect("verification lock poisoned")
        .and_then(|until| {
            if until > now {
                Some(until.duration_since(now).as_secs() as i64)
            } else {
                None
            }
        })
        .unwrap_or(0)
}

#[allow(dead_code)]
fn _assert_paths_are_send_sync(_: PathBuf) {}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<UpdateStatus, IpcError> {
    let config = state.config_snapshot();
    let checked_at = unix_seconds();
    let enabled = !config.general.do_not_check_for_updates;
    let status = match config.general.update_feed_url.clone() {
        Some(feed) => {
            crate::updates::check(
                &feed,
                &state.app_info.version,
                config.general.include_prerelease_updates,
                enabled,
                checked_at,
            )
            .await
        }
        None => crate::updates::failure(
            &state.app_info.version,
            enabled,
            false,
            checked_at,
            "No update feed is configured. Set one in Settings → General.",
        ),
    };
    *state
        .update_status
        .lock()
        .expect("update status lock poisoned") = Some(status.clone());
    if state.config_snapshot().general.update_feed_url.is_some() {
        state
            .config
            .update(|config| config.engagement.last_update_check_at = Some(checked_at));
        state.config.flush()?;
    }
    if status.available {
        let _ = app.emit("update:available", &status);
    }
    Ok(status)
}

#[tauri::command]
pub fn update_check_status(state: State<'_, AppState>) -> UpdateStatus {
    let config = state.config_snapshot();
    let enabled = !config.general.do_not_check_for_updates;
    let feed_configured = config.general.update_feed_url.is_some();
    state
        .update_status
        .lock()
        .expect("update status lock poisoned")
        .clone()
        .unwrap_or_else(|| UpdateStatus::idle(&state.app_info.version, enabled, feed_configured))
}

/// Records the user's rating choice (P9-11). `never` opts out permanently.
#[tauri::command]
pub fn rating_decision(
    state: State<'_, AppState>,
    decision: String,
) -> Result<Configuration, IpcError> {
    let decision = decision.trim().to_ascii_lowercase();
    if !matches!(decision.as_str(), "rated" | "later" | "never") {
        return Err(AppError::validation("unknown rating decision").into());
    }
    let now = unix_seconds();
    let updated = state.config.update(|config| {
        config.engagement.last_rating_prompt_at = Some(now);
        if decision == "never" {
            config.engagement.do_not_show_rating = true;
        }
    });
    state.config.flush()?;
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_servers_are_writable_but_extra_sources_are_not() {
        let local = Server::new_ssh("Local", "host", "root");
        assert!(ensure_writable_server(&local).is_ok());

        let mut foreign = Server::new_ssh("Remote", "host", "root");
        foreign.base_mut().source_id = "team-db".into();
        let error = ensure_writable_server(&foreign).unwrap_err();
        assert!(error.to_string().contains("read-only"));
    }

    #[test]
    fn read_only_credentials_are_rejected() {
        let mut credential = Credential::new("shared", "root");
        assert!(ensure_writable_credential(&credential).is_ok());
        credential.is_editable = false;
        assert!(ensure_writable_credential(&credential).is_err());
    }

    #[test]
    fn match_cache_keys_change_with_every_input() {
        let providers = vec![MatchProvider::Direct];
        let base = match_cache_key("prod", &providers, "local", 1);
        assert_eq!(base, match_cache_key("prod", &providers, "local", 1));
        assert_ne!(base, match_cache_key("stage", &providers, "local", 1));
        assert_ne!(
            base,
            match_cache_key("prod", &[MatchProvider::Pinyin], "local", 1)
        );
        assert_ne!(base, match_cache_key("prod", &providers, "team", 1));
        assert_ne!(base, match_cache_key("prod", &providers, "local", 2));
    }

    #[test]
    fn local_import_target_accepts_only_the_local_source() {
        assert!(ensure_local_target(None).is_ok());
        assert!(ensure_local_target(Some("")).is_ok());
        assert!(ensure_local_target(Some("local")).is_ok());
        let error = ensure_local_target(Some("team-db")).unwrap_err();
        assert!(error.to_string().contains("read-only"));
    }

    #[test]
    fn session_frames_encode_to_a_png_data_url() {
        let frame = Frame {
            seq: 1,
            width: 2,
            height: 1,
            rgba: vec![255, 0, 0, 255, 0, 255, 0, 255],
        };
        let image = encode_frame_image(frame).unwrap();
        assert_eq!((image.seq, image.width, image.height), (1, 2, 1));
        let encoded = image
            .data_url
            .strip_prefix("data:image/png;base64,")
            .expect("png data url");
        let bytes = STANDARD.decode(encoded).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }
}
