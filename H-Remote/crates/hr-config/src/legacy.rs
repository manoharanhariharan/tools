//! Detection and best-effort settings migration from PRemoteM/1Remote.
//!
//! Phase 0 imports preferences only (language, theme, general behaviour).
//! Server/credential import arrives with the catalog schema in Phase 9; the
//! wizard says so explicitly rather than pretending data was migrated.

use std::path::PathBuf;

use hr_core::{AppError, AppResult};
use hr_theme::ThemeConfig;
use serde::Serialize;
use serde_json::Value;

use crate::config::{Configuration, LogLevel};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LegacyKind {
    PRemoteM,
    OneRemote,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyProfile {
    pub kind: LegacyKind,
    pub path: PathBuf,
    pub config_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct LegacyScanInput {
    pub exe_dir: PathBuf,
    pub config_dir: Option<PathBuf>,
    pub data_local_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacySettings {
    pub source: PathBuf,
    pub imported_keys: Vec<String>,
}

/// Scans the well-known profile locations. Duplicate paths are removed.
/// Finds the legacy catalog database inside a detected profile (P9-08).
/// PRemoteM/1Remote store one SQLite file next to the JSON settings.
pub fn find_legacy_database(profile: &LegacyProfile) -> Option<PathBuf> {
    let directory = if profile.path.is_dir() {
        profile.path.clone()
    } else {
        profile.path.parent()?.to_path_buf()
    };
    let entries = std::fs::read_dir(directory).ok()?;
    let mut candidates: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !matches!(extension.as_str(), "db" | "db3" | "sqlite" | "sqlite3") {
                return false;
            }
            let name = path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            name.contains("remote") || name.contains("premotem") || name.contains("1remote")
        })
        .collect();
    candidates.sort();
    candidates.into_iter().next()
}

pub fn detect(input: &LegacyScanInput) -> Vec<LegacyProfile> {
    let mut candidates: Vec<LegacyProfile> = Vec::new();
    let mut bases: Vec<PathBuf> = Vec::new();
    if let Some(config_dir) = &input.config_dir {
        bases.push(config_dir.clone());
    }
    if let Some(data_local_dir) = &input.data_local_dir {
        bases.push(data_local_dir.clone());
    }
    bases.push(input.exe_dir.clone());

    for base in bases {
        for (kind, directory, file) in [
            (LegacyKind::PRemoteM, "PRemoteM", "PRemoteM.json"),
            (LegacyKind::OneRemote, "1Remote", "1Remote.json"),
        ] {
            let profile_dir = base.join(directory);
            let direct_config = base.join(file);
            let config_file = [profile_dir.join(file), direct_config.clone()]
                .into_iter()
                .find(|path| path.is_file());
            if profile_dir.is_dir() || config_file.is_some() {
                let path = if profile_dir.is_dir() {
                    profile_dir
                } else {
                    direct_config
                };
                if !candidates
                    .iter()
                    .any(|candidate| candidate.kind == kind && candidate.path == path)
                {
                    candidates.push(LegacyProfile {
                        kind,
                        path,
                        config_file,
                    });
                }
            }
        }
    }

    candidates.sort_by(|left, right| left.path.cmp(&right.path));
    candidates
}

/// Loads preferences from a legacy configuration file and applies them to
/// `target`. Unknown values are ignored; malformed files return an error.
pub fn import_settings(
    profile: &LegacyProfile,
    target: &mut Configuration,
) -> AppResult<LegacySettings> {
    let config_file = profile
        .config_file
        .as_ref()
        .ok_or_else(|| AppError::not_found("legacy configuration file"))?;
    let bytes = std::fs::read(config_file)?;
    let json: Value = serde_json::from_slice(&bytes).map_err(|error| {
        AppError::config(format!(
            "cannot parse legacy configuration {}: {error}",
            config_file.display()
        ))
    })?;

    let mut imported_keys = Vec::new();
    let general = json.get("General").unwrap_or(&Value::Null);
    let theme = json.get("Theme").unwrap_or(&Value::Null);
    let launcher = json.get("Launcher").unwrap_or(&Value::Null);

    if let Some(language) = general
        .get("CurrentLanguageCode")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        target.general.language = language.to_string();
        imported_keys.push("general.language".to_string());
    }

    import_bool(
        general,
        "ConfirmBeforeClosingSession",
        &mut target.general.confirm_before_closing_session,
        "general.confirmBeforeClosingSession",
        &mut imported_keys,
    );
    import_bool(
        general,
        "ShowSessionIconInSessionWindow",
        &mut target.general.show_session_icon_in_title,
        "general.showSessionIconInTitle",
        &mut imported_keys,
    );
    import_bool(
        general,
        "ShowNoteFieldInListView",
        &mut target.general.show_note_in_list,
        "general.showNoteInList",
        &mut imported_keys,
    );
    import_bool(
        general,
        "CopyPortWhenCopyAddress",
        &mut target.general.copy_port_with_address,
        "general.copyPortWithAddress",
        &mut imported_keys,
    );
    import_bool(
        general,
        "DoNotCheckNewVersion",
        &mut target.general.do_not_check_for_updates,
        "general.doNotCheckForUpdates",
        &mut imported_keys,
    );
    import_bool(
        general,
        "TabWindowSetFocusToLocalDesktopOnMouseLeaveRdpWindow",
        &mut target.general.move_focus_to_local_on_mouse_leave,
        "general.moveFocusToLocalOnMouseLeave",
        &mut imported_keys,
    );
    import_bool(
        general,
        "TabHeaderShowIconButton",
        &mut target.general.tab_header.show_icon,
        "general.tabHeader.showIcon",
        &mut imported_keys,
    );
    import_bool(
        general,
        "TabHeaderShowCloseButton",
        &mut target.general.tab_header.show_close_button,
        "general.tabHeader.showCloseButton",
        &mut imported_keys,
    );
    import_bool(
        general,
        "TabHeaderShowReConnectButton",
        &mut target.general.tab_header.show_reconnect_button,
        "general.tabHeader.showReconnectButton",
        &mut imported_keys,
    );
    import_bool(
        general,
        "TabWindowCloseButtonOnLeft",
        &mut target.general.tab_header.close_button_on_left,
        "general.tabHeader.closeButtonOnLeft",
        &mut imported_keys,
    );

    if let Some(level) = general.get("LogLevel").and_then(Value::as_i64) {
        // 0=Debug, 1=Info, 2=Warning, 3=Error, 4=Disabled in the reference app.
        target.general.log_level = match level {
            0 => LogLevel::Debug,
            1 => LogLevel::Info,
            2 => LogLevel::Warn,
            3 => LogLevel::Error,
            _ => LogLevel::Off,
        };
        imported_keys.push("general.logLevel".to_string());
    }

    if let Some(enabled) = launcher.get("LauncherEnabled").and_then(Value::as_bool) {
        target.launcher.enabled = enabled;
        imported_keys.push("launcher.enabled".to_string());
    }

    if theme.is_object() {
        let mut imported = ThemeConfig {
            theme_name: theme
                .get("ThemeName")
                .and_then(Value::as_str)
                .unwrap_or("Dark")
                .to_string(),
            is_custom: false,
            font_family: theme
                .get("FontFamily")
                .and_then(Value::as_str)
                .filter(|family| !family.trim().is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| "system-ui".to_string()),
            font_size: theme
                .get("FontSize")
                .and_then(Value::as_i64)
                .map(|size| size.clamp(0, 255) as u8)
                .unwrap_or(12),
            ..ThemeConfig::default()
        };
        copy_colour(theme, "PrimaryMidColor", &mut imported.colors.primary_mid);
        copy_colour(
            theme,
            "PrimaryLightColor",
            &mut imported.colors.primary_light,
        );
        copy_colour(theme, "PrimaryDarkColor", &mut imported.colors.primary_dark);
        copy_colour(theme, "PrimaryTextColor", &mut imported.colors.primary_text);
        copy_colour(theme, "AccentMidColor", &mut imported.colors.accent_mid);
        copy_colour(theme, "AccentLightColor", &mut imported.colors.accent_light);
        copy_colour(theme, "AccentDarkColor", &mut imported.colors.accent_dark);
        copy_colour(theme, "AccentTextColor", &mut imported.colors.accent_text);
        copy_colour(theme, "BackgroundColor", &mut imported.colors.background);
        copy_colour(
            theme,
            "BackgroundTextColor",
            &mut imported.colors.background_text,
        );
        imported.sanitize();
        target.theme = imported;
        imported_keys.push("theme".to_string());
    }

    Ok(LegacySettings {
        source: config_file.clone(),
        imported_keys,
    })
}

