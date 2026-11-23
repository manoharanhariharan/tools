//! Auxiliary profile files loaded at startup (P0-13/P0-18/P0-19).
//!
//! Data sources and per-protocol runner configuration are owned by later
//! phases, but the shell must discover and validate them now so a corrupt file
//! is visible instead of failing silently later.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::atomic::quarantine;
use crate::paths::ProfilePaths;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolConfigAsset {
    pub protocol: String,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuxiliaryAssets {
    pub data_sources_present: bool,
    pub data_sources: Value,
    pub protocol_configs: Vec<ProtocolConfigAsset>,
    pub warnings: Vec<String>,
}

impl Default for AuxiliaryAssets {
    fn default() -> Self {
        Self {
            data_sources_present: false,
            data_sources: Value::Array(Vec::new()),
            protocol_configs: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

pub fn load_assets(paths: &ProfilePaths) -> AuxiliaryAssets {
    let mut assets = AuxiliaryAssets::default();

    if paths.data_sources_file.is_file() {
        assets.data_sources_present = true;
        match load_json(&paths.data_sources_file) {
            Ok(value) => assets.data_sources = value,
            Err(error) => assets.warnings.push(format!("data sources: {error}")),
        }
    }

    if let Ok(entries) = std::fs::read_dir(&paths.protocols_dir) {
        let mut files: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            })
            .collect();
        files.sort();
        for path in files {
            let protocol = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("unknown")
                .to_string();
            match load_json(&path) {
                Ok(value) => assets
                    .protocol_configs
                    .push(ProtocolConfigAsset { protocol, value }),
                Err(error) => assets
                    .warnings
                    .push(format!("{protocol} runner configuration: {error}")),
            }
        }
    }

    assets
}

fn load_json(path: &Path) -> Result<Value, String> {
    let bytes =
        std::fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| {
        let quarantined = quarantine(path).ok();
        format!(
            "{} is corrupt ({error}); quarantined as {}",
            path.display(),
            quarantined
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "unavailable".to_string())
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::{ProfileMode, ProfilePaths};
    use tempfile::TempDir;

    fn paths(root: &Path) -> ProfilePaths {
        let paths = ProfilePaths {
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
        };
        paths.ensure_layout().unwrap();
        paths
    }

    #[test]
    fn missing_assets_are_empty() {
        let dir = TempDir::new().unwrap();
        let assets = load_assets(&paths(dir.path()));
        assert!(!assets.data_sources_present);
        assert!(assets.protocol_configs.is_empty());
        assert!(assets.warnings.is_empty());
    }

    #[test]
    fn valid_data_sources_and_runner_configs_are_loaded() {
        let dir = TempDir::new().unwrap();
        let paths = paths(dir.path());
        std::fs::write(&paths.data_sources_file, r#"[{"name":"prod"}]"#).unwrap();
        std::fs::write(paths.protocols_dir.join("RDP.json"), r#"{"runners":[]}"#).unwrap();
        std::fs::write(paths.protocols_dir.join("SSH.json"), r#"{"runners":[]}"#).unwrap();
        std::fs::write(paths.protocols_dir.join("notes.txt"), "ignored").unwrap();

        let assets = load_assets(&paths);
        assert!(assets.data_sources_present);
        assert_eq!(assets.data_sources[0]["name"], "prod");
        assert_eq!(assets.protocol_configs.len(), 2);
        assert_eq!(assets.protocol_configs[0].protocol, "RDP");
        assert!(assets.warnings.is_empty());
    }

    #[test]
    fn corrupt_assets_are_quarantined_and_reported() {
        let dir = TempDir::new().unwrap();
        let paths = paths(dir.path());
        std::fs::write(&paths.data_sources_file, "{broken").unwrap();
        std::fs::write(paths.protocols_dir.join("RDP.json"), "{broken").unwrap();

        let assets = load_assets(&paths);
        assert_eq!(assets.warnings.len(), 2);
        assert!(!paths.data_sources_file.exists());
        assert!(!paths.protocols_dir.join("RDP.json").exists());
        let quarantined = std::fs::read_dir(&paths.protocols_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"))
            .count();
        assert_eq!(quarantined, 1);
    }
}
