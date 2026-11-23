//! Pre/post-connect script orchestration (P9-05/P9-06).
//!
//! The pre-connect script is synchronous and aborts the connection on a non-zero
//! exit. The post-disconnect script runs asynchronously once the session reaches
//! a terminal state, and receives the same environment.

use std::collections::BTreeMap;
use std::sync::Arc;

use hr_core::{AppError, AppResult};
use hr_domain::Server;
use hr_platform::{ScriptEnvironment, ScriptResult, run_script_with_env};
use hr_session::{SessionEvent, SessionManager};
use tokio::sync::broadcast::error::RecvError;

/// Scripts apply to session-backed protocols; file transfers use their own
/// lifecycle and are documented as unsupported for now.
pub fn supports(server: &Server) -> bool {
    !matches!(server, Server::Sftp(_) | Server::Ftp(_))
}

pub fn environment(server: &Server, session_id: &str) -> BTreeMap<String, String> {
    ScriptEnvironment {
        session_id: session_id.to_string(),
        server_id: server.base().id.clone(),
        server_name: server.base().display_name.clone(),
        server_host: server
            .address()
            .map(|address| address.address.clone())
            .unwrap_or_default(),
        server_tags: server.base().tags.clone(),
    }
    .variables()
}

pub async fn run_pre_connect(server: &Server, session_id: &str) -> AppResult<Option<ScriptResult>> {
    let script = server.base().command_before_connected.clone();
    if script.trim().is_empty() {
        return Ok(None);
    }
    let hide = server.base().hide_command_before_connected_window;
    let environment = environment(server, session_id);
    let result = tauri::async_runtime::spawn_blocking(move || {
        run_script_with_env(&script, hide, &environment)
    })
    .await
    .map_err(|error| {
        AppError::platform(format!("the pre-connect script was cancelled: {error}"))
    })??;
    Ok(Some(result))
}

/// Runs the pre-connect script and converts a failure into a connect-time error.
pub async fn ensure_pre_connect(server: &Server, session_id: &str) -> AppResult<()> {
    let Some(result) = run_pre_connect(server, session_id).await? else {
        return Ok(());
    };
    if result.success {
        return Ok(());
    }
    let code = result
        .code
        .map_or_else(|| "unknown".to_string(), |code| code.to_string());
    let detail = result.stderr.trim();
    Err(AppError::validation(if detail.is_empty() {
        format!("the pre-connect script failed (exit code {code})")
    } else {
        format!("the pre-connect script failed (exit code {code}): {detail}")
    }))
}