fn import_bool(
    source: &Value,
    key: &str,
    target: &mut bool,
    label: &str,
    imported: &mut Vec<String>,
) {
    if let Some(value) = source.get(key).and_then(Value::as_bool) {
        *target = value;
        imported.push(label.to_string());
    }
}

fn copy_colour(source: &Value, key: &str, target: &mut String) {
    if let Some(value) = source
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        *target = value.to_string();
    }
}

/// Convenience used by the app: scan, pick the first profile with a config file.
pub fn first_migratable(input: &LegacyScanInput) -> Option<LegacyProfile> {
    detect(input)
        .into_iter()
        .find(|profile| profile.config_file.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const LEGACY_JSON: &str = r##"{
        "General": {
            "CurrentLanguageCode": "de-de",
            "CloseButtonBehavior": 0,
            "ConfirmBeforeClosingSession": true,
            "ShowSessionIconInSessionWindow": false,
            "ShowRecentlySessionInTray": true,
            "ShowNoteFieldInListView": false,
            "CopyPortWhenCopyAddress": false,
            "DoNotCheckNewVersion": true,
            "TabHeaderShowIconButton": false,
            "TabHeaderShowCloseButton": false,
            "TabHeaderShowReConnectButton": true,
            "TabHeaderShowCloseButtonOnLeft": true,
            "TabWindowCloseButtonOnLeft": true,
            "TabWindowSetFocusToLocalDesktopOnMouseLeaveRdpWindow": true,
            "LogLevel": 2
        },
        "Launcher": { "LauncherEnabled": false },
        "Theme": {
            "ThemeName": "Forest",
            "FontFamily": "Inter",
            "FontSize": 14,
            "PrimaryMidColor": "#FF253938",
            "AccentMidColor": "#FF5FA291"
        }
    }"##;

    #[test]
    fn finds_the_legacy_database_next_to_the_profile() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("1Remote.db"), "sqlite").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "ignore").unwrap();
        let profile = LegacyProfile {
            kind: LegacyKind::OneRemote,
            path: dir.path().to_path_buf(),
            config_file: None,
        };
        let database = find_legacy_database(&profile).unwrap();
        assert_eq!(database.file_name().unwrap(), "1Remote.db");

        std::fs::remove_file(dir.path().join("1Remote.db")).unwrap();
        assert!(find_legacy_database(&profile).is_none());
    }

    #[test]
    fn detects_legacy_profiles_in_injected_locations() {
        let exe = TempDir::new().unwrap();
        let config = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        std::fs::create_dir_all(config.path().join("PRemoteM")).unwrap();
        std::fs::write(config.path().join("PRemoteM/PRemoteM.json"), LEGACY_JSON).unwrap();
        std::fs::write(exe.path().join("1Remote.json"), "{}").unwrap();

        let found = detect(&LegacyScanInput {
            exe_dir: exe.path().to_path_buf(),
            config_dir: Some(config.path().to_path_buf()),
            data_local_dir: Some(data.path().to_path_buf()),
        });
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|p| p.kind == LegacyKind::PRemoteM));
        assert!(found.iter().any(|p| p.kind == LegacyKind::OneRemote));
    }

    #[test]
    fn imports_preferences_without_touching_servers() {
        let dir = TempDir::new().unwrap();
        let config_file = dir.path().join("PRemoteM.json");
        std::fs::write(&config_file, LEGACY_JSON).unwrap();
        let profile = LegacyProfile {
            kind: LegacyKind::PRemoteM,
            path: dir.path().to_path_buf(),
            config_file: Some(config_file.clone()),
        };
        let mut target = Configuration::default();
        let settings = import_settings(&profile, &mut target).unwrap();

        assert_eq!(settings.source, config_file);
        assert_eq!(target.general.language, "de-de");
        assert!(target.general.confirm_before_closing_session);
        assert!(!target.general.show_session_icon_in_title);
        assert!(!target.general.show_note_in_list);
        assert!(target.general.do_not_check_for_updates);
        assert!(!target.general.tab_header.show_icon);
        assert!(target.general.tab_header.show_reconnect_button);
        assert!(target.general.tab_header.close_button_on_left);
        assert_eq!(target.general.log_level, LogLevel::Warn);
        assert!(!target.launcher.enabled);
        assert_eq!(target.theme.theme_name, "Forest");
        assert_eq!(target.theme.font_family, "Inter");
        assert_eq!(target.theme.font_size, 14);
        assert_eq!(target.theme.colors.primary_mid, "#FF253938");
        assert!(settings.imported_keys.contains(&"theme".to_string()));
    }

    #[test]
    fn malformed_legacy_files_are_reported() {
        let dir = TempDir::new().unwrap();
        let config_file = dir.path().join("broken.json");
        std::fs::write(&config_file, "{broken").unwrap();
        let profile = LegacyProfile {
            kind: LegacyKind::OneRemote,
            path: dir.path().to_path_buf(),
            config_file: Some(config_file),
        };
        assert!(import_settings(&profile, &mut Configuration::default()).is_err());
    }

    #[test]
    fn missing_config_file_is_not_migratable() {
        let dir = TempDir::new().unwrap();
        let profile = LegacyProfile {
            kind: LegacyKind::PRemoteM,
            path: dir.path().to_path_buf(),
            config_file: None,
        };
        assert!(matches!(
            import_settings(&profile, &mut Configuration::default()),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn detection_is_empty_for_a_clean_machine() {
        let dir = TempDir::new().unwrap();
        let found = detect(&LegacyScanInput {
            exe_dir: dir.path().to_path_buf(),
            config_dir: Some(dir.path().to_path_buf()),
            data_local_dir: Some(dir.path().to_path_buf()),
        });
        assert!(found.is_empty());
    }
}
