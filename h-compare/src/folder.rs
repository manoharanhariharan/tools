//! Recursive folder scanning and entry comparison.

use crate::model::{
    join_rel, CompareOptions, CompareTree, Counts, EntryInfo, EntryStatus, Node, COMMON_SKIP_DIRS,
};
use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use walkdir::WalkDir;

#[derive(Debug)]
struct ScannedEntry {
    is_dir: bool,
    size: u64,
    modified: Option<std::time::SystemTime>,
    path: PathBuf,
}

#[derive(Debug, Default)]
struct SideScan {
    entries: BTreeMap<String, ScannedEntry>,
    warnings: Vec<String>,
}

struct BuildNode {
    name: String,
    rel: String,
    depth: usize,
    parent: Option<usize>,
    is_dir: bool,
    left: Option<EntryInfo>,
    right: Option<EntryInfo>,
    children: BTreeMap<String, usize>,
}

/// Compare two directory trees. This is intentionally side-effect free and
/// suitable for running on a worker thread.
pub fn compare_folders(
    left_root: &Path,
    right_root: &Path,
    options: &CompareOptions,
) -> Result<CompareTree, String> {
    let started = Instant::now();
    for (label, root) in [("Left", left_root), ("Right", right_root)] {
        if !root.exists() {
            return Err(format!("{label} folder does not exist: {}", root.display()));
        }
        if !root.is_dir() {
            return Err(format!("{label} path is not a folder: {}", root.display()));
        }
    }

    let (left_scan, right_scan) = rayon::join(
        || scan_side(left_root, options.skip_build_dirs),
        || scan_side(right_root, options.skip_build_dirs),
    );
    let mut warnings = left_scan.warnings;
    warnings.extend(right_scan.warnings);

    let mut nodes = Vec::new();
    let root_meta = |path: &Path| EntryInfo {
        is_dir: true,
        size: 0,
        modified: fs::metadata(path)
            .ok()
            .and_then(|meta| meta.modified().ok()),
        path: path.to_path_buf(),
    };
    nodes.push(BuildNode {
        name: "Comparison".to_string(),
        rel: String::new(),
        depth: 0,
        parent: None,
        is_dir: true,
        left: Some(root_meta(left_root)),
        right: Some(root_meta(right_root)),
        children: BTreeMap::new(),
    });

    insert_side(&mut nodes, &left_scan.entries, true);
    insert_side(&mut nodes, &right_scan.entries, false);

    // Compare common regular files in parallel. A size check is done first, so
    // only genuinely interesting files require reading.
    let pairs: Vec<(String, PathBuf, PathBuf)> = left_scan
        .entries
        .iter()
        .filter_map(|(rel, left)| {
            let right = right_scan.entries.get(rel)?;
            if left.is_dir || right.is_dir {
                return None;
            }
            Some((rel.clone(), left.path.clone(), right.path.clone()))
        })
        .collect();

    let equality: HashMap<String, Result<bool, String>> = pairs
        .par_iter()
        .map(|(rel, left, right)| {
            let result = files_equal(left, right)
                .map_err(|err| format!("Could not read {rel} or its counterpart: {err}"));
            (rel.clone(), result)
        })
        .collect();

    let mut computed = vec![None; nodes.len()];
    compute_status(0, &mut nodes, &equality, &mut computed, &mut warnings);

    let mut final_nodes = Vec::with_capacity(nodes.len());
    for (idx, node) in nodes.into_iter().enumerate() {
        let Some((status, counts, note)) = computed[idx].take() else {
            continue;
        };
        final_nodes.push(Node {
            name: node.name,
            rel: node.rel,
            depth: node.depth,
            is_dir: node.is_dir,
            status,
            left: node.left,
            right: node.right,
            children: node.children.into_values().collect(),
            parent: node.parent,
            counts,
            note,
        });
    }

    warnings.truncate(200);
    Ok(CompareTree::new(
        final_nodes,
        0,
        left_root.to_path_buf(),
        right_root.to_path_buf(),
        started.elapsed().max(Duration::from_millis(1)),
        warnings,
    ))
}

