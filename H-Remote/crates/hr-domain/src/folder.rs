//! Folder paths (`TreeNodes`) and migration from legacy tags.

use serde::{Deserialize, Serialize};

use crate::protocol::normalize_color;

/// A persisted folder. `path` is the full ordered hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub path: Vec<String>,
    /// Optional `#RRGGBB` / `#AARRGGBB` tint used by the folder tree.
    #[serde(default)]
    pub color_hex: String,
}

impl Folder {
    pub fn new(id: impl Into<String>, path: Vec<String>) -> Self {
        Self {
            id: id.into(),
            path: normalize_tree_nodes(path),
            color_hex: String::new(),
        }
    }

    /// Sets (or clears, with an empty value) the folder tint.
    pub fn with_color(mut self, color_hex: impl AsRef<str>) -> Self {
        self.color_hex = normalize_color(color_hex.as_ref());
        self
    }

    pub fn name(&self) -> &str {
        self.path.last().map(String::as_str).unwrap_or("")
    }

    pub fn parent_path(&self) -> Vec<String> {
        let mut parent = self.path.clone();
        parent.pop();
        parent
    }

    pub fn display(&self) -> String {
        self.path.join(" / ")
    }
}

/// A folder path represented as ordered segments.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FolderPath(pub Vec<String>);

impl FolderPath {
    pub fn new(segments: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(normalize_tree_nodes(
            segments.into_iter().map(Into::into).collect(),
        ))
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    pub fn display(&self) -> String {
        self.0.join(" / ")
    }

    pub fn parent(&self) -> Self {
        let mut segments = self.0.clone();
        segments.pop();
        Self(segments)
    }

    pub fn starts_with(&self, other: &Self) -> bool {
        self.0.len() >= other.0.len() && self.0.iter().zip(&other.0).all(|(a, b)| a == b)
    }

    pub fn validate(&self) -> Vec<String> {
        validate_tree_path(&self.0)
    }
}

/// Trims segments and removes empty ones. Order is preserved.
pub fn normalize_tree_nodes(nodes: Vec<String>) -> Vec<String> {
    nodes
        .into_iter()
        .map(|node| node.trim().replace(['/', '\\'], "-"))
        .filter(|node| !node.is_empty())
        .collect()
}

/// Validates that each segment is a usable folder name.
pub fn validate_tree_path(nodes: &[String]) -> Vec<String> {
    let mut issues = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        if node.trim().is_empty() {
            issues.push(format!("folder segment {index} is empty"));
        }
        if node.contains(['/', '\\']) {
            issues.push(format!("folder segment '{node}' contains a path separator"));
        }
        if node.chars().any(char::is_control) {
            issues.push(format!(
                "folder segment '{node}' contains control characters"
            ));
        }
        if node.chars().count() > 255 {
            issues.push(format!("folder segment '{}' is too long", node.trim()));
        }
    }
    issues
}

/// 1Remote compatibility: existing servers with tags but no folder path get
/// their tags copied into `TreeNodes` (P1-23).
pub fn migrate_tags_to_tree_nodes(tags: &[String], tree_nodes: &mut Vec<String>) {
    if tree_nodes.is_empty() && !tags.is_empty() {
        *tree_nodes = tags.to_vec();
    }
}

/// Rejects moving a folder into itself or one of its descendants (P2-37).
pub fn is_descendant_path(candidate: &[String], parent: &[String]) -> bool {
    candidate.len() > parent.len()
        && candidate
            .iter()
            .zip(parent.iter())
            .all(|(left, right)| left == right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_normalise_segments() {
        assert_eq!(
            normalize_tree_nodes(vec![" Prod ".into(), "".into(), "EU/West".into()]),
            vec!["Prod", "EU-West"]
        );
        assert!(validate_tree_path(&["Prod".into()]).is_empty());
        assert!(!validate_tree_path(&["".into()]).is_empty());
        assert!(!validate_tree_path(&["bad/seg".into()]).is_empty());
    }

    #[test]
    fn empty_path_falls_back_to_tags() {
        let mut tree = Vec::new();
        migrate_tags_to_tree_nodes(&["prod".into(), "linux".into()], &mut tree);
        assert_eq!(tree, vec!["prod", "linux"]);

        let mut existing = vec!["Custom".into()];
        migrate_tags_to_tree_nodes(&["prod".into()], &mut existing);
        assert_eq!(existing, vec!["Custom"]);
    }

    #[test]
    fn descendant_detection_prevents_cycles() {
        let root = vec!["Prod".to_string()];
        let child = vec!["Prod".to_string(), "EU".to_string()];
        assert!(is_descendant_path(&child, &root));
        assert!(!is_descendant_path(&root, &child));
        assert!(!is_descendant_path(&root, &root));
        assert!(FolderPath::new(["Prod", "EU"]).starts_with(&FolderPath::new(["Prod"])));
    }

    #[test]
    fn folder_path_helpers_work() {
        let path = FolderPath::new(["Prod", "EU"]);
        assert_eq!(path.display(), "Prod / EU");
        assert_eq!(path.parent().0, vec!["Prod"]);
        assert!(FolderPath::default().is_root());
    }

    #[test]
    fn folder_colors_are_normalized_and_optional() {
        let folder = Folder::new("1", vec!["Prod".into()]).with_color("#ff8800");
        assert_eq!(folder.color_hex, "#FF8800");
        assert_eq!(Folder::new("2", vec!["Lab".into()]).color_hex, "");
        assert_eq!(folder.clone().with_color("").color_hex, "");
        assert_eq!(folder.with_color("not-a-color").color_hex, "");
    }

    #[test]
    fn legacy_folders_without_colors_still_deserialize() {
        let folder: Folder = serde_json::from_str(r##"{"id":"1","path":["Prod"]}"##).unwrap();
        assert_eq!(folder.path, vec!["Prod".to_string()]);
        assert_eq!(folder.color_hex, "");
    }
}
