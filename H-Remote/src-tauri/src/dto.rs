use std::path::PathBuf;

use hr_config::{Configuration, LegacyProfile, LogLevel, ProfileMode, ProfilePaths};
use hr_core::AppInfo;
use hr_crypto::VaultStatus;
use hr_i18n::LocaleInfo;
use hr_observability::Checkpoint;
use hr_platform::{Capabilities, CliAction};
use hr_theme::{ResolvedTheme, ThemeConfig, ThemePreset};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionFrameImage {
    pub seq: u64,
    pub width: u32,
    pub height: u32,
    pub data_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathsDto {
    pub profile_mode: ProfileMode,
    pub base_dir: String,
    pub data_dir: String,
    pub locality_dir: String,
    pub logs_dir: String,
    pub config_file: String,
    pub data_sources_file: String,
    pub database_file: String,
    pub log_file: String,
    pub marker_file: Option<String>,
}

impl From<&ProfilePaths> for PathsDto {
    fn from(paths: &ProfilePaths) -> Self {
        Self {
            profile_mode: paths.mode,
            base_dir: paths.base_dir.display().to_string(),
            data_dir: paths.data_dir.display().to_string(),
            locality_dir: paths.locality_dir.display().to_string(),
            logs_dir: paths.logs_dir.display().to_string(),
            config_file: paths.config_file.display().to_string(),
            data_sources_file: paths.data_sources_file.display().to_string(),
            database_file: paths.database_file.display().to_string(),
            log_file: paths.log_file().display().to_string(),
            marker_file: paths
                .marker_file
                .as_ref()
                .map(|path| path.display().to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    pub available: bool,
    pub path: String,
    pub writable: Option<bool>,
    pub schema_version: Option<i64>,
    pub encryption_verified: Option<bool>,
    pub server_count: Option<i64>,
    pub credential_count: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapDto {
    pub app: AppInfo,
    pub paths: PathsDto,
    pub first_run: bool,
    pub config: Configuration,
    pub theme: ResolvedTheme,
    pub themes: Vec<ThemePreset>,
    pub locales: Vec<LocaleInfo>,
    pub locale: String,
    pub system_locale: String,
    pub capabilities: Capabilities,
    pub vault: VaultStatus,
    pub preflight: hr_config::PreflightReport,
    pub startup: Vec<Checkpoint>,
    pub database: DatabaseInfo,
    pub legacy: Option<LegacyProfile>,
    pub auxiliary: hr_config::AuxiliaryAssets,
    pub protocols: Vec<hr_core::ProtocolDescriptor>,
    pub recovered_files: Vec<String>,
    pub marker_locked: bool,
    pub feedback_url: String,
    pub log_level: LogLevel,
    pub pending_actions: Vec<CliAction>,
    pub hotkey_error: Option<String>,
    pub show_rating_prompt: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GuidanceSelection {
    pub profile_mode: Option<ProfileMode>,
    pub language: Option<String>,
    pub theme_name: Option<String>,
    pub custom_theme: Option<ThemeConfig>,
    pub run_at_startup: bool,
    pub start_minimized: bool,
    pub create_shortcut: bool,
    pub confirm_before_closing_session: bool,
    pub import_legacy_settings: bool,
}

impl Default for GuidanceSelection {
    fn default() -> Self {
        Self {
            profile_mode: None,
            language: None,
            theme_name: Some("Dark".to_string()),
            custom_theme: None,
            run_at_startup: false,
            start_minimized: false,
            create_shortcut: false,
            confirm_before_closing_session: false,
            import_legacy_settings: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuidanceResult {
    pub config: Configuration,
    pub restart_required: bool,
    pub migrated: Option<hr_config::LegacySettings>,
    pub shortcut_path: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileChange {
    pub mode: ProfileMode,
    pub marker_path: Option<String>,
    pub restart_required: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentSessionInfo {
    pub server_id: String,
    pub last_connected_at: i64,
    pub connect_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashReportInfo {
    pub path: PathBuf,
    pub file_name: String,
    pub modified_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeValidation {
    pub theme: ResolvedTheme,
    pub css_variables: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSnapshot {
    pub servers: Vec<hr_domain::Server>,
    pub folders: Vec<hr_domain::Folder>,
    pub credentials: Vec<hr_domain::Credential>,
    pub protocols: Vec<hr_core::ProtocolDescriptor>,
    pub tags: Vec<hr_domain::Tag>,
    pub connection_records: hr_config::ConnectionRecords,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogView {
    pub tree: hr_domain::CatalogTree,
    pub hits: Vec<hr_domain::SearchHit>,
    pub tags: Vec<hr_domain::Tag>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOutcome {
    pub imported: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RdpExport {
    pub content: String,
    pub password_protected: bool,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutOutcome {
    pub path: String,
    pub icon_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconLibraryEntry {
    pub name: String,
    pub path: String,
    pub base64: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickConnectOutcome {
    pub session: hr_session::SessionSnapshot,
    pub saved: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailabilityEntry {
    pub name: String,
    pub address: String,
    pub port: u16,
    pub reachable: bool,
    pub latency_ms: Option<u128>,
}
