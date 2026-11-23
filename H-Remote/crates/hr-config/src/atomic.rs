//! Crash-safe JSON persistence.
//!
//! Writes go to a temporary file in the destination directory, are flushed to
//! disk, and are then renamed over the target. Unreadable files are preserved
//! with a `.corrupt-<timestamp>` suffix instead of being silently destroyed.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hr_core::{AppError, AppResult};
use serde::Serialize;
use serde::de::DeserializeOwned;

const MAX_WRITE_ATTEMPTS: u32 = 5;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct LoadOutcome<T> {
    pub value: T,
    pub recovered_from: Option<PathBuf>,
}

/// Writes JSON atomically with bounded retries for transient Windows sharing
/// violations.
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    write_bytes_atomic(path, &bytes)
}

pub fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::validation(format!("path {} has no parent", path.display())))?;
    std::fs::create_dir_all(parent)?;

    let mut last_error = None;
    for attempt in 0..MAX_WRITE_ATTEMPTS {
        match attempt_write(path, parent, bytes) {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                if attempt + 1 < MAX_WRITE_ATTEMPTS {
                    std::thread::sleep(Duration::from_millis(25 * u64::from(attempt + 1)));
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| AppError::internal("atomic write failed without an error")))
}

fn attempt_write(path: &Path, parent: &Path, bytes: &[u8]) -> AppResult<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("data.json");
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = parent.join(format!(
        ".{file_name}.tmp-{}-{}-{counter}",
        std::process::id(),
        unix_millis()
    ));

    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.map_err(AppError::from)?;

    // Best-effort directory flush so the rename itself is durable on POSIX.
    #[cfg(unix)]
    {
        if let Ok(directory) = std::fs::File::open(parent) {
            let _ = directory.sync_all();
        }
    }
    Ok(())
}

/// Reads JSON, tolerating missing and corrupted files. Corrupted files are
/// quarantined and the caller receives the default value.
pub fn read_json_or_default<T>(path: &Path) -> AppResult<LoadOutcome<T>>
where
    T: DeserializeOwned + Default,
{
    match std::fs::read(path) {
        Ok(bytes) => {
            // Editors and interrupted copies can leave a BOM or a whitespace-only
            // file; treat both as "no data" rather than logging a JSON error on
            // every start.
            let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
            if bytes.iter().all(u8::is_ascii_whitespace) {
                let quarantined = quarantine(path).ok();
                tracing::warn!(
                    path = %path.display(),
                    quarantined = ?quarantined,
                    "configuration file is empty; using defaults"
                );
                return Ok(LoadOutcome {
                    value: T::default(),
                    recovered_from: Some(quarantined.unwrap_or_else(|| path.to_path_buf())),
                });
            }
            match serde_json::from_slice::<T>(bytes) {
                Ok(value) => Ok(LoadOutcome {
                    value,
                    recovered_from: None,
                }),
                Err(error) => {
                    let quarantined = quarantine(path).ok();
                    tracing::error!(
                        path = %path.display(),
                        %error,
                        quarantined = ?quarantined,
                        "configuration file is corrupt; using defaults"
                    );
                    Ok(LoadOutcome {
                        value: T::default(),
                        recovered_from: Some(quarantined.unwrap_or_else(|| path.to_path_buf())),
                    })
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(LoadOutcome {
            value: T::default(),
            recovered_from: None,
        }),
        Err(error) => Err(AppError::from(error)),
    }
}

pub(crate) fn quarantine(path: &Path) -> std::io::Result<PathBuf> {
    let target = path.with_file_name(format!(
        "{}.corrupt-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("data.json"),
        unix_millis()
    ));
    std::fs::rename(path, &target)?;
    Ok(target)
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use tempfile::TempDir;

    #[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct Sample {
        value: u32,
        name: String,
    }

    #[test]
    fn writes_and_reads_json() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        let sample = Sample {
            value: 7,
            name: "seven".to_string(),
        };
        write_json_atomic(&path, &sample).unwrap();
        let loaded: LoadOutcome<Sample> = read_json_or_default(&path).unwrap();
        assert_eq!(loaded.value, sample);
        assert!(loaded.recovered_from.is_none());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn missing_file_yields_default() {
        let dir = TempDir::new().unwrap();
        let loaded: LoadOutcome<Sample> =
            read_json_or_default(&dir.path().join("missing.json")).unwrap();
        assert_eq!(loaded.value, Sample::default());
        assert!(loaded.recovered_from.is_none());
    }

    #[test]
    fn corrupt_file_is_quarantined_and_replaced_by_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, b"{ not json").unwrap();

        let loaded: LoadOutcome<Sample> = read_json_or_default(&path).unwrap();
        assert_eq!(loaded.value, Sample::default());
        let quarantined = loaded.recovered_from.expect("quarantine path");
        assert!(quarantined.is_file());
        assert!(quarantined.to_string_lossy().contains(".corrupt-"));
        assert_eq!(std::fs::read(&quarantined).unwrap(), b"{ not json");
        assert!(!path.exists());
    }

    #[test]
    fn empty_or_bom_prefixed_file_yields_default_without_json_error() {
        for bytes in [
            b"".as_slice(),
            b"  \r\n".as_slice(),
            b"\xEF\xBB\xBF".as_slice(),
        ] {
            let dir = TempDir::new().unwrap();
            let path = dir.path().join("config.json");
            std::fs::write(&path, bytes).unwrap();
            let loaded: LoadOutcome<Sample> = read_json_or_default(&path).unwrap();
            assert_eq!(loaded.value, Sample::default());
            assert!(loaded.recovered_from.is_some());
            assert!(!path.exists());
        }

        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, b"\xEF\xBB\xBF{\"value\":3,\"name\":\"bom\"}").unwrap();
        let loaded: LoadOutcome<Sample> = read_json_or_default(&path).unwrap();
        assert_eq!(loaded.value.value, 3);
        assert_eq!(loaded.value.name, "bom");
    }

    #[test]
    fn overwrite_is_atomic_and_leaves_no_temp_files() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        write_json_atomic(
            &path,
            &Sample {
                value: 1,
                name: "one".into(),
            },
        )
        .unwrap();
        write_json_atomic(
            &path,
            &Sample {
                value: 2,
                name: "two".into(),
            },
        )
        .unwrap();

        let loaded: LoadOutcome<Sample> = read_json_or_default(&path).unwrap();
        assert_eq!(loaded.value.value, 2);
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.contains(".tmp-"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "temporary files leaked: {leftovers:?}"
        );
    }

    #[test]
    fn write_failure_is_reported_without_panicking() {
        let dir = TempDir::new().unwrap();
        let not_a_directory = dir.path().join("blocker");
        std::fs::write(&not_a_directory, b"x").unwrap();
        let path = not_a_directory.join("config.json");
        let error = write_json_atomic(
            &path,
            &Sample {
                value: 1,
                name: "x".into(),
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            AppError::Io(_) | AppError::PermissionDenied(_)
        ));
    }

    #[test]
    fn concurrent_writers_do_not_corrupt_the_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        let mut handles = Vec::new();
        for index in 0..8u32 {
            let path = path.clone();
            handles.push(std::thread::spawn(move || {
                for round in 0..10 {
                    write_json_atomic(
                        &path,
                        &Sample {
                            value: index * 100 + round,
                            name: format!("{index}-{round}"),
                        },
                    )
                    .unwrap();
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        let loaded: LoadOutcome<Sample> = read_json_or_default(&path).unwrap();
        assert!(!loaded.value.name.is_empty());
    }
}
