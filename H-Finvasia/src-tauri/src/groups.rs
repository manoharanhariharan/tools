//! Named watch lists ("Groups"), persisted locally.
//!
//! Deliberately file-backed and broker-free: switching a group only rewrites the
//! local watch set, which the subscription layer then reconciles within the
//! 100-instrument budget.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupFile {
    #[serde(default)]
    pub groups: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GroupSummary {
    pub name: String,
    pub count: usize,
}

pub fn file_path(config_dir: &Path) -> PathBuf {
    config_dir.join("watch-groups.json")
}

/// The live watch list is persisted separately so a restart can resume.
pub fn watch_path(config_dir: &Path) -> PathBuf {
    config_dir.join("watch.json")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WatchFile {
    #[serde(default)]
    pub keys: Vec<String>,
}

pub fn read_watch(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<WatchFile>(&text).ok())
        .map(|file| file.keys)
        .unwrap_or_default()
}

pub fn write_watch(path: &Path, keys: Vec<String>) -> Result<(), String> {
    let file = WatchFile { keys };
    write_json(path, &file)
}

fn validate_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("group name is required".into());
    }
    if trimmed.chars().count() > 32 {
        return Err("group name is limited to 32 characters".into());
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err("group name contains control characters".into());
    }
    Ok(trimmed.to_string())
}

pub fn read(path: &Path) -> GroupFile {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn write(path: &Path, file: &GroupFile) -> Result<(), String> {
    write_json(path, file)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| format!("encode: {e}"))?;
    // Write-then-rename so a crash cannot leave a half-written file.
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, text).map_err(|e| format!("write {}: {e}", temp.display()))?;
    std::fs::rename(&temp, path).map_err(|e| format!("rename into {}: {e}", path.display()))
}

pub fn list(path: &Path) -> Vec<GroupSummary> {
    read(path)
        .groups
        .into_iter()
        .map(|(name, keys)| GroupSummary {
            name,
            count: keys.len(),
        })
        .collect()
}

pub fn save(path: &Path, name: &str, keys: Vec<String>) -> Result<(), String> {
    let name = validate_name(name)?;
    let mut file = read(path);
    file.groups.insert(name, keys);
    write(path, &file)
}

pub fn load(path: &Path, name: &str) -> Result<Vec<String>, String> {
    let name = validate_name(name)?;
    read(path)
        .groups
        .remove(&name)
        .ok_or_else(|| format!("group \"{name}\" does not exist"))
}

pub fn delete(path: &Path, name: &str) -> Result<(), String> {
    let name = validate_name(name)?;
    let mut file = read(path);
    if file.groups.remove(&name).is_none() {
        return Err(format!("group \"{name}\" does not exist"));
    }
    write(path, &file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "fv-groups-{}-{tag}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn round_trips_groups() {
        let path = temp_path("roundtrip");
        save(&path, "Nifty", vec!["NSE|26000".into(), "NFO|1".into()]).unwrap();
        save(&path, "Banks", vec!["NSE|26009".into()]).unwrap();

        let mut listed = list(&path);
        listed.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].name, "Banks");
        assert_eq!(listed[1].count, 2);

        assert_eq!(load(&path, "Nifty").unwrap().len(), 2);
        delete(&path, "Nifty").unwrap();
        assert!(load(&path, "Nifty").is_err());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_bad_names() {
        let path = temp_path("names");
        assert!(save(&path, "   ", vec![]).is_err());
        assert!(save(&path, &"x".repeat(33), vec![]).is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn watch_list_round_trips_and_defaults_to_empty() {
        let path = temp_path("watch");
        assert!(read_watch(&path).is_empty());
        write_watch(&path, vec!["NSE|2885".into(), "NFO|1".into()]).unwrap();
        assert_eq!(read_watch(&path), vec!["NSE|2885", "NFO|1"]);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn missing_file_reads_as_empty() {
        let path = temp_path("missing");
        assert!(list(&path).is_empty());
        assert!(load(&path, "anything").is_err());
    }

    #[test]
    fn overwrite_replaces_the_group() {
        let path = temp_path("overwrite");
        save(&path, "G", vec!["A".into(), "B".into()]).unwrap();
        save(&path, "G", vec!["C".into()]).unwrap();
        assert_eq!(load(&path, "G").unwrap(), vec!["C"]);
        let _ = std::fs::remove_file(path);
    }
}
