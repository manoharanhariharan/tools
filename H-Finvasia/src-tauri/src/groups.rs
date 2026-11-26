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
    /// Optional `#RRGGBB` per group, independent of the watch list.
    #[serde(default)]
    pub colors: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GroupSummary {
    pub name: String,
    pub count: usize,
    pub color: Option<String>,
}

/// Containers plus the one that currently owns the live watch list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContainersView {
    pub active: Option<String>,
    pub groups: Vec<GroupSummary>,
}

pub const DEFAULT_CONTAINER: &str = "Watchlist";

const GROUP_COLORS: &[&str] = &[
    "#4B9BD8", "#3DC77E", "#D9A441", "#B05BD8", "#E25C5C", "#0F9D8A",
];

fn default_group_color(index: usize) -> String {
    GROUP_COLORS[index % GROUP_COLORS.len()].to_string()
}

fn normalize_color(color: &str) -> Result<String, String> {
    let trimmed = color.trim();
    let hex = trimmed
        .strip_prefix('#')
        .ok_or_else(|| "color must be #RRGGBB".to_string())?;
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("color must be #RRGGBB".into());
    }
    Ok(format!("#{}", hex.to_ascii_uppercase()))
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
                color: file.colors.get(name).cloned(),
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
        file.colors
            .entry(DEFAULT_CONTAINER.to_string())
            .or_insert_with(|| default_group_color(0));
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
                    .ok_or_else(|| "no groups".to_string())?
            };
            file.groups.insert(name.clone(), current);
            file.active = Some(name);
        }
    }
    // Fill colors for groups created before colors existed.
    for (index, name) in file
        .groups
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .iter()
        .enumerate()
    {
        file.colors
            .entry(name.clone())
            .or_insert_with(|| default_group_color(index));
    }
    write(path, &file)?;
    Ok(view_from_file(&file))
}

/// Create a container from the supplied keys and make it active.
pub fn create(path: &Path, name: &str, keys: Vec<String>) -> Result<ContainersView, String> {
    let name = validate_name(name)?;
    let mut file = read(path);
    if file.groups.contains_key(&name) {
        return Err(format!("group \"{name}\" already exists"));
    }
    let color = default_group_color(file.groups.len());
    file.groups.insert(name.clone(), keys);
    file.colors.insert(name.clone(), color);
    file.active = Some(name);
    write(path, &file)?;
    Ok(view_from_file(&file))
}

/// Rename a container. The active pointer and color follow the rename.
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
        .ok_or_else(|| format!("group \"{old}\" does not exist"))?;
    if file.groups.contains_key(&new) {
        file.groups.insert(old.clone(), keys);
        return Err(format!("group \"{new}\" already exists"));
    }
    let color = file.colors.remove(&old);
    file.groups.insert(new.clone(), keys);
    if let Some(color) = color {
        file.colors.insert(new.clone(), color);
    }
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
        return Err(format!("group \"{name}\" does not exist"));
    }
    file.active = Some(name);
    write(path, &file)
}

/// Set a group's display color.
pub fn set_color(path: &Path, name: &str, color: &str) -> Result<ContainersView, String> {
    let name = validate_name(name)?;
    let color = normalize_color(color)?;
    let mut file = read(path);
    if !file.groups.contains_key(&name) {
        return Err(format!("group \"{name}\" does not exist"));
    }
    file.colors.insert(name, color);
    write(path, &file)?;
    Ok(view_from_file(&file))
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
        return Err(format!("group \"{name}\" does not exist"));
    }
    if file.groups.len() == 1 {
        return Err("cannot delete the last group".into());
    }
    let was_active = file.active.as_deref() == Some(name.as_str());
    file.groups.remove(&name);
    file.colors.remove(&name);
    let next = if was_active {
        let next = file
            .groups
            .keys()
            .next()
            .cloned()
            .ok_or_else(|| "no groups".to_string())?;
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

    #[test]
    fn group_colors_follow_create_rename_and_delete() {
        let path = temp_path("containers-colors");
        let view = ensure_default(&path, vec!["A".into()]).unwrap();
        assert!(view.groups[0].color.is_some());

        create(&path, "Colored", vec!["B".into()]).unwrap();
        let view = set_color(&path, "Colored", "#12AB34").unwrap();
        let colored = view.groups.iter().find(|g| g.name == "Colored").unwrap();
        assert_eq!(colored.color.as_deref(), Some("#12AB34"));

        let view = rename(&path, "Colored", "Moved").unwrap();
        let moved = view.groups.iter().find(|g| g.name == "Moved").unwrap();
        assert_eq!(moved.color.as_deref(), Some("#12AB34"));

        let (view, _) = remove(&path, "Moved").unwrap();
        assert!(view.groups.iter().all(|g| g.name != "Moved"));
        assert!(set_color(&path, "Moved", "#000000").is_err());
        assert!(set_color(&path, DEFAULT_CONTAINER, "red").is_err());
        let _ = std::fs::remove_file(path);
    }
}
