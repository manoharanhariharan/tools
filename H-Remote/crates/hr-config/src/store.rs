//! Debounced, crash-safe JSON store used for configuration and locality files.
//!
//! A single worker thread per store coalesces bursts of updates, writes the
//! latest snapshot atomically, and is joined on drop (flushing any pending
//! value). This is the only persistence thread the application spawns for a
//! given file, which keeps lifecycle and leak behaviour deterministic.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

use hr_core::{AppError, AppResult};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::watch;

use crate::atomic::{read_json_or_default, write_json_atomic};

struct Shared<T> {
    state: Mutex<SaveState<T>>,
    condvar: Condvar,
    path: PathBuf,
    debounce: Duration,
    sender: Arc<watch::Sender<T>>,
    last_error: Mutex<Option<String>>,
    finished: Arc<AtomicBool>,
}

struct SaveState<T> {
    pending: Option<T>,
    generation: u64,
    shutdown: bool,
    saving: bool,
    flush: bool,
}

pub struct JsonStore<T> {
    shared: Arc<Shared<T>>,
    current: Arc<RwLock<T>>,
    worker: Mutex<Option<JoinHandle<()>>>,
    recovered_from: Option<PathBuf>,
}

impl<T> JsonStore<T>
where
    T: Clone + Serialize + DeserializeOwned + Default + Send + Sync + 'static,
{
    /// Opens or creates a store, returning defaults for missing/corrupt files.
    pub fn open(path: PathBuf, debounce: Duration) -> AppResult<Self> {
        Self::open_with(path, debounce, |_| {})
    }

    /// Opens a store and normalises the loaded value in memory before it is
    /// exposed (for example clamping configuration fields).
    pub fn open_with<F>(path: PathBuf, debounce: Duration, normalise: F) -> AppResult<Self>
    where
        F: FnOnce(&mut T),
    {
        let loaded = read_json_or_default::<T>(&path)?;
        let mut value = loaded.value;
        normalise(&mut value);

        let (sender, _receiver) = watch::channel(value.clone());
        let shared = Arc::new(Shared {
            state: Mutex::new(SaveState {
                pending: None,
                generation: 0,
                shutdown: false,
                saving: false,
                flush: false,
            }),
            condvar: Condvar::new(),
            path: path.clone(),
            debounce: debounce.max(Duration::from_millis(1)),
            sender: Arc::new(sender),
            last_error: Mutex::new(None),
            finished: Arc::new(AtomicBool::new(false)),
        });

        let worker_shared = Arc::clone(&shared);
        let worker = std::thread::Builder::new()
            .name("hr-config-saver".to_string())
            .spawn(move || worker_loop(worker_shared))
            .map_err(AppError::from)?;

        Ok(Self {
            shared,
            current: Arc::new(RwLock::new(value)),
            worker: Mutex::new(Some(worker)),
            recovered_from: loaded.recovered_from,
        })
    }

    pub fn path(&self) -> &Path {
        &self.shared.path
    }

    /// Path of a corrupt file that was quarantined while opening.
    pub fn recovered_from(&self) -> Option<&Path> {
        self.recovered_from.as_deref()
    }

    pub fn get(&self) -> T {
        self.current
            .read()
            .expect("configuration lock poisoned")
            .clone()
    }

    /// Applies an in-memory update and schedules a debounced save.
    pub fn update<F>(&self, mutate: F) -> T
    where
        F: FnOnce(&mut T),
    {
        let next = {
            let mut guard = self.current.write().expect("configuration lock poisoned");
            mutate(&mut guard);
            guard.clone()
        };

        {
            let mut state = self.shared.state.lock().expect("saver lock poisoned");
            state.pending = Some(next.clone());
            state.generation = state.generation.wrapping_add(1);
            state.flush = false;
        }
        self.shared.condvar.notify_all();
        next
    }

    pub fn replace(&self, value: T) -> T {
        self.update(|slot| *slot = value)
    }

    /// Subscribes to successfully persisted snapshots.
    pub fn subscribe(&self) -> watch::Receiver<T> {
        self.shared.sender.subscribe()
    }

    pub fn last_error(&self) -> Option<String> {
        self.shared
            .last_error
            .lock()
            .expect("saver lock poisoned")
            .clone()
    }

    /// Forces a save immediately and waits for it to finish. Used on shutdown
    /// and in tests; regular updates remain debounced.
    pub fn flush(&self) -> AppResult<()> {
        {
            let mut state = self.shared.state.lock().expect("saver lock poisoned");
            state.pending = Some(self.get());
            state.generation = state.generation.wrapping_add(1);
            state.flush = true;
        }
        self.shared.condvar.notify_all();

        let mut state = self.shared.state.lock().expect("saver lock poisoned");
        while state.pending.is_some() || state.saving {
            state = self
                .shared
                .condvar
                .wait(state)
                .expect("saver lock poisoned");
        }
        drop(state);

        if let Some(error) = self.last_error() {
            return Err(AppError::config(error));
        }
        Ok(())
    }

    /// Number of updates waiting to be written (tests and diagnostics).
    pub fn pending(&self) -> bool {
        self.shared
            .state
            .lock()
            .expect("saver lock poisoned")
            .pending
            .is_some()
    }

    /// Test-only handle that becomes true when the saver thread has exited.
    #[cfg(test)]
    pub fn worker_finished_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.shared.finished)
    }
}

