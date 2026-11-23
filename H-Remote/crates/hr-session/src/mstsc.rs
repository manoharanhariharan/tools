//! External `mstsc` fallback (P3-35).
//!
//! Generates a temporary `.rdp` file from the same plan the embedded engine
//! consumes, protects the password with DPAPI when available, launches
//! `mstsc.exe`, and reports the external process lifecycle. Frame capture is
//! intentionally absent: the embedded engine is the in-webview path.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use hr_core::{AppError, AppResult, new_id};
use hr_domain::RdpFile;

use crate::manager::{Backend, BackendContext, BackendOutcome};
use crate::plan::RdpPlan;
use crate::session_plan::SessionPlan;
use crate::state::{DisconnectReason, SessionBackend};

#[derive(Debug, Default)]
pub struct MstscBackend;

impl Backend for MstscBackend {
    fn kind(&self) -> SessionBackend {
        SessionBackend::Mstsc
    }

    fn run<'a>(
        &'a self,
        context: BackendContext<'a>,
    ) -> Pin<Box<dyn Future<Output = BackendOutcome> + Send + 'a>> {
        Box::pin(async move { run_mstsc(context).await })
    }
}

async fn run_mstsc(context: BackendContext<'_>) -> BackendOutcome {
    let SessionPlan::Rdp(plan) = context.plan else {
        return BackendOutcome::Failed {
            reason: DisconnectReason::ConnectionFailed,
            message: "the mstsc backend received a non-RDP plan".to_string(),
        };
    };
    let path = match write_rdp_file(plan, &std::env::temp_dir()) {
        Ok(path) => path,
        Err(error) => {
            return BackendOutcome::Failed {
                reason: DisconnectReason::ConnectionFailed,
                message: error.to_string(),
            };
        }
    };
    let result = spawn_and_wait(&path, context.cancel).await;
    let _ = tokio::fs::remove_file(&path).await;
    result
}

/// Builds the `.rdp` payload for the fallback. Exposed for tests and for the
/// "export as .rdp" flow when the caller wants a dry run.
pub fn rdp_file_content(plan: &RdpPlan) -> AppResult<String> {
    let mut file = RdpFile::default();
    for (key, value_type, value) in plan.rdp_file_settings() {
        file.set(&key, &value_type, value);
    }
    file.merge_freeform(&plan.additional_settings);
    if !plan.password.is_empty() {
        match hr_platform::rdp_password_blob(plan.password.expose()) {
            Ok(blob) => file.set("password 51", "b", hex_encode(&blob)),
            Err(error) => tracing::warn!(
                %error,
                "mstsc fallback will prompt for the password because DPAPI is unavailable"
            ),
        }
    }
    Ok(file.to_string())
}

pub fn write_rdp_file(plan: &RdpPlan, directory: &Path) -> AppResult<PathBuf> {
    let content = rdp_file_content(plan)?;
    let path = directory.join(format!("hremote-{}.rdp", new_id()));
    std::fs::write(&path, content).map_err(|error| {
        AppError::platform(format!(
            "cannot write the temporary .rdp file {}: {error}",
            path.display()
        ))
    })?;
    Ok(path)
}

#[cfg(windows)]
async fn spawn_and_wait(path: &Path, cancel: crate::manager::CancelToken) -> BackendOutcome {
    use tokio::process::Command;

    let mut command = Command::new("mstsc.exe");
    command.arg(path);
    command.kill_on_drop(true);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return BackendOutcome::Failed {
                reason: DisconnectReason::ConnectionFailed,
                message: format!("cannot launch mstsc.exe: {error}"),
            };
        }
    };
    tokio::select! {
        _ = cancel.cancelled() => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            BackendOutcome::Cancelled
        }
        status = child.wait() => {
            match status {
                Ok(status) if status.success() => BackendOutcome::Disconnected {
                    reason: DisconnectReason::NoInfo,
                    message: Some("The external mstsc window was closed".to_string()),
                },
                Ok(status) => BackendOutcome::Disconnected {
                    reason: DisconnectReason::NoInfo,
                    message: Some(format!(
                        "mstsc exited with code {}",
                        status.code().unwrap_or(-1)
                    )),
                },
                Err(error) => BackendOutcome::Failed {
                    reason: DisconnectReason::ConnectionFailed,
                    message: format!("cannot monitor mstsc: {error}"),
                },
            }
        }
    }
}

#[cfg(not(windows))]
async fn spawn_and_wait(_path: &Path, _cancel: crate::manager::CancelToken) -> BackendOutcome {
    BackendOutcome::Failed {
        reason: DisconnectReason::ConnectionFailed,
        message: "the external mstsc engine is only available on Windows".to_string(),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut output, byte| {
        let _ = write!(output, "{byte:02X}");
        output
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hr_domain::Server;
    use tempfile::TempDir;

    fn plan() -> RdpPlan {
        let mut server = Server::new_rdp("Desktop", "10.0.0.9", 3390);
        let Server::Rdp(rdp) = &mut server else {
            unreachable!()
        };
        rdp.credentials.username = "admin".into();
        rdp.credentials.password = "hunter2".into();
        rdp.domain = "CORP".into();
        rdp.is_administrative_purposes = true;
        rdp.rdp_file_additional_settings = "custom:s:value".into();
        RdpPlan::from_server(&server, (1280, 720), 100, Default::default()).unwrap()
    }

    #[test]
    fn generated_rdp_contains_plan_fields_and_extra_settings() {
        let content = rdp_file_content(&plan()).unwrap();
        assert!(content.contains("full address:s:10.0.0.9:3390"));
        assert!(content.contains("username:s:admin"));
        assert!(content.contains("domain:s:CORP"));
        assert!(content.contains("administrative session:i:1"));
        assert!(content.contains("custom:s:value"));
    }

    #[cfg(windows)]
    #[test]
    fn password_is_dpapi_protected() {
        let content = rdp_file_content(&plan()).unwrap();
        assert!(content.contains("password 51:b:"));
        assert!(!content.contains("hunter2"));
    }

    #[cfg(not(windows))]
    #[test]
    fn password_is_omitted_without_dpapi() {
        let content = rdp_file_content(&plan()).unwrap();
        assert!(!content.contains("password 51"));
        assert!(!content.contains("hunter2"));
    }

    #[test]
    fn file_is_written_to_the_requested_directory() {
        let dir = TempDir::new().unwrap();
        let path = write_rdp_file(&plan(), dir.path()).unwrap();
        assert_eq!(path.parent(), Some(dir.path()));
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("full address:s:10.0.0.9:3390"));
    }
}