/// Waits for the session to finish and then runs the post-disconnect script in
/// the background. The task ends with the session or when the manager shuts down.
pub fn spawn_post_disconnect(sessions: Arc<SessionManager>, server: Server, session_id: String) {
    let script = server.base().command_after_disconnected.clone();
    if script.trim().is_empty() {
        return;
    }
    let hide = server.base().hide_command_before_connected_window;
    let environment = environment(&server, &session_id);
    // A very short-lived session may already be terminal before the watcher
    // subscribes; check first so the script still runs and the task never leaks.
    let already_terminal = sessions
        .snapshot(&session_id)
        .map(|snapshot| snapshot.state.is_terminal())
        .unwrap_or(false);
    let mut events = sessions.subscribe();
    tokio::spawn(async move {
        if !already_terminal {
            loop {
                match events.recv().await {
                    Ok(SessionEvent::State { session }) => {
                        if session.id == session_id && session.state.is_terminal() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => return,
                }
            }
        }
        let outcome =
            tokio::task::spawn_blocking(move || run_script_with_env(&script, hide, &environment))
                .await;
        match outcome {
            Ok(Ok(result)) if result.success => {
                tracing::debug!(session = %session_id, "post-disconnect script finished");
            }
            Ok(Ok(result)) => {
                tracing::warn!(
                    session = %session_id,
                    code = ?result.code,
                    stderr = %result.stderr,
                    "post-disconnect script failed"
                );
            }
            Ok(Err(error)) => {
                tracing::warn!(%error, session = %session_id, "post-disconnect script cannot run");
            }
            Err(error) => {
                tracing::warn!(%error, session = %session_id, "post-disconnect script was cancelled");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use hr_domain::Server;

    #[test]
    fn environment_exports_the_documented_variables() {
        let mut server = Server::new_ssh("Build box", "10.0.0.5", "root");
        server.base_mut().tags = vec!["prod".into(), "linux".into()];
        let variables = environment(&server, "session-1");
        assert_eq!(variables["SESSION_ID"], "session-1");
        assert_eq!(variables["SERVER_ID"], server.base().id);
        assert_eq!(variables["SERVER_NAME"], "Build box");
        assert_eq!(variables["SERVER_HOST"], "10.0.0.5");
        assert_eq!(variables["SERVER_TAGS"], "prod,linux");
    }

    #[test]
    fn file_transfers_are_excluded_from_session_scripts() {
        assert!(supports(&Server::new_ssh("Box", "host", "root")));
        assert!(supports(&Server::new_rdp("Desk", "host", 3389)));
        assert!(!supports(&Server::new_sftp("Files", "host", "root")));
        assert!(!supports(&Server::new_ftp("FTP", "host", "root")));
    }

    #[tokio::test]
    async fn pre_connect_failure_aborts_with_the_exit_code() {
        let mut server = Server::new_ssh("Box", "host", "root");
        #[cfg(windows)]
        let script = "echo boom 1>&2 & exit /b 7";
        #[cfg(not(windows))]
        let script = "echo boom 1>&2; exit 7";
        server.base_mut().command_before_connected = script.into();
        server.base_mut().hide_command_before_connected_window = true;
        let error = ensure_pre_connect(&server, "session-1").await.unwrap_err();
        let message = error.to_string();
        assert!(message.contains("exit code 7"), "{message}");
        assert!(message.contains("boom"), "{message}");
    }

    #[tokio::test]
    async fn empty_pre_connect_script_is_skipped() {
        let server = Server::new_ssh("Box", "host", "root");
        assert!(
            run_pre_connect(&server, "session-1")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn post_script_runs_even_when_the_session_already_finished() {
        use std::sync::Arc;
        use std::time::Duration;

        use hr_session::{
            BackendRegistry, ProcessBackend, ProcessKind, ProcessPlan, SessionManager, SessionPlan,
        };

        let manager = Arc::new(SessionManager::new(
            BackendRegistry::new(Arc::new(ProcessBackend), Arc::new(ProcessBackend))
                .with_process(Arc::new(ProcessBackend)),
        ));
        #[cfg(windows)]
        let (program, arguments) = ("cmd", vec!["/C".to_string(), "exit 0".to_string()]);
        #[cfg(not(windows))]
        let (program, arguments) = ("sh", vec!["-c".to_string(), "exit 0".to_string()]);
        let plan = ProcessPlan {
            server_id: "server".into(),
            server_name: "Process".into(),
            kind: ProcessKind::LocalApp,
            program: program.into(),
            arguments,
            working_dir: None,
            environment: Default::default(),
            temporary_content: None,
            private_key_file: None,
            run_with_hosting: false,
            max_attempts: 1,
            warnings: Vec::new(),
        };
        let snapshot = manager
            .start_plan(SessionPlan::Process(plan))
            .expect("session starts");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while !manager
            .snapshot(&snapshot.id)
            .map(|snapshot| snapshot.state.is_terminal())
            .unwrap_or(true)
        {
            assert!(
                tokio::time::Instant::now() < deadline,
                "process did not finish"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        let marker = std::env::temp_dir().join(format!("hremote-post-{}.txt", hr_core::new_id()));
        let marker_argument = marker.display().to_string();
        let mut server = Server::new_ssh("Box", "host", "root");
        server.base_mut().command_after_disconnected = format!("echo done > \"{marker_argument}\"");
        server.base_mut().hide_command_before_connected_window = true;
        spawn_post_disconnect(Arc::clone(&manager), server, snapshot.id.clone());

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while !marker.exists() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "post script did not run"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let _ = std::fs::remove_file(marker);
    }
}
