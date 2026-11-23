//! Profile resolution and directory layout.
//!
//! Two profile modes are supported:
//! - `Portable`: everything lives beside the executable.
//! - `AppData`: configuration lives in the user config directory, while logs,
//!   locality and the database live in the local data directory.
//!
//! Marker files in either candidate base directory override detection. The
//! environment variables `HREMOTE_PROFILE_MODE` and `HREMOTE_BASE_DIR` are
//! intended for automated tests and portable launchers.

use std::path::{Path, PathBuf};

use hr_core::{AppError, AppResult};
use serde::{Deserialize, Serialize};

pub const APP_DIR_NAME: &str = "H-Remote";
pub const LEGACY_APP_DIR_NAMES: [&str; 2] = ["1Remote", "PRemoteM"];
pub const MARKER_PORTABLE: &str = "FORCE_INTO_PORTABLE_MODE";
pub const MARKER_APPDATA: &str = "FORCE_INTO_APPDATA_MODE";
pub const CONFIG_FILE: &str = "hremote.json";
pub const DATA_SOURCES_FILE: &str = "hremote.dataSources.json";
pub const DATABASE_FILE: &str = "hremote.db";
pub const LEGACY_CONFIG_FILE: &str = "1Remote.json";
pub const LOCALE_ENV: &str = "HREMOTE_PROFILE_MODE";
pub const BASE_ENV: &str = "HREMOTE_BASE_DIR";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileMode {
    Portable,
    AppData,
}

impl ProfileMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Portable => "portable",
            Self::AppData => "appdata",
        }
    }
}

/// All paths derived from the selected profile mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfilePaths {
    pub mode: ProfileMode,
    /// Config and executable-adjacent data.
    pub base_dir: PathBuf,
    /// Root for logs, icons and locality (may differ from `base_dir`).
    pub data_dir: PathBuf,
    pub locality_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub icons_dir: PathBuf,
    pub icon_library_dir: PathBuf,
    pub runners_dir: PathBuf,
    pub protocols_dir: PathBuf,
    pub config_file: PathBuf,
    pub data_sources_file: PathBuf,
    pub database_file: PathBuf,
    /// Marker that forced the mode, if any.
    pub marker_file: Option<PathBuf>,
}

impl ProfilePaths {
    pub fn locality_file(&self) -> PathBuf {
        self.locality_dir.join(".locality.json")
    }

    pub fn tags_file(&self) -> PathBuf {
        self.locality_dir.join(".tags.json")
    }

    pub fn connection_records_file(&self) -> PathBuf {
        self.locality_dir.join(".connection_records.json")
    }

    pub fn list_view_file(&self) -> PathBuf {
        self.locality_dir.join(".list_view.json")
    }

    pub fn tree_view_file(&self) -> PathBuf {
        self.locality_dir.join(".tree_view.json")
    }

    pub fn log_file(&self) -> PathBuf {
        self.logs_dir.join("hremote.log.md")
    }

    pub fn has_profile(&self) -> bool {
        self.config_file.is_file()
    }

    /// Creates every directory the application needs. Idempotent.
    pub fn ensure_layout(&self) -> AppResult<()> {
        for directory in [
            &self.base_dir,
            &self.data_dir,
            &self.locality_dir,
            &self.logs_dir,
            &self.icons_dir,
            &self.icon_library_dir,
            &self.runners_dir,
            &self.protocols_dir,
        ] {
            std::fs::create_dir_all(directory).map_err(|error| {
                AppError::PermissionDenied(format!(
                    "cannot create directory {}: {error}",
                    directory.display()
                ))
            })?;
        }
        Ok(())
    }

    /// Writes the marker file that pins the selected mode for next launch.
    pub fn write_marker(&self, mode: ProfileMode) -> AppResult<PathBuf> {
        let name = match mode {
            ProfileMode::Portable => MARKER_PORTABLE,
            ProfileMode::AppData => MARKER_APPDATA,
        };
        let path = self.base_dir.join(name);
        std::fs::write(
            &path,
            b"Managed by H-Remote. Delete to reset profile detection.\n",
        )?;
        Ok(path)
    }
}

/// Injectable inputs so profile detection is fully unit-testable.
#[derive(Debug, Clone, Default)]
pub struct ResolveInput {
    pub exe_dir: PathBuf,
    pub config_dir: Option<PathBuf>,
    pub data_local_dir: Option<PathBuf>,
    pub env_mode: Option<String>,
    pub env_base: Option<PathBuf>,
    /// Trust this answer instead of probing the file system (tests, dry runs).
    pub exe_dir_writable: Option<bool>,
}

