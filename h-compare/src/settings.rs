//! Persisted user settings with backward-compatible defaults.

use crate::csvdiff::CsvOptions;
use crate::model::CompareOptions;
use crate::ui::theme::ThemeMode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentCompare {
    pub left: String,
    pub right: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Settings {
    pub left: String,
    pub right: String,
    pub compare: CompareOptions,
    pub csv: CsvOptions,
    pub theme: ThemeMode,
    pub tree_collapsed: bool,
    pub recent: Vec<RecentCompare>,
    pub csv_side_by_side: bool,
    pub csv_differences_only: bool,
    pub csv_options_collapsed: bool,
    pub text_options_collapsed: bool,
}

impl Settings {
    pub fn load() -> Option<Self> {
        let path = settings_path()?;
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    pub fn save(&self) -> io::Result<()> {
        let Some(path) = settings_path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(path, data)
    }
}

pub fn settings_path() -> Option<PathBuf> {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return Some(
            PathBuf::from(appdata)
                .join("h-compare")
                .join("settings.json"),
        );
    }
    if let Some(home) = std::env::var_os("HOME") {
        return Some(
            PathBuf::from(home)
                .join(".config")
                .join("h-compare")
                .join("settings.json"),
        );
    }
    None
}