impl<T> Drop for JsonStore<T> {
    fn drop(&mut self) {
        {
            let mut state = self.shared.state.lock().expect("saver lock poisoned");
            state.shutdown = true;
        }
        self.shared.condvar.notify_all();

        if let Some(worker) = self.worker.lock().expect("worker lock poisoned").take() {
            if worker.join().is_err() {
                tracing::error!("configuration saver thread panicked");
            }
        }
    }
}

fn worker_loop<T>(shared: Arc<Shared<T>>)
where
    T: Clone + Serialize + Send + Sync + 'static,
{
    let mut state = shared.state.lock().expect("saver lock poisoned");
    loop {
        while state.pending.is_none() && !state.shutdown {
            state = shared.condvar.wait(state).expect("saver lock poisoned");
        }
        if state.pending.is_none() {
            break;
        }

        // Coalesce bursts unless this is a forced flush or shutdown.
        if !state.flush && !state.shutdown {
            let generation = state.generation;
            let (next, _) = shared
                .condvar
                .wait_timeout(state, shared.debounce)
                .expect("saver lock poisoned");
            state = next;
            if state.generation != generation {
                continue;
            }
        }

        let Some(value) = state.pending.take() else {
            continue;
        };
        state.saving = true;
        state.flush = false;
        drop(state);

        let result = write_json_atomic(&shared.path, &value);
        match result {
            Ok(()) => {
                *shared.last_error.lock().expect("saver lock poisoned") = None;
                // Ignore send errors: no subscribers is a valid state.
                let _ = shared.sender.send(value);
            }
            Err(error) => {
                tracing::error!(
                    path = %shared.path.display(),
                    %error,
                    "configuration save failed"
                );
                *shared.last_error.lock().expect("saver lock poisoned") = Some(error.to_string());
            }
        }

        state = shared.state.lock().expect("saver lock poisoned");
        state.saving = false;
        shared.condvar.notify_all();
        if state.shutdown && state.pending.is_none() {
            break;
        }
    }
    shared.finished.store(true, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use tempfile::TempDir;

    #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(default, rename_all = "camelCase")]
    struct State {
        counter: u32,
        label: String,
    }

    fn open(dir: &TempDir, debounce_ms: u64) -> JsonStore<State> {
        JsonStore::open(
            dir.path().join("state.json"),
            Duration::from_millis(debounce_ms),
        )
        .unwrap()
    }

    fn read(dir: &TempDir) -> State {
        let bytes = std::fs::read(dir.path().join("state.json")).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn debounced_updates_are_coalesced() {
        let dir = TempDir::new().unwrap();
        let store = open(&dir, 40);
        store.update(|state| {
            state.counter = 1;
            state.label = "first".into();
        });
        store.update(|state| {
            state.counter = 2;
            state.label = "second".into();
        });
        store.flush().unwrap();
        assert_eq!(read(&dir).label, "second");
        assert!(!store.pending());
    }

    #[test]
    fn drop_flushes_pending_updates() {
        let dir = TempDir::new().unwrap();
        {
            let store = open(&dir, 500);
            store.update(|state| {
                state.counter = 7;
                state.label = "flushed-on-drop".into();
            });
        }
        assert_eq!(read(&dir).label, "flushed-on-drop");
    }

    #[test]
    fn corrupt_files_are_recovered_with_defaults() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("state.json"), b"]bad json").unwrap();
        let store = open(&dir, 10);
        assert_eq!(store.get(), State::default());
        let recovered = store.recovered_from().expect("recovery path");
        assert!(recovered.is_file());
        assert!(recovered.to_string_lossy().contains(".corrupt-"));
    }

    #[test]
    fn watchers_receive_persisted_snapshots() {
        let dir = TempDir::new().unwrap();
        let store = open(&dir, 5);
        let mut receiver = store.subscribe();
        store.update(|state| {
            state.counter = 42;
            state.label = "watched".into();
        });
        store.flush().unwrap();
        let snapshot = receiver.borrow_and_update().clone();
        assert_eq!(snapshot.counter, 42);
    }

    #[test]
    fn flush_reports_write_failures() {
        let dir = TempDir::new().unwrap();
        let store = open(&dir, 5);
        // Replace the parent directory with a file so the atomic write fails.
        std::fs::remove_dir_all(dir.path()).unwrap();
        std::fs::write(dir.path(), b"blocker").unwrap();
        store.update(|state| state.counter = 1);
        let error = store.flush().unwrap_err();
        assert!(matches!(error, AppError::Config(_)));
    }

    #[test]
    fn concurrent_updates_do_not_panic_or_corrupt() {
        let dir = TempDir::new().unwrap();
        let store = std::sync::Arc::new(open(&dir, 5));
        let mut handles = Vec::new();
        for index in 0..8u32 {
            let store = Arc::clone(&store);
            handles.push(std::thread::spawn(move || {
                for round in 0..20 {
                    store.update(|state| {
                        state.counter = index * 100 + round;
                    });
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        store.flush().unwrap();
        assert_eq!(read(&dir).counter, store.get().counter);
    }

    #[test]
    fn worker_thread_terminates_after_drop() {
        let dir = TempDir::new().unwrap();
        let store = open(&dir, 5);
        let finished = store.worker_finished_handle();
        assert!(!finished.load(Ordering::SeqCst));
        drop(store);
        assert!(
            finished.load(Ordering::SeqCst),
            "saver worker did not terminate before Drop returned"
        );
    }
}