/// Resolves the profile using real environment variables and OS directories.
pub fn resolve_default(exe_dir: &Path) -> AppResult<ProfilePaths> {
    let input = ResolveInput {
        exe_dir: exe_dir.to_path_buf(),
        config_dir: dirs::config_dir(),
        data_local_dir: dirs::data_local_dir(),
        env_mode: std::env::var(LOCALE_ENV).ok(),
        env_base: std::env::var_os(BASE_ENV).map(PathBuf::from),
        exe_dir_writable: None,
    };
    resolve(&input)
}

/// Pure profile resolution.
pub fn resolve(input: &ResolveInput) -> AppResult<ProfilePaths> {
    if input.exe_dir.as_os_str().is_empty() {
        return Err(AppError::config("executable directory is unknown"));
    }

    let appdata_base = input
        .config_dir
        .clone()
        .ok_or_else(|| AppError::config("user configuration directory is unavailable"))?
        .join(APP_DIR_NAME);
    let appdata_data = input
        .data_local_dir
        .clone()
        .unwrap_or_else(|| appdata_base.clone())
        .join(APP_DIR_NAME);

    let portable_markers: Vec<PathBuf> = [
        input.exe_dir.join(MARKER_PORTABLE),
        appdata_base.join(MARKER_PORTABLE),
    ]
    .into_iter()
    .filter(|path| path.is_file())
    .collect();
    let appdata_markers: Vec<PathBuf> = [
        input.exe_dir.join(MARKER_APPDATA),
        appdata_base.join(MARKER_APPDATA),
    ]
    .into_iter()
    .filter(|path| path.is_file())
    .collect();

    if !portable_markers.is_empty() && !appdata_markers.is_empty() {
        return Err(AppError::conflict(format!(
            "both portable and AppData marker files exist: {}",
            portable_markers
                .iter()
                .chain(appdata_markers.iter())
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }

    let requested_mode = input
        .env_mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| match value.to_ascii_lowercase().as_str() {
            "portable" => Ok(ProfileMode::Portable),
            "appdata" | "app-data" => Ok(ProfileMode::AppData),
            other => Err(AppError::config(format!(
                "unknown profile mode '{other}' in {LOCALE_ENV}"
            ))),
        })
        .transpose()?;

    let (mode, marker_file) = if let Some(mode) = requested_mode {
        (mode, None)
    } else if let Some(marker) = portable_markers.first() {
        (ProfileMode::Portable, Some(marker.clone()))
    } else if let Some(marker) = appdata_markers.first() {
        (ProfileMode::AppData, Some(marker.clone()))
    } else if appdata_base.join(CONFIG_FILE).is_file() {
        (ProfileMode::AppData, None)
    } else if input.env_base.is_some()
        || input
            .exe_dir_writable
            .unwrap_or_else(|| directory_is_writable(&input.exe_dir))
    {
        (ProfileMode::Portable, None)
    } else {
        (ProfileMode::AppData, None)
    };

    let (base_dir, data_dir) = match mode {
        ProfileMode::Portable => {
            let base = input
                .env_base
                .clone()
                .unwrap_or_else(|| input.exe_dir.clone());
            (base.clone(), base)
        }
        ProfileMode::AppData => (appdata_base, appdata_data),
    };

    Ok(ProfilePaths {
        mode,
        locality_dir: data_dir.join(".locality"),
        logs_dir: data_dir.join(".logs"),
        icons_dir: data_dir.join(".icons"),
        icon_library_dir: data_dir.join(".icons").join("library"),
        runners_dir: base_dir.join("runners"),
        protocols_dir: base_dir.join("Protocols"),
        config_file: base_dir.join(CONFIG_FILE),
        data_sources_file: base_dir.join(DATA_SOURCES_FILE),
        database_file: base_dir.join(DATABASE_FILE),
        base_dir,
        data_dir,
        marker_file,
    })
}

/// Probes a directory by creating and deleting a temporary file.
pub fn directory_is_writable(directory: &Path) -> bool {
    if std::fs::create_dir_all(directory).is_err() {
        return false;
    }
    let probe = directory.join(format!(
        ".hremote-write-probe-{}-{}",
        std::process::id(),
        chrono_like_counter()
    ));
    match std::fs::write(&probe, b"probe") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn chrono_like_counter() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn input(exe: &Path, config: &Path, data: &Path) -> ResolveInput {
        ResolveInput {
            exe_dir: exe.to_path_buf(),
            config_dir: Some(config.to_path_buf()),
            data_local_dir: Some(data.to_path_buf()),
            ..ResolveInput::default()
        }
    }

    #[test]
    fn portable_by_default_when_executable_directory_is_writable() {
        let exe = TempDir::new().unwrap();
        let config = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let paths = resolve(&input(exe.path(), config.path(), data.path())).unwrap();
        assert_eq!(paths.mode, ProfileMode::Portable);
        assert_eq!(paths.base_dir, exe.path());
        assert_eq!(paths.locality_dir, exe.path().join(".locality"));
        assert_eq!(paths.config_file, exe.path().join(CONFIG_FILE));
    }

    #[test]
    fn existing_appdata_profile_wins_over_portable_default() {
        let exe = TempDir::new().unwrap();
        let config = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let appdata_base = config.path().join(APP_DIR_NAME);
        std::fs::create_dir_all(&appdata_base).unwrap();
        std::fs::write(appdata_base.join(CONFIG_FILE), b"{}").unwrap();

        let paths = resolve(&input(exe.path(), config.path(), data.path())).unwrap();
        assert_eq!(paths.mode, ProfileMode::AppData);
        assert_eq!(paths.base_dir, appdata_base);
        assert_eq!(
            paths.locality_dir,
            data.path().join(APP_DIR_NAME).join(".locality")
        );
    }

    #[test]
    fn marker_files_override_detection() {
        let exe = TempDir::new().unwrap();
        let config = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        std::fs::write(exe.path().join(MARKER_APPDATA), b"").unwrap();

        let paths = resolve(&input(exe.path(), config.path(), data.path())).unwrap();
        assert_eq!(paths.mode, ProfileMode::AppData);
        assert_eq!(paths.marker_file, Some(exe.path().join(MARKER_APPDATA)));
    }

    #[test]
    fn conflicting_markers_are_rejected() {
        let exe = TempDir::new().unwrap();
        let config = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        std::fs::create_dir_all(config.path().join(APP_DIR_NAME)).unwrap();
        std::fs::write(exe.path().join(MARKER_PORTABLE), b"").unwrap();
        std::fs::write(config.path().join(APP_DIR_NAME).join(MARKER_APPDATA), b"").unwrap();

        let error = resolve(&input(exe.path(), config.path(), data.path())).unwrap_err();
        assert!(matches!(error, AppError::Conflict(_)));
    }

    #[test]
    fn environment_mode_and_base_override_everything() {
        let exe = TempDir::new().unwrap();
        let config = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let custom = TempDir::new().unwrap();
        let mut options = input(exe.path(), config.path(), data.path());
        options.env_mode = Some("portable".to_string());
        options.env_base = Some(custom.path().to_path_buf());

        let paths = resolve(&options).unwrap();
        assert_eq!(paths.mode, ProfileMode::Portable);
        assert_eq!(paths.base_dir, custom.path());

        options.env_mode = Some("nonsense".to_string());
        assert!(resolve(&options).is_err());
    }

    #[test]
    fn read_only_executable_directory_falls_back_to_appdata() {
        let exe = TempDir::new().unwrap();
        let config = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let mut options = input(exe.path(), config.path(), data.path());
        options.exe_dir_writable = Some(false);
        let paths = resolve(&options).unwrap();
        assert_eq!(paths.mode, ProfileMode::AppData);
    }

    #[test]
    fn layout_and_marker_round_trip() {
        let exe = TempDir::new().unwrap();
        let config = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let paths = resolve(&input(exe.path(), config.path(), data.path())).unwrap();
        paths.ensure_layout().unwrap();
        assert!(paths.locality_dir.is_dir());
        assert!(paths.logs_dir.is_dir());
        assert!(paths.icon_library_dir.is_dir());

        let marker = paths.write_marker(ProfileMode::Portable).unwrap();
        assert!(marker.is_file());
        let reloaded = resolve(&input(exe.path(), config.path(), data.path())).unwrap();
        assert_eq!(reloaded.mode, ProfileMode::Portable);
        assert_eq!(reloaded.marker_file, Some(marker));
    }

    #[test]
    fn locality_file_names_match_the_specification() {
        let paths = ProfilePaths {
            mode: ProfileMode::Portable,
            base_dir: PathBuf::from("/b"),
            data_dir: PathBuf::from("/b"),
            locality_dir: PathBuf::from("/b/.locality"),
            logs_dir: PathBuf::from("/b/.logs"),
            icons_dir: PathBuf::from("/b/.icons"),
            icon_library_dir: PathBuf::from("/b/.icons/library"),
            runners_dir: PathBuf::from("/b/runners"),
            protocols_dir: PathBuf::from("/b/Protocols"),
            config_file: PathBuf::from("/b/hremote.json"),
            data_sources_file: PathBuf::from("/b/hremote.dataSources.json"),
            database_file: PathBuf::from("/b/hremote.db"),
            marker_file: None,
        };
        assert!(paths.locality_file().ends_with(".locality/.locality.json"));
        assert!(paths.tags_file().ends_with(".locality/.tags.json"));
        assert!(
            paths
                .connection_records_file()
                .ends_with(".locality/.connection_records.json")
        );
        assert!(
            paths
                .list_view_file()
                .ends_with(".locality/.list_view.json")
        );
        assert!(
            paths
                .tree_view_file()
                .ends_with(".locality/.tree_view.json")
        );
    }
}