fn scan_side(root: &Path, skip_build_dirs: bool) -> SideScan {
    let mut side = SideScan::default();
    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if !skip_build_dirs || entry.depth() == 0 || !entry.file_type().is_dir() {
                return true;
            }
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            !COMMON_SKIP_DIRS.contains(&name.as_str())
        });

    for item in walker {
        let entry = match item {
            Ok(entry) => entry,
            Err(err) => {
                if side.warnings.len() < 100 {
                    side.warnings.push(format!("Skipped an entry: {err}"));
                }
                continue;
            }
        };
        if entry.depth() == 0 || entry.file_type().is_symlink() {
            continue;
        }
        let rel = match entry.path().strip_prefix(root) {
            Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        if rel.is_empty() {
            continue;
        }
        let is_dir = entry.file_type().is_dir();
        if !is_dir && (rel.ends_with(".hcompare.bak") || rel.ends_with(".hcompare-tmp")) {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(err) => {
                if side.warnings.len() < 100 {
                    side.warnings.push(format!("Could not stat {rel}: {err}"));
                }
                continue;
            }
        };
        side.entries.insert(
            rel,
            ScannedEntry {
                is_dir,
                size: if is_dir { 0 } else { metadata.len() },
                modified: metadata.modified().ok(),
                path: entry.path().to_path_buf(),
            },
        );
    }
    side
}

fn insert_side(nodes: &mut Vec<BuildNode>, entries: &BTreeMap<String, ScannedEntry>, left: bool) {
    for (rel, entry) in entries {
        let parts: Vec<&str> = rel.split('/').collect();
        let mut parent = 0usize;
        for (depth, part) in parts.iter().enumerate() {
            let is_last = depth + 1 == parts.len();
            let child_rel = if nodes[parent].rel.is_empty() {
                (*part).to_string()
            } else {
                join_rel(&nodes[parent].rel, part)
            };
            let child_idx = if let Some(idx) = nodes[parent].children.get(*part) {
                *idx
            } else {
                let idx = nodes.len();
                nodes.push(BuildNode {
                    name: (*part).to_string(),
                    rel: child_rel.clone(),
                    depth: depth + 1,
                    parent: Some(parent),
                    is_dir: !is_last || entry.is_dir,
                    left: None,
                    right: None,
                    children: BTreeMap::new(),
                });
                nodes[parent].children.insert((*part).to_string(), idx);
                idx
            };
            if is_last {
                let info = EntryInfo {
                    is_dir: entry.is_dir,
                    size: entry.size,
                    modified: entry.modified,
                    path: entry.path.clone(),
                };
                if left {
                    nodes[child_idx].left = Some(info);
                } else {
                    nodes[child_idx].right = Some(info);
                }
                nodes[child_idx].is_dir = entry.is_dir;
            } else {
                nodes[child_idx].is_dir = true;
            }
            parent = child_idx;
        }
    }
}

/// Status, counts and optional note for one arena node.
type Computed = Option<(EntryStatus, Counts, Option<String>)>;

fn compute_status(
    idx: usize,
    nodes: &mut Vec<BuildNode>,
    equality: &HashMap<String, Result<bool, String>>,
    computed: &mut Vec<Computed>,
    warnings: &mut Vec<String>,
) -> (EntryStatus, Counts) {
    let is_dir = nodes[idx].is_dir;
    let rel = nodes[idx].rel.clone();
    let left = nodes[idx].left.clone();
    let right = nodes[idx].right.clone();
    let children = nodes[idx].children.values().copied().collect::<Vec<_>>();

    let mut counts = Counts::default();
    let mut any_child_difference = false;
    for child in children {
        let (status, child_counts) = compute_status(child, nodes, equality, computed, warnings);
        counts.merge(child_counts);
        any_child_difference |= status.is_difference();
    }

    let mut note = None;
    let status = match (&left, &right) {
        (Some(left_info), Some(right_info)) if left_info.is_dir != right_info.is_dir => {
            note = Some("One side is a file, the other is a directory".to_string());
            EntryStatus::TypeMismatch
        }
        (None, Some(_)) => EntryStatus::RightOnly,
        (Some(_), None) => EntryStatus::LeftOnly,
        (Some(_), Some(_)) if is_dir => {
            if any_child_difference {
                EntryStatus::Different
            } else {
                EntryStatus::Same
            }
        }
        (Some(_), Some(_)) => match equality.get(&rel) {
            Some(Ok(true)) => EntryStatus::Same,
            Some(Ok(false)) => EntryStatus::Different,
            Some(Err(err)) => {
                if warnings.len() < 100 {
                    warnings.push(err.clone());
                }
                note = Some(err.clone());
                EntryStatus::Error
            }
            None => {
                note = Some("File was not compared".to_string());
                EntryStatus::Error
            }
        },
        (None, None) => EntryStatus::Error,
    };

    if !is_dir {
        counts = Counts::for_status(status);
    }
    computed[idx] = Some((status, counts, note));
    (status, counts)
}

