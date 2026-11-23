//! Integration test in its own process so the global tracing subscriber is
//! isolated from other tests. Verifies file output, flush-on-drop and runtime
//! level reloads (P0-50/P0-72).

use std::time::Duration;

use hr_observability::{LogGuard, init_logging};
use tempfile::TempDir;

fn wait_for_log(guard: &LogGuard, needle: &str) -> String {
    for _ in 0..100 {
        if let Ok(text) = std::fs::read_to_string(guard.file()) {
            if text.contains(needle) {
                return text;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let text = std::fs::read_to_string(guard.file()).unwrap_or_default();
    panic!(
        "log line '{needle}' not found in {}:\n{text}",
        guard.file().display()
    );
}

#[test]
fn writes_logs_and_reloads_level_at_runtime() {
    let dir = TempDir::new().unwrap();
    let guard = init_logging(dir.path(), "info").unwrap();

    tracing::info!("first-info-line");
    let text = wait_for_log(&guard, "first-info-line");
    assert!(text.contains("INFO"));

    guard.set_directive("error").unwrap();
    tracing::info!("hidden-info-line");
    tracing::error!("visible-error-line");
    let text = wait_for_log(&guard, "visible-error-line");
    assert!(!text.contains("hidden-info-line"));
    assert!(text.contains("ERROR"));

    guard.set_directive("debug").unwrap();
    tracing::debug!("visible-debug-line");
    let text = wait_for_log(&guard, "visible-debug-line");
    assert!(text.contains("DEBUG"));

    let file = guard.file().to_path_buf();
    drop(guard);
    assert!(
        std::fs::read_to_string(file)
            .unwrap()
            .contains("visible-debug-line")
    );
}
