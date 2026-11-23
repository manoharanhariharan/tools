//! Locality data persisted beside the profile: window/UI state, tags,
//! connection history and view preferences.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub const LOCALITY_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ServerView {
    Card,
    #[default]
    List,
    Tree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ServerSort {
    #[default]
    Insertion,
    Protocol,
    Name,
    Address,
    Custom,
}

/// Persisted window geometry. `monitor_name` and `layout_signature` let the
/// shell detect monitor changes and reject off-screen placements (P0-42/43).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WindowPlacement {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
    pub monitor_name: Option<String>,
    pub scale_factor: Option<f64>,
    pub layout_signature: Option<String>,
}

impl Default for WindowPlacement {
    fn default() -> Self {
        Self {
            x: 100,
            y: 100,
            width: 1200,
            height: 800,
            maximized: false,
            monitor_name: None,
            scale_factor: None,
            layout_signature: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UiLocality {
    pub server_view: ServerView,
    pub server_sort: ServerSort,
    pub column_widths: BTreeMap<String, u32>,
    pub main_window: Option<WindowPlacement>,
    pub launcher_window: Option<WindowPlacement>,
    pub prefer_last_display: bool,
    pub last_active_tab: Option<String>,
    /// Selected grid layout id (`auto` or a preset such as `2-side`).
    pub tab_layout: String,
    pub grid_mode: bool,
    pub show_tab_icons: bool,
    pub show_tab_close: bool,
    pub show_tab_reconnect: bool,
}

impl Default for UiLocality {
    fn default() -> Self {
        Self {
            server_view: ServerView::List,
            server_sort: ServerSort::Insertion,
            column_widths: BTreeMap::new(),
            main_window: None,
            launcher_window: None,
            prefer_last_display: false,
            last_active_tab: None,
            tab_layout: "auto".to_string(),
            grid_mode: false,
            show_tab_icons: true,
            show_tab_close: true,
            show_tab_reconnect: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionLocality {
    /// Per-server monitor index used for last full-screen RDP sessions.
    pub fullscreen_monitors: BTreeMap<String, i32>,
    /// Last active tab token per window label.
    pub last_tab_tokens: BTreeMap<String, String>,
    /// Placement of detached session windows keyed by window label.
    pub session_windows: BTreeMap<String, WindowPlacement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Locality {
    pub schema_version: u32,
    pub ui: UiLocality,
    pub session: SessionLocality,
}

impl Default for Locality {
    fn default() -> Self {
        Self {
            schema_version: LOCALITY_SCHEMA_VERSION,
            ui: UiLocality::default(),
            session: SessionLocality::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TagsLocality {
    pub schema_version: u32,
    pub pinned: Vec<String>,
    pub order: Vec<String>,
}

impl Default for TagsLocality {
    fn default() -> Self {
        Self {
            schema_version: LOCALITY_SCHEMA_VERSION,
            pinned: Vec::new(),
            order: Vec::new(),
        }
    }
}

impl TagsLocality {
    /// Normalises and de-duplicates pins/order using the same rules as server
    /// tags: lower-case, no `#`, trimmed.
    pub fn sanitize(&mut self) {
        self.schema_version = LOCALITY_SCHEMA_VERSION;
        let normalise = |values: &mut Vec<String>| {
            *values = values
                .drain(..)
                .map(|tag| tag.trim().trim_start_matches('#').to_lowercase())
                .filter(|tag| !tag.is_empty())
                .collect();
            let mut seen = BTreeSet::new();
            values.retain(|tag| seen.insert(tag.clone()));
        };
        normalise(&mut self.pinned);
        normalise(&mut self.order);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRecord {
    pub last_connected_at: i64,
    pub connect_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickConnectEntry {
    pub protocol: String,
    pub address: String,
    pub port: u16,
    pub username: String,
    pub last_used_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ConnectionRecords {
    pub schema_version: u32,
    pub records: BTreeMap<String, ConnectionRecord>,
    /// Newest-first quick-connect history (P5-15).
    pub quick_connect: Vec<QuickConnectEntry>,
}

impl Default for ConnectionRecords {
    fn default() -> Self {
        Self {
            schema_version: LOCALITY_SCHEMA_VERSION,
            records: BTreeMap::new(),
            quick_connect: Vec::new(),
        }
    }
}

impl ConnectionRecords {
    pub fn record_connection(&mut self, server_id: &str, timestamp: i64) {
        let record = self
            .records
            .entry(server_id.to_string())
            .or_insert(ConnectionRecord {
                last_connected_at: timestamp,
                connect_count: 0,
            });
        record.last_connected_at = timestamp;
        record.connect_count = record.connect_count.saturating_add(1);
    }

    /// Server ids ordered by most recent connection, newest first.
    pub fn recent_server_ids(&self, limit: usize) -> Vec<String> {
        let mut entries: Vec<(&String, &ConnectionRecord)> = self.records.iter().collect();
        entries.sort_by(|left, right| {
            right
                .1
                .last_connected_at
                .cmp(&left.1.last_connected_at)
                .then_with(|| left.0.cmp(right.0))
        });
        entries
            .into_iter()
            .take(limit)
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn record_quick_connect(&mut self, entry: QuickConnectEntry, limit: usize) {
        self.quick_connect.retain(|existing| {
            !(existing.protocol.eq_ignore_ascii_case(&entry.protocol)
                && existing.address == entry.address
                && existing.port == entry.port
                && existing.username == entry.username)
        });
        self.quick_connect.insert(0, entry);
        self.quick_connect.truncate(limit.clamp(1, 200));
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ListViewLocality {
    pub schema_version: u32,
    pub sort: ServerSort,
    pub group_expanded: BTreeMap<String, bool>,
    /// Server ids in user-defined order (P2-18).
    pub custom_order: Vec<String>,
    pub column_widths: BTreeMap<String, u32>,
}

impl Default for ListViewLocality {
    fn default() -> Self {
        Self {
            schema_version: LOCALITY_SCHEMA_VERSION,
            sort: ServerSort::Insertion,
            group_expanded: BTreeMap::new(),
            custom_order: Vec::new(),
            column_widths: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TreeViewLocality {
    pub schema_version: u32,
    pub expanded: BTreeSet<String>,
    pub node_order: Vec<String>,
}

impl Default for TreeViewLocality {
    fn default() -> Self {
        Self {
            schema_version: LOCALITY_SCHEMA_VERSION,
            expanded: BTreeSet::new(),
            node_order: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locality_defaults_round_trip() {
        let locality = Locality::default();
        assert_eq!(locality.ui.tab_layout, "auto");
        assert!(!locality.ui.grid_mode);
        assert!(locality.ui.show_tab_icons);
        assert!(locality.session.session_windows.is_empty());
        let json = serde_json::to_string(&locality).unwrap();
        let decoded: Locality = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, locality);
    }

    #[test]
    fn window_placement_survives_json() {
        let placement = WindowPlacement {
            x: -1200,
            y: 40,
            width: 1600,
            height: 900,
            maximized: true,
            monitor_name: Some("DELL U2720Q".into()),
            scale_factor: Some(1.5),
            layout_signature: Some("1920x1080+0,0|2560x1440+1920,0".into()),
        };
        let decoded: WindowPlacement =
            serde_json::from_str(&serde_json::to_string(&placement).unwrap()).unwrap();
        assert_eq!(decoded, placement);
    }

    #[test]
    fn tags_are_normalised_and_deduplicated() {
        let mut tags = TagsLocality {
            pinned: vec![" #Prod".into(), "prod".into(), "".into()],
            order: vec!["#Zeta".into(), "alpha".into(), "ALPHA".into()],
            ..TagsLocality::default()
        };
        tags.sanitize();
        assert_eq!(tags.pinned, vec!["prod"]);
        assert_eq!(tags.order, vec!["zeta", "alpha"]);
        assert_eq!(tags.schema_version, LOCALITY_SCHEMA_VERSION);
    }

    #[test]
    fn connection_records_are_ordered_and_counted() {
        let mut records = ConnectionRecords::default();
        records.record_connection("a", 100);
        records.record_connection("b", 300);
        records.record_connection("a", 200);
        assert_eq!(records.records["a"].connect_count, 2);
        assert_eq!(records.recent_server_ids(10), vec!["b", "a"]);
        assert_eq!(records.recent_server_ids(1), vec!["b"]);
    }

    #[test]
    fn quick_connect_history_deduplicates_and_truncates() {
        let mut records = ConnectionRecords::default();
        for index in 0..5 {
            records.record_quick_connect(
                QuickConnectEntry {
                    protocol: "rdp".into(),
                    address: format!("host{index}"),
                    port: 3389,
                    username: "admin".into(),
                    last_used_at: i64::from(index),
                },
                3,
            );
        }
        assert_eq!(records.quick_connect.len(), 3);
        assert_eq!(records.quick_connect[0].address, "host4");
        records.record_quick_connect(
            QuickConnectEntry {
                protocol: "rdp".into(),
                address: "host4".into(),
                port: 3389,
                username: "admin".into(),
                last_used_at: 99,
            },
            3,
        );
        assert_eq!(
            records
                .quick_connect
                .iter()
                .filter(|entry| entry.address == "host4")
                .count(),
            1
        );
        assert_eq!(records.quick_connect[0].last_used_at, 99);
    }

    #[test]
    fn unknown_fields_are_tolerated() {
        let locality: Locality =
            serde_json::from_str(r#"{"schemaVersion":1,"ui":{"future":true}}"#).unwrap();
        assert_eq!(locality.ui.server_view, ServerView::List);
    }
}
