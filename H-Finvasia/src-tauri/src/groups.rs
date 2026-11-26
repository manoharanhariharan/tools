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
    /// Name of the container the live watch list belongs to.
    #[serde(default)]
    pub active: Option<String>,
    #[serde(default)]
    pub groups: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GroupSummary {
    pub name: String,
    pub count: usize,
}

/// Containers plus the one that currently owns the live watch list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContainersView {
    pub active: Option<String>,
    pub groups: Vec<GroupSummary>,
}

pub const DEFAULT_CONTAINER: &str = "Watchlist";

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
    view(path).groups
}

fn view_from_file(file: &GroupFile) -> ContainersView {
    ContainersView {
        active: file.active.clone(),
        groups: file
            .groups
            .iter()
            .map(|(name, keys)| GroupSummary {
                name: name.clone(),
                count: keys.len(),
            })
            .collect(),
    }
}

pub fn view(path: &Path) -> ContainersView {
    view_from_file(&read(path))
}

/// Make sure at least one container exists, and that `active` points at one.
/// On first use the live watch list becomes `Watchlist`. This is deliberately
/// conservative: existing groups are never overwritten unless there was no
/// valid active container to begin with.
pub fn ensure_default(path: &Path, current: Vec<String>) -> Result<ContainersView, String> {
    let mut file = read(path);
    if file.groups.is_empty() {
        file.groups.insert(DEFAULT_CONTAINER.to_string(), current);
        file.active = Some(DEFAULT_CONTAINER.to_string());
    } else {
        let valid = file
            .active
            .as_ref()
            .map(|name| file.groups.contains_key(name))
            .unwrap_or(false);
        if !valid {
            let name = if file.groups.contains_key(DEFAULT_CONTAINER) {
                DEFAULT_CONTAINER.to_string()
            } else {
                file.groups
                    .keys()
                    .next()
                    .cloned()
                    .ok_or_else(|| "no containers".to_string())?
            };
            file.groups.insert(name.clone(), current);
            file.active = Some(name);
        }
    }
    write(path, &file)?;
    Ok(view_from_file(&file))
}

/// Create a container from the supplied keys and make it active.
pub fn create(path: &Path, name: &str, keys: Vec<String>) -> Result<ContainersView, String> {
    let name = validate_name(name)?;
    let mut file = read(path);
    if file.groups.contains_key(&name) {
        return Err(format!("container \"{name}\" already exists"));
    }
    file.groups.insert(name.clone(), keys);
    file.active = Some(name);
    write(path, &file)?;
    Ok(view_from_file(&file))
}

/// Rename a container. The active pointer follows the rename.
pub fn rename(path: &Path, old: &str, new: &str) -> Result<ContainersView, String> {
    let old = validate_name(old)?;
    let new = validate_name(new)?;
    if old == new {
        return Ok(view(path));
    }
    let mut file = read(path);
    let keys = file
        .groups
        .remove(&old)
        .ok_or_else(|| format!("container \"{old}\" does not exist"))?;
    if file.groups.contains_key(&new) {
        file.groups.insert(old, keys);
        return Err(format!("container \"{new}\" already exists"));
    }
    file.groups.insert(new.clone(), keys);
    if file.active.as_deref() == Some(old.as_str()) {
        file.active = Some(new);
    }
    write(path, &file)?;
    Ok(view_from_file(&file))
}

/// Point the active marker at an existing container without changing the watch
/// list (the caller does that separately).
pub fn set_active(path: &Path, name: &str) -> Result<(), String> {
    let name = validate_name(name)?;
    let mut file = read(path);
    if !file.groups.contains_key(&name) {
        return Err(format!("container \"{name}\" does not exist"));
    }
    file.active = Some(name);
    write(path, &file)
}

/// Write the live watch list into the active container.
pub fn save_active(path: &Path, keys: Vec<String>) -> Result<(), String> {
    let mut file = read(path);
    let Some(active) = file.active.clone() else {
        return Ok(());
    };
    if !file.groups.contains_key(&active) {
        return Ok(());
    }
    file.groups.insert(active, keys);
    write(path, &file)
}

/// Remove a container, returning its keys. When the active container is
/// removed the first remaining container becomes active; the caller decides
/// what to do with the live watch list.
pub fn remove(path: &Path, name: &str) -> Result<(ContainersView, Option<String>), String> {
    let name = validate_name(name)?;
    let mut file = read(path);
    if !file.groups.contains_key(&name) {
        return Err(format!("container \"{name}\" does not exist"));
    }
    if file.groups.len() == 1 {
        return Err("cannot delete the last container".into());
    }
    let was_active = file.active.as_deref() == Some(name.as_str());
    file.groups.remove(&name);
    let next = if was_active {
        let next = file
            .groups
            .keys()
            .next()
            .cloned()
            .ok_or_else(|| "no containers".to_string())?;
        file.active = Some(next.clone());
        Some(next)
    } else {
        None
    };
    write(path, &file)?;
    Ok((view_from_file(&file), next))
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
        remove(&path, "Nifty").unwrap();
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

    #[test]
    fn containers_start_with_the_live_watch_list() {
        let path = temp_path("containers-init");
        let view = ensure_default(&path, vec!["NSE|2885".into()]).unwrap();
        assert_eq!(view.active.as_deref(), Some(DEFAULT_CONTAINER));
        assert_eq!(view.groups.len(), 1);
        assert_eq!(view.groups[0].count, 1);

        // A second call must not duplicate or overwrite the container.
        let view = ensure_default(&path, vec!["NSE|22".into()]).unwrap();
        assert_eq!(view.groups.len(), 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn create_rename_and_save_active_keep_one_pointer() {
        let path = temp_path("containers-edit");
        ensure_default(&path, vec!["A".into()]).unwrap();
        create(&path, "Banks", vec!["B".into(), "C".into()]).unwrap();
        assert_eq!(view(&path).active.as_deref(), Some("Banks"));

        rename(&path, "Banks", "Financials").unwrap();
        assert_eq!(view(&path).active.as_deref(), Some("Financials"));

        save_active(&path, vec!["D".into()]).unwrap();
        assert_eq!(load(&path, "Financials").unwrap(), vec!["D"]);
        assert!(create(&path, "Financials", vec![]).is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn removing_the_active_container_promotes_another() {
        let path = temp_path("containers-remove");
        ensure_default(&path, vec!["A".into()]).unwrap();
        create(&path, "Second", vec!["B".into()]).unwrap();
        let (view, next) = remove(&path, "Second").unwrap();
        assert_eq!(next.as_deref(), Some(DEFAULT_CONTAINER));
        assert_eq!(view.active.as_deref(), Some(DEFAULT_CONTAINER));
        assert_eq!(view.groups.len(), 1);
        assert!(remove(&path, DEFAULT_CONTAINER).is_err(), "last one stays");
        let _ = std::fs::remove_file(path);
    }
}
