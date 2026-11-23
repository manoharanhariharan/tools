use std::path::{Path, PathBuf};

use hr_core::{AppError, AppInfo, AppResult};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use time::macros::format_description;

const MAX_DETAILS: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashReport {
    pub path: PathBuf,
    pub markdown: String,
}

/// Installs a process-wide panic hook that writes a Markdown crash report.
///
/// Call this immediately after logging is initialised (P0-07/P0-53). The hook
/// never panics itself and always prints a short message to stderr.
pub fn install_panic_hook(logs_dir: PathBuf, app_info: AppInfo) {
    std::panic::set_hook(Box::new(move |panic_info| {
        let message = if let Some(payload) = panic_info.payload().downcast_ref::<&str>() {
            (*payload).to_string()
        } else if let Some(payload) = panic_info.payload().downcast_ref::<String>() {
            payload.clone()
        } else {
            "unknown panic payload".to_string()
        };
        let location = panic_info
            .location()
            .map(|location| {
                format!(
                    "{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            })
            .unwrap_or_else(|| "unknown location".to_string());
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let details = format!("location: {location}\n\nbacktrace:\n{backtrace}");
        match write_crash_report(&logs_dir, "panic", &message, &details, &app_info) {
            Ok(report) => {
                eprintln!(
                    "H-Remote crashed: {message}\nCrash report: {}",
                    report.path.display()
                );
            }
            Err(error) => {
                eprintln!("H-Remote crashed: {message}\nCould not write crash report: {error}");
            }
        }
    }));
}

/// Writes a crash report for a panic or a UI-side unhandled error.
pub fn write_crash_report(
    logs_dir: &Path,
    source: &str,
    message: &str,
    details: &str,
    app_info: &AppInfo,
) -> AppResult<CrashReport> {
    std::fs::create_dir_all(logs_dir).map_err(|error| {
        AppError::permission_denied(format!(
            "cannot create log directory {}: {error}",
            logs_dir.display()
        ))
    })?;
    let timestamp = OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(format_description!(
            "[year][month][day]-[hour][minute][second]"
        ))
        .unwrap_or_else(|_| "unknown-time".to_string());
    let markdown = format_crash_markdown(source, message, details, app_info);
    let path = logs_dir.join(format!(
        "crash-{}-{}-{}.md",
        timestamp,
        std::process::id(),
        monotonic_suffix()
    ));
    std::fs::write(&path, markdown.as_bytes())?;
    Ok(CrashReport { path, markdown })
}

/// Pure report formatter (unit-tested without touching the file system).
pub fn format_crash_markdown(
    source: &str,
    message: &str,
    details: &str,
    app_info: &AppInfo,
) -> String {
    let details = redact_secrets(details);
    let details = truncate(&details, MAX_DETAILS);
    let timestamp = OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string());

    format!(
        "# H-Remote crash report\n\n\
         **Time:** {timestamp}  \n\
         **Source:** {source}  \n\
         **Version:** {version}  \n\
         **Platform:** {os}/{arch}  \n\
         **Build:** {build}  \n\n\
         ## Summary\n\n```\n{message}\n```\n\n\
         ## Details\n\n```\n{details}\n```\n\n\
         ## Environment\n\n\
         - OS: {os}\n\
         - Architecture: {arch}\n\
         - App version: {version}\n\
         - Debug build: {debug}\n\
         - Process id: {pid}\n\n\
         ---\n\n\
         *Secrets are redacted before a report is written. Review the details above before sharing.*\n",
        version = app_info.version,
        os = app_info.target_os,
        arch = app_info.target_arch,
        build = if app_info.debug { "debug" } else { "release" },
        debug = app_info.debug,
        pid = std::process::id(),
        message = redact_secrets(message),
    )
}

/// Removes common `key=value` secrets from free-form details.
pub fn redact_secrets(input: &str) -> String {
    const SENSITIVE: [&str; 8] = [
        "password",
        "passwd",
        "token",
        "secret",
        "private_key",
        "private-key",
        "api_key",
        "apikey",
    ];
    input
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if let Some(marker) = SENSITIVE.iter().find(|marker| lower.contains(**marker)) {
                if lower.contains('=') || lower.contains(':') {
                    return format!("[redacted line containing '{marker}']");
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn list_crash_reports(logs_dir: &Path) -> Vec<PathBuf> {
    let mut reports: Vec<PathBuf> = std::fs::read_dir(logs_dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("crash-") && name.ends_with(".md"))
        })
        .collect();
    reports.sort();
    reports.reverse();
    reports
}

fn truncate(value: &str, max: usize) -> String {
    if value.len() <= max {
        return value.to_string();
    }
    let mut end = max;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}…\n[truncated {} bytes]",
        &value[..end],
        value.len() - end
    )
}

fn monotonic_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn info() -> AppInfo {
        AppInfo {
            name: "H-Remote".into(),
            version: "0.1.0".into(),
            target_os: "test-os".into(),
            target_arch: "test-arch".into(),
            debug: true,
        }
    }

    #[test]
    fn report_contains_environment_and_message() {
        let markdown = format_crash_markdown("ui", "boom", "stack line", &info());
        assert!(markdown.contains("# H-Remote crash report"));
        assert!(markdown.contains("**Source:** ui"));
        assert!(markdown.contains("boom"));
        assert!(markdown.contains("test-os/test-arch"));
        assert!(markdown.contains("0.1.0"));
        assert!(markdown.contains("Debug build: true"));
    }

    #[test]
    fn secrets_are_redacted() {
        let details = "user=alice\npassword=hunter2\ntoken = abc123\nsafe=yes";
        let redacted = redact_secrets(details);
        assert!(!redacted.contains("hunter2"));
        assert!(!redacted.contains("abc123"));
        assert!(redacted.contains("user=alice"));
        assert!(redacted.contains("safe=yes"));
    }

    #[test]
    fn large_details_are_truncated_on_a_char_boundary() {
        let details = "é".repeat(MAX_DETAILS);
        let markdown = format_crash_markdown("panic", "x", &details, &info());
        assert!(markdown.contains("[truncated"));
        assert!(markdown.is_char_boundary(markdown.len()));
    }

    #[test]
    fn writes_unique_files_and_lists_them() {
        let dir = TempDir::new().unwrap();
        let first = write_crash_report(dir.path(), "panic", "one", "details", &info()).unwrap();
        let second = write_crash_report(dir.path(), "ui", "two", "details", &info()).unwrap();
        assert_ne!(first.path, second.path);
        assert!(first.path.is_file());
        let listed = list_crash_reports(dir.path());
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().all(|path| path.is_file()));
    }

    #[test]
    fn missing_log_directory_is_created() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("nested").join("logs");
        let report = write_crash_report(&nested, "panic", "message", "details", &info()).unwrap();
        assert!(report.path.is_file());
    }
}
