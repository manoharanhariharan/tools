use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// A named startup boundary (P0-06/P0-50, Appendix B `StartupTrace`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    pub name: String,
    pub elapsed_ms: u64,
}

/// Records time from process start to each start-up milestone. Cheap enough to
/// call on every startup and included in diagnostics.
pub struct StartupTrace {
    start: Instant,
    marks: Mutex<Vec<Checkpoint>>,
}

impl Default for StartupTrace {
    fn default() -> Self {
        Self::new()
    }
}

impl StartupTrace {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            marks: Mutex::new(Vec::new()),
        }
    }

    pub fn mark(&self, name: impl Into<String>) -> Checkpoint {
        let checkpoint = Checkpoint {
            name: name.into(),
            elapsed_ms: self.elapsed_ms(),
        };
        tracing::info!(
            checkpoint = %checkpoint.name,
            elapsed_ms = checkpoint.elapsed_ms,
            "startup checkpoint"
        );
        self.marks
            .lock()
            .expect("startup trace lock poisoned")
            .push(checkpoint.clone());
        checkpoint
    }

    pub fn snapshot(&self) -> Vec<Checkpoint> {
        self.marks
            .lock()
            .expect("startup trace lock poisoned")
            .clone()
    }

    pub fn total_ms(&self) -> u64 {
        self.elapsed_ms()
    }

    fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoints_are_ordered_and_serialisable() {
        let trace = StartupTrace::new();
        trace.mark("logging");
        std::thread::sleep(std::time::Duration::from_millis(2));
        trace.mark("profile");
        trace.mark("window");

        let marks = trace.snapshot();
        assert_eq!(marks.len(), 3);
        assert_eq!(marks[0].name, "logging");
        assert_eq!(marks[1].name, "profile");
        assert!(
            marks
                .windows(2)
                .all(|pair| pair[0].elapsed_ms <= pair[1].elapsed_ms)
        );
        assert!(trace.total_ms() >= marks[2].elapsed_ms);

        let json = serde_json::to_string(&marks).unwrap();
        assert!(json.contains("\"elapsedMs\""));
    }

    #[test]
    fn marks_are_isolated_per_trace() {
        let first = StartupTrace::new();
        let second = StartupTrace::new();
        first.mark("a");
        assert_eq!(first.snapshot().len(), 1);
        assert!(second.snapshot().is_empty());
    }
}
