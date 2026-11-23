//! Write-permission pre-flight (P0-09).
//!
//! Before the UI starts, every directory and file H-Remote needs must be
//! writable. The report is surfaced verbatim when startup cannot continue.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::paths::ProfilePaths;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathCheck {
    pub key: String,
    pub path: PathBuf,
    pub writable: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightReport {
    pub checks: Vec<PathCheck>,
    pub ok: bool,
}

impl PreflightReport {
    pub fn failures(&self) -> impl Iterator<Item = &PathCheck> {
        self.checks.iter().filter(|check| !check.writable)
    }

    pub fn first_error(&self) -> Option<String> {
        self.failures().next().map(|check| {
            check
                .error
                .clone()
                .unwrap_or_else(|| format!("{} is not writable", check.path.display()))
        })
    }
}

/// Creates and probes every required path. `create` should be true during
/// startup; tests use false to inspect an existing layout.
pub fn preflight(paths: &ProfilePaths, create: bool) -> PreflightReport {
    let mut checks = Vec::new();

    let directories: [(&str, &Path); 8] = [
        ("baseDir", &paths.base_dir),
        ("dataDir", &paths.data_dir),
        ("localityDir", &paths.locality_dir),
        ("logsDir", &paths.logs_dir),
        ("iconsDir", &paths.icons_dir),
        ("iconLibraryDir", &paths.icon_library_dir),
        ("runnersDir", &paths.runners_dir),
        ("protocolsDir", &paths.protocols_dir),
    ];

    for (key, path) in directories {
        checks.push(probe_directory(key, path, create));
    }

    for (key, path) in [
        ("configFile", &paths.config_file),
        ("dataSourcesFile", &paths.data_sources_file),
        ("databaseFile", &paths.database_file),
    ] {
        checks.push(probe_file(key, path));
    }

    let ok = checks.iter().all(|check| check.writable);
    PreflightReport { checks, ok }
}

fn probe_directory(key: &str, path: &Path, create: bool) -> PathCheck {
    if create && let Err(error) = std::fs::create_dir_all(path) {
        return PathCheck {
            key: key.to_string(),
            path: path.to_path_buf(),
            writable: false,
            error: Some(format!("cannot create directory: {error}")),
        };
    }
    if !path.is_dir() {
        return PathCheck {
            key: key.to_string(),
            path: path.to_path_buf(),
            writable: false,
            error: Some("directory does not exist".to_string()),
        };
    }

    let probe = path.join(format!(
        ".hremote-write-probe-{}-{}",
        std::process::id(),
        now_nanos()
    ));
    match std::fs::write(&probe, b"probe") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            PathCheck {
                key: key.to_string(),
                path: path.to_path_buf(),
                writable: true,
                error: None,
            }
        }
        Err(error) => PathCheck {
            key: key.to_string(),
            path: path.to_path_buf(),
            writable: false,
            error: Some(format!("directory is not writable: {error}")),
        },
    }
}

fn probe_file(key: &str, path: &Path) -> PathCheck {
    if path.exists() {
        match std::fs::OpenOptions::new().append(true).open(path) {
            Ok(_) => PathCheck {
                key: key.to_string(),
                path: path.to_path_buf(),
                writable: true,
                error: None,
            },
            Err(error) => PathCheck {
                key: key.to_string(),
                path: path.to_path_buf(),
                writable: false,
                error: Some(format!("file is not writable: {error}")),
            },
        }
    } else {
        // Do not create the file: existence of hremote.json marks a completed
        // profile, so the parent directory probe decides writability.
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let parent_check = probe_directory("parent", parent, false);
        PathCheck {
            key: key.to_string(),
            path: path.to_path_buf(),
            writable: parent_check.writable,
            error: parent_check
                .error
                .map(|error| format!("cannot create file: {error}")),
        }
    }
}

fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::{ProfileMode, ProfilePaths};
    use tempfile::TempDir;

    fn paths_for(root: &Path) -> ProfilePaths {
        ProfilePaths {
            mode: ProfileMode::Portable,
            base_dir: root.to_path_buf(),
            data_dir: root.to_path_buf(),
            locality_dir: root.join(".locality"),
            logs_dir: root.join(".logs"),
            icons_dir: root.join(".icons"),
            icon_library_dir: root.join(".icons").join("library"),
            runners_dir: root.join("runners"),
            protocols_dir: root.join("Protocols"),
            config_file: root.join("hremote.json"),
            data_sources_file: root.join("hremote.dataSources.json"),
            database_file: root.join("hremote.db"),
            marker_file: None,
        }
    }

    #[test]
    fn writable_layout_passes_and_creates_directories() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        let report = preflight(&paths, true);
        assert!(report.ok, "{:?}", report.failures().collect::<Vec<_>>());
        assert!(paths.logs_dir.is_dir());
        assert!(paths.icon_library_dir.is_dir());
        assert_eq!(report.checks.len(), 11);
        assert!(report.first_error().is_none());
    }

    #[test]
    fn unwritable_directory_is_reported() {
        let dir = TempDir::new().unwrap();
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, b"file").unwrap();
        let mut paths = paths_for(dir.path());
        paths.locality_dir = blocker; // a file, not a directory
        let report = preflight(&paths, false);
        assert!(!report.ok);
        let failure = report
            .failures()
            .find(|check| check.key == "localityDir")
            .unwrap();
        assert!(!failure.writable);
        assert!(failure.error.is_some());
    }

    #[test]
    fn missing_files_report_parent_writability_without_creating_them() {
        let dir = TempDir::new().unwrap();
        let paths = paths_for(dir.path());
        paths.ensure_layout().unwrap();
        let report = preflight(&paths, false);
        assert!(report.ok);
        assert!(
            !paths.config_file.exists(),
            "preflight must not create profile files"
        );
    }

    #[test]
    fn existing_read_only_file_is_detected_on_unix() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir = TempDir::new().unwrap();
            let paths = paths_for(dir.path());
            paths.ensure_layout().unwrap();
            std::fs::write(&paths.config_file, b"{}").unwrap();
            let mut permissions = std::fs::metadata(&paths.config_file).unwrap().permissions();
            permissions.set_mode(0o444);
            std::fs::set_permissions(&paths.config_file, permissions).unwrap();

            let report = preflight(&paths, false);
            let check = report
                .checks
                .iter()
                .find(|check| check.key == "configFile")
                .unwrap();
            assert!(!check.writable);
        }
    }
}
