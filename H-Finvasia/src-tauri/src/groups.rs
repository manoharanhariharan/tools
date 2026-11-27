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
    /// Display order for the group-card view. Unknown/missing names are appended
    /// in map order so older files keep working without a migration.
    #[serde(default)]
    pub order: Vec<String>,
    /// Optional manual instrument order per group ("Custom" sort).
    #[serde(default)]
    pub custom: BTreeMap<String, Vec<String>>,
    /// One-time marker for the Index default card. Not used by Stocks groups.
    #[serde(default, skip_serializing_if = "is_false")]
    pub seeded: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
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

/// Index-tab containers are independent from the Stocks watch groups.
pub fn index_file_path(config_dir: &Path) -> PathBuf {
    config_dir.join("index-groups.json")
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

/// Stored order first, then any group that has no order entry yet.
fn ordered_names(file: &GroupFile) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for name in &file.order {
        if file.groups.contains_key(name) && !names.contains(name) {
            names.push(name.clone());
        }
    }
    for name in file.groups.keys() {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    names
}

fn sync_order(file: &mut GroupFile) {
    file.order = ordered_names(file);
}

fn view_from_file(file: &GroupFile) -> ContainersView {
    ContainersView {
        active: file.active.clone(),
        groups: ordered_names(file)
            .into_iter()
            .filter_map(|name| {
                file.groups.get(&name).map(|keys| GroupSummary {
                    count: keys.len(),
                    color: file.colors.get(&name).cloned(),
                    name,
                })
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
    sync_order(&mut file);
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
    sync_order(&mut file);
    write(path, &file)?;
    Ok(view_from_file(&file))
}

/// Create an empty container without changing the active watch owner.
///
/// The Stocks card view uses this so a new card starts blank instead of
/// copying (and later clearing) the live watch list.
pub fn create_empty(path: &Path, name: &str) -> Result<ContainersView, String> {
    let name = validate_name(name)?;
    let mut file = read(path);
    if file.groups.contains_key(&name) {
        return Err(format!("group \"{name}\" already exists"));
    }
    let color = default_group_color(file.groups.len());
    file.groups.insert(name.clone(), Vec::new());
    file.colors.insert(name.clone(), color);
    sync_order(&mut file);
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
    let custom = file.custom.remove(&old);
    file.groups.insert(new.clone(), keys);
    if let Some(color) = color {
        file.colors.insert(new.clone(), color);
    }
    if let Some(custom) = custom {
        file.custom.insert(new.clone(), custom);
    }
    if file.active.as_deref() == Some(old.as_str()) {
        file.active = Some(new.clone());
    }
    // Keep the renamed group in its current card position.
    for slot in &mut file.order {
        if slot == &old {
            *slot = new.clone();
        }
    }
    sync_order(&mut file);
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
    file.custom.remove(&name);
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
    sync_order(&mut file);
    write(path, &file)?;
    Ok((view_from_file(&file), next))
}

pub fn save(path: &Path, name: &str, keys: Vec<String>) -> Result<(), String> {
    let name = validate_name(name)?;
    let mut file = read(path);
    if let Some(custom) = file.custom.get_mut(&name) {
        custom.retain(|key| keys.contains(key));
        for key in &keys {
            if !custom.contains(key) {
                custom.push(key.clone());
            }
        }
    }
    file.groups.insert(name, keys);
    sync_order(&mut file);
    write(path, &file)
}

pub fn load(path: &Path, name: &str) -> Result<Vec<String>, String> {
    let name = validate_name(name)?;
    read(path)
        .groups
        .remove(&name)
        .ok_or_else(|| format!("group \"{name}\" does not exist"))
}

/// Append one instrument to a group, returning its resulting key list.
pub fn add_key(path: &Path, name: &str, key: &str) -> Result<Vec<String>, String> {
    let name = validate_name(name)?;
    let key = key.trim();
    if key.is_empty() {
        return Err("instrument key is required".into());
    }
    let mut file = read(path);
    let keys = file
        .groups
        .get_mut(&name)
        .ok_or_else(|| format!("group \"{name}\" does not exist"))?;
    let added = !keys.iter().any(|existing| existing == key);
    if added {
        keys.push(key.to_string());
    }
    let result = keys.clone();
    if added {
        if let Some(custom) = file.custom.get_mut(&name) {
            if !custom.iter().any(|existing| existing == key) {
                custom.push(key.to_string());
            }
        }
    }
    write(path, &file)?;
    Ok(result)
}

/// Remove one instrument from a group, returning its resulting key list.
pub fn remove_key(path: &Path, name: &str, key: &str) -> Result<Vec<String>, String> {
    let name = validate_name(name)?;
    let mut file = read(path);
    let keys = file
        .groups
        .get_mut(&name)
        .ok_or_else(|| format!("group \"{name}\" does not exist"))?;
    keys.retain(|existing| existing != key);
    let result = keys.clone();
    if let Some(custom) = file.custom.get_mut(&name) {
        custom.retain(|existing| existing != key);
    }
    write(path, &file)?;
    Ok(result)
}

/// Remove an instrument from every group. Used by the Market Watch global
/// remove action, which sees the union of all group members.
pub fn remove_key_everywhere(path: &Path, key: &str) -> Result<(), String> {
    let mut file = read(path);
    for keys in file.groups.values_mut() {
        keys.retain(|existing| existing != key);
    }
    for custom in file.custom.values_mut() {
        custom.retain(|existing| existing != key);
    }
    write(path, &file)
}

/// Save a manual ("Custom" sort) instrument order for one group.
///
/// Unknown keys are ignored and any group member not listed is appended in
/// insertion order, so a stale UI can never hide an instrument.
pub fn set_custom_order(path: &Path, name: &str, keys: Vec<String>) -> Result<(), String> {
    let name = validate_name(name)?;
    let mut file = read(path);
    let members = file
        .groups
        .get(&name)
        .cloned()
        .ok_or_else(|| format!("group \"{name}\" does not exist"))?;
    let members_set: std::collections::BTreeSet<&String> = members.iter().collect();
    let mut ordered: Vec<String> = Vec::new();
    for key in keys {
        if members_set.contains(&key) && !ordered.contains(&key) {
            ordered.push(key);
        }
    }
    for key in members {
        if !ordered.contains(&key) {
            ordered.push(key);
        }
    }
    file.custom.insert(name, ordered);
    write(path, &file)
}

/// Persist the card order. The caller must send every existing group exactly
/// once; this prevents a stale UI from dropping a group from the file.
pub fn reorder(path: &Path, names: Vec<String>) -> Result<ContainersView, String> {
    let mut file = read(path);
    let mut seen = std::collections::BTreeSet::new();
    for name in &names {
        if !file.groups.contains_key(name) {
            return Err(format!("group \"{name}\" does not exist"));
        }
        if !seen.insert(name.clone()) {
            return Err(format!("group \"{name}\" appears twice"));
        }
    }
    if seen.len() != file.groups.len() {
        return Err("the new order must list every group".into());
    }
    file.order = names;
    write(path, &file)?;
    Ok(view_from_file(&file))
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
    fn card_order_survives_reload_and_reorder() {
        let path = temp_path("card-order");
        ensure_default(&path, vec!["A".into()]).unwrap();
        create(&path, "Beta", vec!["B".into()]).unwrap();
        create(&path, "Gamma", vec!["C".into()]).unwrap();

        let names: Vec<String> = view(&path).groups.iter().map(|g| g.name.clone()).collect();
        assert_eq!(names, vec!["Watchlist", "Beta", "Gamma"]);

        reorder(
            &path,
            vec!["Gamma".into(), "Watchlist".into(), "Beta".into()],
        )
        .unwrap();
        let names: Vec<String> = view(&path).groups.iter().map(|g| g.name.clone()).collect();
        assert_eq!(names, vec!["Gamma", "Watchlist", "Beta"]);

        rename(&path, "Watchlist", "Core").unwrap();
        let names: Vec<String> = view(&path).groups.iter().map(|g| g.name.clone()).collect();
        assert_eq!(names, vec!["Gamma", "Core", "Beta"]);

        assert!(reorder(&path, vec!["Gamma".into()]).is_err());
        assert!(reorder(&path, vec!["Gamma".into(), "Core".into(), "Nope".into()]).is_err());
        assert!(reorder(
            &path,
            vec!["Gamma".into(), "Core".into(), "Beta".into(), "Beta".into()]
        )
        .is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn add_and_remove_keys_leave_other_groups_alone() {
        let path = temp_path("card-keys");
        ensure_default(&path, vec!["A".into()]).unwrap();
        create(&path, "Options", vec!["NFO|1".into()]).unwrap();

        assert_eq!(
            add_key(&path, "Options", "NFO|158614").unwrap(),
            vec!["NFO|1", "NFO|158614"]
        );
        assert_eq!(
            add_key(&path, "Options", "NFO|158614").unwrap(),
            vec!["NFO|1", "NFO|158614"],
            "adding the same key twice is a no-op"
        );
        assert_eq!(load(&path, DEFAULT_CONTAINER).unwrap(), vec!["A"]);
        assert_eq!(
            remove_key(&path, "Options", "NFO|1").unwrap(),
            vec!["NFO|158614"]
        );
        assert!(add_key(&path, "Missing", "NFO|1").is_err());
        assert!(add_key(&path, "Options", "  ").is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn empty_groups_do_not_steal_the_active_watch() {
        let path = temp_path("empty-card");
        ensure_default(&path, vec!["A".into()]).unwrap();
        let view = create_empty(&path, "New card").unwrap();
        assert_eq!(view.active.as_deref(), Some(DEFAULT_CONTAINER));
        assert_eq!(load(&path, "New card").unwrap(), Vec::<String>::new());
        assert!(create_empty(&path, "New card").is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn custom_order_tracks_add_remove_rename_and_missing_keys() {
        let path = temp_path("custom-order");
        ensure_default(&path, vec!["A".into(), "B".into()]).unwrap();
        add_key(&path, DEFAULT_CONTAINER, "C").unwrap();
        set_custom_order(
            &path,
            DEFAULT_CONTAINER,
            vec!["C".into(), "A".into(), "Nope".into()],
        )
        .unwrap();
        assert_eq!(
            read(&path).custom.get(DEFAULT_CONTAINER).unwrap(),
            &vec!["C".to_string(), "A".to_string(), "B".to_string()],
            "unknown keys are ignored and missing members are appended"
        );

        add_key(&path, DEFAULT_CONTAINER, "D").unwrap();
        assert_eq!(
            read(&path).custom.get(DEFAULT_CONTAINER).unwrap(),
            &vec![
                "C".to_string(),
                "A".to_string(),
                "B".to_string(),
                "D".to_string()
            ]
        );

        remove_key(&path, DEFAULT_CONTAINER, "A").unwrap();
        assert_eq!(
            read(&path).custom.get(DEFAULT_CONTAINER).unwrap(),
            &vec!["C".to_string(), "B".to_string(), "D".to_string()]
        );

        rename(&path, DEFAULT_CONTAINER, "Renamed").unwrap();
        assert!(!read(&path).custom.contains_key(DEFAULT_CONTAINER));
        assert_eq!(
            read(&path).custom.get("Renamed").unwrap(),
            &vec!["C".to_string(), "B".to_string(), "D".to_string()]
        );

        create_empty(&path, "Other").unwrap();
        remove(&path, "Renamed").unwrap();
        assert!(!read(&path).custom.contains_key("Renamed"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn removing_everywhere_cleans_all_groups_and_custom_orders() {
        let path = temp_path("remove-everywhere");
        ensure_default(&path, vec!["A".into(), "B".into()]).unwrap();
        create_empty(&path, "Other").unwrap();
        add_key(&path, "Other", "B").unwrap();
        set_custom_order(&path, "Other", vec!["B".into()]).unwrap();
        remove_key_everywhere(&path, "B").unwrap();
        assert_eq!(load(&path, DEFAULT_CONTAINER).unwrap(), vec!["A"]);
        assert_eq!(load(&path, "Other").unwrap(), Vec::<String>::new());
        assert!(!read(&path)
            .custom
            .get("Other")
            .map(|keys| keys.contains(&"B".to_string()))
            .unwrap_or(false));
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
