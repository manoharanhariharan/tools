//! Folder comparison model, statuses, counts and formatting helpers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Result of comparing a single entry (file or directory) on both sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
pub enum EntryStatus {
    #[default]
    Same,
    Different,
    LeftOnly,
    RightOnly,
    TypeMismatch,
    Error,
}

impl EntryStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Same => "Same",
            Self::Different => "Different",
            Self::LeftOnly => "Left only",
            Self::RightOnly => "Right only",
            Self::TypeMismatch => "Type mismatch",
            Self::Error => "Error",
        }
    }

    pub fn is_difference(self) -> bool {
        !matches!(self, Self::Same)
    }

    /// Single glyph used in the compact tree/file list.
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Same => "=",
            Self::Different => "~",
            Self::LeftOnly => "-",
            Self::RightOnly => "+",
            Self::TypeMismatch => "!",
            Self::Error => "!",
        }
    }
}

/// File-only counters. Directories are described by the aggregate of their
/// children, which keeps the totals intuitive.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub same: usize,
    pub different: usize,
    pub left_only: usize,
    pub right_only: usize,
    pub type_mismatch: usize,
    pub error: usize,
}

impl Counts {
    pub fn add_status(&mut self, status: EntryStatus) {
        match status {
            EntryStatus::Same => self.same += 1,
            EntryStatus::Different => self.different += 1,
            EntryStatus::LeftOnly => self.left_only += 1,
            EntryStatus::RightOnly => self.right_only += 1,
            EntryStatus::TypeMismatch => self.type_mismatch += 1,
            EntryStatus::Error => self.error += 1,
        }
    }

    pub fn merge(&mut self, other: Counts) {
        self.same += other.same;
        self.different += other.different;
        self.left_only += other.left_only;
        self.right_only += other.right_only;
        self.type_mismatch += other.type_mismatch;
        self.error += other.error;
    }

    pub fn total(&self) -> usize {
        self.same
            + self.different
            + self.left_only
            + self.right_only
            + self.type_mismatch
            + self.error
    }

    pub fn differences(&self) -> usize {
        self.different + self.left_only + self.right_only + self.type_mismatch + self.error
    }

    pub fn for_status(status: EntryStatus) -> Self {
        let mut counts = Self::default();
        counts.add_status(status);
        counts
    }
}

/// Options shared by folder and text comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompareOptions {
    pub ignore_case: bool,
    pub ignore_whitespace: bool,
    pub trim_trailing: bool,
    pub skip_build_dirs: bool,
}

impl Default for CompareOptions {
    fn default() -> Self {
        Self {
            ignore_case: false,
            ignore_whitespace: false,
            trim_trailing: false,
            skip_build_dirs: true,
        }
    }
}

/// Directories that are almost never interesting in a source-tree comparison.
pub const COMMON_SKIP_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    ".idea",
    ".vs",
    ".vscode",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    ".next",
    ".nuxt",
    ".cache",
];

#[derive(Debug, Clone)]
pub struct EntryInfo {
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub name: String,
    pub rel: String,
    pub depth: usize,
    pub is_dir: bool,
    pub status: EntryStatus,
    pub left: Option<EntryInfo>,
    pub right: Option<EntryInfo>,
    pub children: Vec<usize>,
    pub parent: Option<usize>,
    /// File-only aggregate below this node (including itself for files).
    pub counts: Counts,
    pub note: Option<String>,
}

impl Node {
    pub fn left_path(&self) -> Option<&Path> {
        self.left.as_ref().map(|entry| entry.path.as_path())
    }

    pub fn right_path(&self) -> Option<&Path> {
        self.right.as_ref().map(|entry| entry.path.as_path())
    }

