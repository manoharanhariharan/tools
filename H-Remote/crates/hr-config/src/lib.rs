//! Configuration, profile resolution, locality data and crash-safe persistence.

pub mod assets;
pub mod atomic;
pub mod config;
pub mod legacy;
pub mod locality;
pub mod paths;
pub mod preflight;
pub mod runners;
pub mod store;

use std::time::Duration;

pub use assets::{AuxiliaryAssets, ProtocolConfigAsset, load_assets};
pub use atomic::{LoadOutcome, read_json_or_default, write_bytes_atomic, write_json_atomic};
pub use config::{
    CloseButtonAction, Configuration, EngagementConfig, GeneralConfig, KeywordMatchConfig,
    LauncherConfig, LogLevel, MatcherKind, RATING_MIN_LAUNCHES, RATING_PROMPT_COOLDOWN_SECONDS,
    TabHeaderConfig, normalize_tags, rating_prompt_due,
};
pub use legacy::{
    LegacyKind, LegacyProfile, LegacyScanInput, LegacySettings, detect as detect_legacy_profiles,
    find_legacy_database, import_settings as import_legacy_settings,
};
pub use locality::{
    ConnectionRecord, ConnectionRecords, ListViewLocality, Locality, QuickConnectEntry, ServerSort,
    ServerView, SessionLocality, TagsLocality, TreeViewLocality, UiLocality, WindowPlacement,
};
pub use paths::{
    APP_DIR_NAME, BASE_ENV, CONFIG_FILE, DATA_SOURCES_FILE, DATABASE_FILE, LOCALE_ENV,
    MARKER_APPDATA, MARKER_PORTABLE, ProfileMode, ProfilePaths, ResolveInput,
    resolve as resolve_profile, resolve_default as resolve_default_profile,
};
pub use preflight::{PathCheck, PreflightReport, preflight};
pub use runners::{
    DEFAULT_RUNNER_ID, KITTY_SSH, PUTTY_SERIAL, PUTTY_SSH, PUTTY_TELNET, Runner, RunnerKind,
    RunnersConfig, default_macros, is_builtin_id,
};
pub use store::JsonStore;

use hr_core::AppResult;

/// Opens `hremote.json`, sanitising values before they reach the UI.
pub fn open_config_store(
    paths: &ProfilePaths,
    debounce: Duration,
) -> AppResult<JsonStore<Configuration>> {
    JsonStore::open_with(
        paths.config_file.clone(),
        debounce,
        |config: &mut Configuration| config.sanitize(),
    )
}

pub fn open_locality_store(
    paths: &ProfilePaths,
    debounce: Duration,
) -> AppResult<JsonStore<Locality>> {
    JsonStore::open(paths.locality_file(), debounce)
}

pub fn open_tags_store(
    paths: &ProfilePaths,
    debounce: Duration,
) -> AppResult<JsonStore<TagsLocality>> {
    JsonStore::open_with(paths.tags_file(), debounce, |tags: &mut TagsLocality| {
        tags.sanitize()
    })
}

pub fn open_connection_records_store(
    paths: &ProfilePaths,
    debounce: Duration,
) -> AppResult<JsonStore<ConnectionRecords>> {
    JsonStore::open(paths.connection_records_file(), debounce)
}

pub fn open_list_view_store(
    paths: &ProfilePaths,
    debounce: Duration,
) -> AppResult<JsonStore<ListViewLocality>> {
    JsonStore::open(paths.list_view_file(), debounce)
}

pub fn open_tree_view_store(
    paths: &ProfilePaths,
    debounce: Duration,
) -> AppResult<JsonStore<TreeViewLocality>> {
    JsonStore::open(paths.tree_view_file(), debounce)
}
