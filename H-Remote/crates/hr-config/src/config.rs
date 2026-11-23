//! Global configuration (`hremote.json`).
//!
//! Every field has a default and unknown fields are preserved via `extra`, so
//! a newer build can add settings without older builds destroying them.

use hr_theme::ThemeConfig;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const CONFIG_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_HOTKEY: &str = "Alt+M";
pub const MAX_RECENT_SESSIONS: u8 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum CloseButtonAction {
    Exit,
    #[default]
    MinimizeToTray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Off,
    Error,
    #[default]
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    /// Filter directive understood by `tracing-subscriber`.
    pub const fn directive(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }

    pub fn from_tracing(level: tracing::Level) -> Self {
        if level == tracing::Level::ERROR {
            Self::Error
        } else if level == tracing::Level::WARN {
            Self::Warn
        } else if level == tracing::Level::INFO {
            Self::Info
        } else if level == tracing::Level::DEBUG {
            Self::Debug
        } else {
            Self::Trace
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MatcherKind {
    #[default]
    Direct,
    Initials,
    Discrete,
    Pinyin,
    PinyinInitials,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TabHeaderConfig {
    pub show_icon: bool,
    pub show_close_button: bool,
    pub show_reconnect_button: bool,
    pub close_button_on_left: bool,
}

impl Default for TabHeaderConfig {
    fn default() -> Self {
        Self {
            show_icon: true,
            show_close_button: true,
            show_reconnect_button: false,
            close_button_on_left: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GeneralConfig {
    /// `system` or a supported locale id such as `de-DE`.
    pub language: String,
    pub run_at_startup: bool,
    pub close_button_action: CloseButtonAction,
    pub confirm_before_closing_session: bool,
    pub show_session_icon_in_title: bool,
    pub tab_header: TabHeaderConfig,
    pub show_recent_in_tray: bool,
    pub show_note_in_list: bool,
    pub log_level: LogLevel,
    pub copy_port_with_address: bool,
    pub move_focus_to_local_on_mouse_leave: bool,
    pub do_not_check_for_updates: bool,
    /// Offer pre-release manifests during update checks (P9-10).
    pub include_prerelease_updates: bool,
    /// Automatic update-check interval in hours; `0` disables periodic checks.
    pub update_check_period_hours: u16,
    /// Optional JSON manifest URL for update checks.
    pub update_feed_url: Option<String>,
    pub start_minimized: bool,
    /// Set after the first-run guidance wizard completes.
    pub guidance_completed: bool,
    /// Skip the migration prompt permanently.
    pub skip_legacy_migration: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            language: "system".to_string(),
            run_at_startup: false,
            close_button_action: CloseButtonAction::MinimizeToTray,
            confirm_before_closing_session: false,
            show_session_icon_in_title: true,
            tab_header: TabHeaderConfig::default(),
            show_recent_in_tray: false,
            show_note_in_list: true,
            log_level: LogLevel::Warn,
            copy_port_with_address: true,
            move_focus_to_local_on_mouse_leave: false,
            do_not_check_for_updates: false,
            include_prerelease_updates: false,
            update_check_period_hours: 24,
            update_feed_url: None,
            start_minimized: false,
            guidance_completed: false,
            skip_legacy_migration: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LauncherConfig {
    pub enabled: bool,
    /// Canonical hotkey string, e.g. `Alt+M`.
    pub hotkey: String,
    pub show_note: bool,
    pub show_credentials: bool,
    pub save_entered_info: bool,
    pub match_credential: bool,
    pub recent_limit: u8,
    pub remember_position: bool,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hotkey: DEFAULT_HOTKEY.to_string(),
            show_note: true,
            show_credentials: false,
            save_entered_info: false,
            match_credential: true,
            recent_limit: MAX_RECENT_SESSIONS,
            remember_position: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct KeywordMatchConfig {
    pub matchers: Vec<MatcherKind>,
    /// Search debounce in milliseconds.
    pub debounce_ms: u64,
    pub parallel: bool,
    /// Add pinyin matchers automatically for Chinese locales while only Direct
    /// is selected (P10-03).
    pub auto_locale: bool,
}

impl Default for KeywordMatchConfig {
    fn default() -> Self {
        Self {
            matchers: vec![MatcherKind::Direct],
            debounce_ms: 150,
            parallel: true,
            auto_locale: true,
        }
    }
}

impl KeywordMatchConfig {
    /// Matchers used for a launch, applying locale-aware defaults without
    /// overriding an explicit multi-provider selection.
    pub fn effective_matchers(&self, language: &str) -> Vec<MatcherKind> {
        let mut matchers = self.matchers.clone();
        let only_direct = matchers
            .iter()
            .all(|matcher| *matcher == MatcherKind::Direct);
        if self.auto_locale && only_direct && language.to_ascii_lowercase().starts_with("zh") {
            matchers.push(MatcherKind::Pinyin);
            matchers.push(MatcherKind::PinyinInitials);
        }
        matchers
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct EngagementConfig {
    pub launches: u32,
    pub first_run_at: Option<i64>,
    pub last_update_check_at: Option<i64>,
    pub last_rating_prompt_at: Option<i64>,
    pub do_not_show_rating: bool,
}

/// Number of launches before the first rating prompt (P9-11).
pub const RATING_MIN_LAUNCHES: u32 = 10;
/// Minimum time between rating prompts in seconds (90 days).
pub const RATING_PROMPT_COOLDOWN_SECONDS: i64 = 90 * 24 * 60 * 60;

/// Pure threshold logic so the prompt is testable without a UI.
pub fn rating_prompt_due(engagement: &EngagementConfig, now: i64) -> bool {
    if engagement.do_not_show_rating || engagement.launches < RATING_MIN_LAUNCHES {
        return false;
    }
    match engagement.last_rating_prompt_at {
        Some(last) => now.saturating_sub(last) >= RATING_PROMPT_COOLDOWN_SECONDS,
        None => true,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Configuration {
    pub schema_version: u32,
    pub general: GeneralConfig,
    pub launcher: LauncherConfig,
    pub runners: crate::runners::RunnersConfig,
    pub keyword_match: KeywordMatchConfig,
    pub theme: ThemeConfig,
    pub engagement: EngagementConfig,
    pub database_check_period: u16,
    pub database_reconnect_period: u16,
    /// Absolute or profile-relative SQLite path. `None` uses the profile default.
    pub sqlite_database_path: Option<String>,
    pub pinned_tags: Vec<String>,
    /// Forward-compatible storage for settings added by newer builds.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            general: GeneralConfig::default(),
            launcher: LauncherConfig::default(),
            runners: crate::runners::RunnersConfig::default(),
            keyword_match: KeywordMatchConfig::default(),
            theme: ThemeConfig::default(),
            engagement: EngagementConfig::default(),
            database_check_period: 10,
            database_reconnect_period: 120,
            sqlite_database_path: None,
            pinned_tags: Vec::new(),
            extra: Map::new(),
        }
    }
}

impl Configuration {
    pub fn language(&self) -> Option<&str> {
        let language = self.general.language.trim();
        if language.is_empty() || language.eq_ignore_ascii_case("system") {
            None
        } else {
            Some(language)
        }
    }

    /// Clamps every numeric/range-constrained field and normalises tags. Called
    /// after loading and before saving so persisted state is always valid.
    pub fn sanitize(&mut self) {
        self.schema_version = CONFIG_SCHEMA_VERSION;
        self.database_check_period = self.database_check_period.min(99);
        self.database_reconnect_period = self.database_reconnect_period.min(3600);
        self.general.update_check_period_hours = self.general.update_check_period_hours.min(168);
        self.general.update_feed_url = self
            .general
            .update_feed_url
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(str::to_string);
        self.keyword_match.debounce_ms = self.keyword_match.debounce_ms.clamp(50, 1000);
        self.launcher.recent_limit = self.launcher.recent_limit.min(MAX_RECENT_SESSIONS);
        self.runners.sanitize();
        if self.launcher.hotkey.trim().is_empty() {
            self.launcher.hotkey = DEFAULT_HOTKEY.to_string();
        } else {
            self.launcher.hotkey = self.launcher.hotkey.trim().to_string();
        }
        if self.general.language.trim().is_empty() {
            self.general.language = "system".to_string();
        }
        self.theme.sanitize();

        self.keyword_match
            .matchers
            .retain(|matcher| *matcher != MatcherKind::Direct);
        self.keyword_match.matchers.insert(0, MatcherKind::Direct);
        let mut seen = std::collections::HashSet::new();
        self.keyword_match
            .matchers
            .retain(|matcher| seen.insert(*matcher));

        self.pinned_tags = normalize_tags(std::mem::take(&mut self.pinned_tags));
        if self
            .sqlite_database_path
            .as_deref()
            .is_some_and(|path| path.trim().is_empty())
        {
            self.sqlite_database_path = None;
        }
    }

    /// Applies the default profile-relative SQLite path when none is configured.
    pub fn database_path(&self, default_path: &std::path::Path) -> std::path::PathBuf {
        self.sqlite_database_path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(std::path::PathBuf::from)
            .map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    default_path
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."))
                        .join(path)
                }
            })
            .unwrap_or_else(|| default_path.to_path_buf())
    }

    /// Environment override used by tests and portable launchers.
    pub fn apply_env_overrides(&mut self) {
        if let Ok(mode) = std::env::var(crate::paths::LOCALE_ENV)
            && !mode.trim().is_empty()
        {
            tracing::debug!(%mode, "profile override environment variable present");
        }
    }
}

pub fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut normalized: Vec<String> = tags
        .into_iter()
        .map(|tag| tag.trim().trim_start_matches('#').to_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect();
    normalized.sort();
    normalized.dedup();
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_phase_zero_settings() {
        let config = Configuration::default();
        assert_eq!(config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(config.general.language, "system");
        assert_eq!(
            config.general.close_button_action,
            CloseButtonAction::MinimizeToTray
        );
        assert!(config.general.show_note_in_list);
        assert_eq!(config.general.log_level, LogLevel::Warn);
        assert_eq!(config.launcher.hotkey, DEFAULT_HOTKEY);
        assert_eq!(config.keyword_match.matchers, vec![MatcherKind::Direct]);
    }

    #[test]
    fn older_config_schema_is_upgraded_on_sanitize() {
        let mut config = Configuration {
            schema_version: 0,
            ..Configuration::default()
        };
        config.sanitize();
        assert_eq!(config.schema_version, CONFIG_SCHEMA_VERSION);
    }

    #[test]
    fn locale_aware_matchers_only_apply_to_a_direct_only_selection() {
        let config = KeywordMatchConfig::default();
        assert_eq!(
            config.effective_matchers("zh-CN"),
            vec![
                MatcherKind::Direct,
                MatcherKind::Pinyin,
                MatcherKind::PinyinInitials
            ]
        );
        assert_eq!(
            config.effective_matchers("en-US"),
            vec![MatcherKind::Direct]
        );

        let mut explicit = config.clone();
        explicit.matchers = vec![MatcherKind::Direct, MatcherKind::Initials];
        assert_eq!(
            explicit.effective_matchers("zh-CN"),
            vec![MatcherKind::Direct, MatcherKind::Initials]
        );

        let mut disabled = config;
        disabled.auto_locale = false;
        assert_eq!(
            disabled.effective_matchers("zh-CN"),
            vec![MatcherKind::Direct]
        );
    }

    #[test]
    fn rating_prompt_respects_thresholds_and_opt_out() {
        let mut engagement = EngagementConfig::default();
        assert!(!rating_prompt_due(&engagement, 0));

        engagement.launches = RATING_MIN_LAUNCHES;
        assert!(rating_prompt_due(&engagement, 1_000));

        engagement.last_rating_prompt_at = Some(1_000);
        assert!(!rating_prompt_due(
            &engagement,
            1_000 + RATING_PROMPT_COOLDOWN_SECONDS - 1
        ));
        assert!(rating_prompt_due(
            &engagement,
            1_000 + RATING_PROMPT_COOLDOWN_SECONDS
        ));

        engagement.do_not_show_rating = true;
        assert!(!rating_prompt_due(&engagement, 9_999_999));
    }

    #[test]
    fn sanitize_clamps_and_normalises() {
        let mut config = Configuration {
            database_check_period: 500,
            database_reconnect_period: u16::MAX,
            pinned_tags: vec![
                " #Linux ".into(),
                "linux".into(),
                "".into(),
                "#Windows".into(),
            ],
            ..Configuration::default()
        };
        config.launcher.recent_limit = 200;
        config.launcher.hotkey = "   ".into();
        config.general.update_check_period_hours = 1000;
        config.general.update_feed_url =
            Some("  https://updates.example.invalid/latest.json  ".into());
        config.keyword_match.debounce_ms = 1;
        config.keyword_match.matchers = vec![
            MatcherKind::Pinyin,
            MatcherKind::Direct,
            MatcherKind::Pinyin,
        ];
        config.theme.font_size = 99;
        config.sanitize();

        assert_eq!(config.database_check_period, 99);
        assert_eq!(config.database_reconnect_period, 3600);
        assert_eq!(config.general.update_check_period_hours, 168);
        assert_eq!(
            config.general.update_feed_url.as_deref(),
            Some("https://updates.example.invalid/latest.json")
        );
        assert_eq!(config.launcher.recent_limit, MAX_RECENT_SESSIONS);
        assert_eq!(config.launcher.hotkey, DEFAULT_HOTKEY);
        assert_eq!(config.keyword_match.debounce_ms, 50);
        assert_eq!(
            config.keyword_match.matchers,
            vec![MatcherKind::Direct, MatcherKind::Pinyin]
        );
        assert_eq!(config.pinned_tags, vec!["linux", "windows"]);
        assert_eq!(config.theme.font_size, hr_theme::MAX_FONT_SIZE);
    }

    #[test]
    fn unknown_fields_survive_a_round_trip() {
        let json = r#"{
            "schemaVersion": 1,
            "general": { "language": "de-DE", "futureSetting": true },
            "futureTopLevel": { "nested": 42 }
        }"#;
        let mut config: Configuration = serde_json::from_str(json).unwrap();
        assert_eq!(config.general.language, "de-DE");
        assert!(config.extra.contains_key("futureTopLevel"));

        config.sanitize();
        let encoded = serde_json::to_string(&config).unwrap();
        assert!(encoded.contains("futureTopLevel"));
    }

    #[test]
    fn missing_fields_use_defaults() {
        let config: Configuration = serde_json::from_str("{}").unwrap();
        assert_eq!(config, Configuration::default());
    }

    #[test]
    fn relative_database_path_resolves_against_profile() {
        let default = std::path::Path::new("/profile/hremote.db");
        let mut config = Configuration {
            sqlite_database_path: Some("data/custom.db".into()),
            ..Configuration::default()
        };
        assert_eq!(
            config.database_path(default),
            std::path::PathBuf::from("/profile/data/custom.db")
        );

        config.sqlite_database_path = Some("/absolute/custom.db".into());
        assert_eq!(
            config.database_path(default),
            std::path::PathBuf::from("/absolute/custom.db")
        );

        config.sqlite_database_path = None;
        assert_eq!(config.database_path(default), default.to_path_buf());
    }

    #[test]
    fn log_level_directives_are_valid_filters() {
        assert_eq!(LogLevel::Off.directive(), "off");
        assert_eq!(LogLevel::Warn.directive(), "warn");
        assert_eq!(
            LogLevel::from_tracing(tracing::Level::DEBUG),
            LogLevel::Debug
        );
    }

    #[test]
    fn language_sentinel_maps_to_system() {
        let mut config = Configuration::default();
        assert_eq!(config.language(), None);
        config.general.language = "system".into();
        assert_eq!(config.language(), None);
        config.general.language = "fr-FR".into();
        assert_eq!(config.language(), Some("fr-FR"));
    }
}