/// Compare two files byte for byte without loading them into memory.
fn files_equal(left: &Path, right: &Path) -> io::Result<bool> {
    let left_meta = fs::metadata(left)?;
    let right_meta = fs::metadata(right)?;
    if left_meta.len() != right_meta.len() {
        return Ok(false);
    }
    let mut left_reader = BufReader::with_capacity(128 * 1024, File::open(left)?);
    let mut right_reader = BufReader::with_capacity(128 * 1024, File::open(right)?);

    loop {
        let (equal, consumed, left_done, right_done) = {
            let left_buf = left_reader.fill_buf()?;
            let right_buf = right_reader.fill_buf()?;
            let len = left_buf.len().min(right_buf.len());
            if len == 0 {
                (
                    left_buf.is_empty() == right_buf.is_empty(),
                    0,
                    left_buf.is_empty(),
                    right_buf.is_empty(),
                )
            } else {
                (
                    left_buf[..len] == right_buf[..len],
                    len,
                    left_buf.len() == len,
                    right_buf.len() == len,
                )
            }
        };
        if !equal {
            return Ok(false);
        }
        if left_done && right_done {
            return Ok(true);
        }
        left_reader.consume(consumed);
        right_reader.consume(consumed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(label: &str) -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "hcompare-test-{}-{}-{}",
                std::process::id(),
                id,
                label
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn file(&self, rel: &str, contents: &str) {
            let path = self.path.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, contents).unwrap();
        }

        fn dir(&self, rel: &str) {
            fs::create_dir_all(self.path.join(rel)).unwrap();
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn compares_files_directories_and_one_sided_entries() {
        let left = TempDir::new("left");
        let right = TempDir::new("right");
        left.file("same.txt", "hello\n");
        right.file("same.txt", "hello\n");
        left.file("changed.txt", "one\n");
        right.file("changed.txt", "two\n");
        left.file("left-only.txt", "left\n");
        right.file("right-only.txt", "right\n");
        left.file("sub/nested.txt", "nested\n");
        right.file("sub/nested.txt", "nested\n");
        left.file("sub/other.txt", "left\n");
        right.file("sub/other.txt", "right\n");
        left.dir("empty-left");
        right.dir("empty-right");

        let tree = compare_folders(&left.path, &right.path, &CompareOptions::default()).unwrap();

        assert_eq!(tree.by_rel("same.txt").unwrap().status, EntryStatus::Same);
        assert_eq!(
            tree.by_rel("changed.txt").unwrap().status,
            EntryStatus::Different
        );
        assert_eq!(
            tree.by_rel("left-only.txt").unwrap().status,
            EntryStatus::LeftOnly
        );
        assert_eq!(
            tree.by_rel("right-only.txt").unwrap().status,
            EntryStatus::RightOnly
        );
        assert_eq!(
            tree.by_rel("sub/other.txt").unwrap().status,
            EntryStatus::Different
        );
        assert_eq!(tree.by_rel("sub").unwrap().status, EntryStatus::Different);
        assert_eq!(tree.counts.same, 2);
        assert_eq!(tree.counts.different, 2);
        assert_eq!(tree.counts.left_only, 1);
        assert_eq!(tree.counts.right_only, 1);
        assert_eq!(tree.counts.total(), 6);
        assert_eq!(tree.first_difference().is_some(), true);
    }

    #[test]
    fn skips_common_build_directories_when_enabled() {
        let left = TempDir::new("skip-left");
        let right = TempDir::new("skip-right");
        left.file("node_modules/pkg/index.js", "left\n");
        left.file("src/main.ts", "same\n");
        right.file("src/main.ts", "same\n");

        let mut options = CompareOptions::default();
        options.skip_build_dirs = true;
        let tree = compare_folders(&left.path, &right.path, &options).unwrap();
        assert!(tree.by_rel("node_modules/pkg/index.js").is_none());
        assert_eq!(tree.counts.total(), 1);

        options.skip_build_dirs = false;
        let tree = compare_folders(&left.path, &right.path, &options).unwrap();
        assert_eq!(
            tree.by_rel("node_modules/pkg/index.js").unwrap().status,
            EntryStatus::LeftOnly
        );
    }

    #[test]
    fn reports_type_mismatches() {
        let left = TempDir::new("type-left");
        let right = TempDir::new("type-right");
        left.file("thing", "file\n");
        right.dir("thing");
        right.file("thing/inner.txt", "inner\n");

        let tree = compare_folders(&left.path, &right.path, &CompareOptions::default()).unwrap();
        assert_eq!(
            tree.by_rel("thing").unwrap().status,
            EntryStatus::TypeMismatch
        );
        assert!(tree.by_rel("thing").unwrap().is_dir);
    }

    #[test]
    fn empty_files_compare_equal() {
        let left = TempDir::new("empty-left");
        let right = TempDir::new("empty-right");
        left.file("empty", "");
        right.file("empty", "");
        let tree = compare_folders(&left.path, &right.path, &CompareOptions::default()).unwrap();
        assert_eq!(tree.by_rel("empty").unwrap().status, EntryStatus::Same);
    }
}