    pub fn size_delta(&self) -> Option<i64> {
        match (&self.left, &self.right) {
            (Some(left), Some(right)) => Some(right.size as i64 - left.size as i64),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompareTree {
    pub nodes: Vec<Node>,
    pub root: usize,
    pub left_root: PathBuf,
    pub right_root: PathBuf,
    pub counts: Counts,
    pub elapsed: Duration,
    pub warnings: Vec<String>,
    index: HashMap<String, usize>,
}

impl CompareTree {
    pub fn new(
        nodes: Vec<Node>,
        root: usize,
        left_root: PathBuf,
        right_root: PathBuf,
        elapsed: Duration,
        warnings: Vec<String>,
    ) -> Self {
        let counts = nodes.get(root).map(|node| node.counts).unwrap_or_default();
        let index = nodes
            .iter()
            .enumerate()
            .map(|(idx, node)| (node.rel.clone(), idx))
            .collect();
        Self {
            nodes,
            root,
            left_root,
            right_root,
            counts,
            elapsed,
            warnings,
            index,
        }
    }

    pub fn node(&self, idx: usize) -> &Node {
        &self.nodes[idx]
    }

    pub fn by_rel(&self, rel: &str) -> Option<&Node> {
        self.index.get(rel).map(|idx| &self.nodes[*idx])
    }

    pub fn idx_by_rel(&self, rel: &str) -> Option<usize> {
        self.index.get(rel).copied()
    }

    pub fn root_node(&self) -> &Node {
        &self.nodes[self.root]
    }

    pub fn ancestors(&self, idx: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut current = self.nodes[idx].parent;
        while let Some(parent) = current {
            out.push(parent);
            current = self.nodes[parent].parent;
        }
        out.reverse();
        out
    }

    /// Files under `idx` whose status differs, sorted by relative path.
    pub fn differing_files(&self, idx: usize, limit: usize) -> Vec<usize> {
        let mut out = Vec::new();
        self.collect_files(idx, &mut out);
        out.retain(|node_idx| self.nodes[*node_idx].status.is_difference());
        out.sort_by(|a, b| self.nodes[*a].rel.cmp(&self.nodes[*b].rel));
        out.truncate(limit);
        out
    }

    /// All file nodes under `idx`, depth-first (directories first as displayed).
    pub fn files(&self, idx: usize) -> Vec<usize> {
        let mut out = Vec::new();
        self.collect_files(idx, &mut out);
        out
    }

    fn collect_files(&self, idx: usize, out: &mut Vec<usize>) {
        let node = &self.nodes[idx];
        if node.is_dir {
            for child in &node.children {
                self.collect_files(*child, out);
            }
        } else {
            out.push(idx);
        }
    }

    /// First file (in tree order) that has a difference.
    pub fn first_difference(&self) -> Option<usize> {
        self.find_difference_from(self.root, None)
    }

    pub fn next_difference(&self, current: Option<usize>) -> Option<usize> {
        let files = self.files(self.root);
        if files.is_empty() {
            return None;
        }
        let start = current
            .and_then(|current| files.iter().position(|idx| *idx == current))
            .map(|pos| pos + 1)
            .unwrap_or(0);
        files[start..]
            .iter()
            .copied()
            .find(|idx| self.nodes[*idx].status.is_difference())
    }

    pub fn previous_difference(&self, current: Option<usize>) -> Option<usize> {
        let files = self.files(self.root);
        let start = current
            .and_then(|current| files.iter().position(|idx| *idx == current))
            .unwrap_or(files.len());
        files[..start]
            .iter()
            .rev()
            .copied()
            .find(|idx| self.nodes[*idx].status.is_difference())
    }

    fn find_difference_from(&self, idx: usize, skip: Option<usize>) -> Option<usize> {
        let node = &self.nodes[idx];
        if Some(idx) == skip {
            return None;
        }
        if !node.is_dir {
            return node.status.is_difference().then_some(idx);
        }
        for child in &node.children {
            if let Some(found) = self.find_difference_from(*child, skip) {
                return Some(found);
            }
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusFilter {
    #[default]
    All,
    Differences,
    Same,
    LeftOnly,
    RightOnly,
}

impl StatusFilter {
    pub fn matches(self, status: EntryStatus) -> bool {
        match self {
            Self::All => true,
            Self::Differences => status.is_difference(),
            Self::Same => status == EntryStatus::Same,
            Self::LeftOnly => status == EntryStatus::LeftOnly,
            Self::RightOnly => status == EntryStatus::RightOnly,
        }
    }
}

pub fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else if value >= 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

pub fn format_size_delta(delta: i64) -> String {
    if delta == 0 {
        "same size".to_string()
    } else if delta > 0 {
        format!("+{}", format_size(delta as u64))
    } else {
        format!("-{}", format_size(delta.unsigned_abs()))
    }
}

pub fn format_modified(modified: Option<SystemTime>) -> String {
    let Some(modified) = modified else {
        return "unknown".to_string();
    };
    match SystemTime::now().duration_since(modified) {
        Ok(age) => format_age(age),
        Err(_) => "in the future".to_string(),
    }
}

fn format_age(age: Duration) -> String {
    let secs = age.as_secs();
    if secs < 60 {
        "just now".to_string()
    } else if secs < 60 * 60 {
        format!("{}m ago", secs / 60)
    } else if secs < 60 * 60 * 24 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}

pub fn join_rel(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_string()
    } else {
        format!("{parent}/{child}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_math() {
        let mut counts = Counts::default();
        counts.add_status(EntryStatus::Same);
        counts.add_status(EntryStatus::Different);
        counts.add_status(EntryStatus::LeftOnly);
        assert_eq!(counts.total(), 3);
        assert_eq!(counts.differences(), 2);
    }

    #[test]
    fn sizes_are_human_readable() {
        assert_eq!(format_size(999), "999 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(10 * 1024), "10.0 KB");
        assert_eq!(format_size(150 * 1024), "150 KB");
    }

    #[test]
    fn rel_paths_are_joined_with_forward_slashes() {
        assert_eq!(join_rel("", "a.txt"), "a.txt");
        assert_eq!(join_rel("dir", "a.txt"), "dir/a.txt");
    }
}
