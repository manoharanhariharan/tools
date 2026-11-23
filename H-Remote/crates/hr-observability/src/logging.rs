use std::path::{Path, PathBuf};

use hr_core::{AppError, AppResult};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::reload;
use tracing_subscriber::util::SubscriberInitExt;

type ReloadHandle = reload::Handle<EnvFilter, tracing_subscriber::Registry>;

/// Keeps the non-blocking log worker alive. Dropping it flushes pending lines.
pub struct LogGuard {
    _worker: WorkerGuard,
    logs_dir: PathBuf,
    file: PathBuf,
    filter: ReloadHandle,
}

impl LogGuard {
    pub fn logs_dir(&self) -> &Path {
        &self.logs_dir
    }

    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Changes the active log level without restarting (P0-72).
    pub fn set_directive(&self, directive: &str) -> AppResult<()> {
        self.filter
            .reload(EnvFilter::new(directive.trim()))
            .map_err(|error| AppError::config(format!("cannot change log level: {error}")))
    }
}

/// Installs the global tracing subscriber writing daily Markdown-friendly logs.
///
/// Calling this more than once in one process is safe: the first installation
/// wins and later calls only return a guard for the same file sink.
pub fn init_logging(logs_dir: &Path, directive: &str) -> AppResult<LogGuard> {
    std::fs::create_dir_all(logs_dir).map_err(|error| {
        AppError::permission_denied(format!(
            "cannot create log directory {}: {error}",
            logs_dir.display()
        ))
    })?;

    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("hremote.log")
        .filename_suffix("md")
        .max_log_files(14)
        .build(logs_dir)
        .map_err(|error| AppError::config(format!("cannot create log file: {error}")))?;
    let file = current_log_file(logs_dir);

    let (writer, worker) = tracing_appender::non_blocking(appender);
    let (filter, reload_handle) = reload::Layer::new(EnvFilter::new(directive.trim()));

    let subscriber = tracing_subscriber::registry().with(filter).with(
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_target(true)
            .with_thread_names(true)
            .with_writer(writer),
    );

    if let Err(error) = subscriber.try_init() {
        // A global subscriber already exists (tests or a second initialisation).
        tracing::debug!(%error, "logging already initialised");
    }

    Ok(LogGuard {
        _worker: worker,
        logs_dir: logs_dir.to_path_buf(),
        file,
        filter: reload_handle,
    })
}

/// Newest file produced by the rolling appender, falling back to the prefix.
pub fn current_log_file(logs_dir: &Path) -> PathBuf {
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(logs_dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("hremote.log") && name.ends_with(".md"))
        })
        .collect();
    candidates.sort();
    candidates
        .pop()
        .unwrap_or_else(|| logs_dir.join("hremote.log.md"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn log_directory_is_created() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("a").join("b");
        let guard = init_logging(&nested, "info").unwrap();
        assert!(nested.is_dir());
        assert!(guard.logs_dir().ends_with("b"));
    }
}
