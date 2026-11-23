//! External process backend: terminal runners, local apps and RemoteApp.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use hr_core::AppResult;
use tokio::process::Command;

use crate::manager::{Backend, BackendContext, BackendEvent, BackendOutcome};
use crate::process::{ProcessKind, ProcessPlan};
use crate::session_plan::SessionPlan;
use crate::state::{DisconnectReason, SessionBackend};

#[derive(Debug, Default)]
pub struct ProcessBackend;

impl Backend for ProcessBackend {
    fn kind(&self) -> SessionBackend {
        SessionBackend::Terminal
    }

    fn run<'a>(
        &'a self,
        context: BackendContext<'a>,
    ) -> Pin<Box<dyn Future<Output = BackendOutcome> + Send + 'a>> {
        Box::pin(async move { run(context).await })
    }
}

async fn run(context: BackendContext<'_>) -> BackendOutcome {
    let SessionPlan::Process(plan) = context.plan else {
        return BackendOutcome::Failed {
            reason: DisconnectReason::ConnectionFailed,
            message: "the process backend received a non-process plan".to_string(),
        };
    };
    let (program, arguments, temporary) = match prepare(plan) {
        Ok(prepared) => prepared,
        Err(error) => {
            return BackendOutcome::Failed {
                reason: DisconnectReason::ConnectionFailed,
                message: error.to_string(),
            };
        }
    };
    let mut command = Command::new(&program);
    command.args(&arguments);
    for (key, value) in &plan.environment {
        command.env(key, value);
    }
    if let Some(directory) = &plan.working_dir {
        command.current_dir(directory);
    }
    command.kill_on_drop(true);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            cleanup(&temporary).await;
            return BackendOutcome::Failed {
                reason: DisconnectReason::ConnectionFailed,
                message: format!("cannot launch '{program}': {error}"),
            };
        }
    };
    let _ = context.events.send(BackendEvent::Connected);

    tokio::select! {
        _ = context.cancel.cancelled() => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            cleanup(&temporary).await;
            BackendOutcome::Cancelled
        }
        status = child.wait() => {
            cleanup(&temporary).await;
            match status {
                Ok(status) if status.success() => BackendOutcome::Disconnected {
                    reason: DisconnectReason::NoInfo,
                    message: Some(exit_message(plan, status.code())),
                },
                Ok(status) => BackendOutcome::Disconnected {
                    reason: DisconnectReason::NoInfo,
                    message: Some(exit_message(plan, status.code())),
                },
                Err(error) => BackendOutcome::Failed {
                    reason: DisconnectReason::ConnectionFailed,
                    message: format!("cannot monitor '{program}': {error}"),
                },
            }
        }
    }
}

fn exit_message(plan: &ProcessPlan, code: Option<i32>) -> String {
    let suffix = code.map_or_else(|| "unknown".to_string(), |code| code.to_string());
    match plan.kind {
        ProcessKind::Ssh | ProcessKind::Telnet | ProcessKind::Serial | ProcessKind::Custom => {
            format!("{} exited with code {suffix}", plan.program)
        }
        ProcessKind::LocalApp => format!("{} exited with code {suffix}", plan.server_name),
        ProcessKind::RemoteApp => "RemoteApp window closed".to_string(),
    }
}

/// Materialises temporary `.rdp`/startup/key files and returns the final command.
fn prepare(plan: &ProcessPlan) -> AppResult<(String, Vec<String>, Vec<PathBuf>)> {
    let mut arguments = plan.arguments.clone();
    let mut temporary = Vec::new();
    if let Some((extension, content)) = &plan.temporary_content {
        let path =
            std::env::temp_dir().join(format!("hremote-{}.{}", hr_core::new_id(), extension));
        std::fs::write(&path, content).map_err(|error| {
            hr_core::AppError::platform(format!(
                "cannot write the temporary file {}: {error}",
                path.display()
            ))
        })?;
        arguments.push(path.display().to_string());
        temporary.push(path);
    }
    if let Some((path, content)) = &plan.private_key_file {
        write_private_key(path, content)?;
        temporary.push(PathBuf::from(path));
    }
    Ok((plan.program.clone(), arguments, temporary))
}

fn write_private_key(path: &str, content: &str) -> AppResult<()> {
    let path = std::path::Path::new(path);
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|error| {
                hr_core::AppError::platform(format!(
                    "cannot write the private key file {}: {error}",
                    path.display()
                ))
            })?;
        file.write_all(content.as_bytes()).map_err(|error| {
            hr_core::AppError::platform(format!(
                "cannot write the private key file {}: {error}",
                path.display()
            ))
        })?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, content).map_err(|error| {
            hr_core::AppError::platform(format!(
                "cannot write the private key file {}: {error}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

async fn cleanup(paths: &[PathBuf]) {
    for path in paths {
        let _ = tokio::fs::remove_file(path).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hr_config::RunnersConfig;
    use hr_domain::Server;
    use std::sync::Arc;
    use std::time::Duration;

    use crate::manager::{BackendRegistry, SessionManager};
    use crate::plan::ReconnectPolicy;
    use crate::session_plan::{SessionPlan, VncPlan};
    use crate::state::SessionState;

    fn process_plan(kind: ProcessKind) -> ProcessPlan {
        #[cfg(windows)]
        let (program, arguments) = ("cmd", vec!["/C".into(), "exit 0".into()]);
        #[cfg(not(windows))]
        let (program, arguments) = ("sh", vec!["-c".into(), "exit 0".into()]);
        ProcessPlan {
            server_id: "server".into(),
            server_name: "Process".into(),
            kind,
            program: program.into(),
            arguments,
            working_dir: None,
            environment: Default::default(),
            temporary_content: None,
            private_key_file: None,
            run_with_hosting: false,
            max_attempts: 1,
            warnings: Vec::new(),
        }
    }

    fn manager() -> SessionManager {
        SessionManager::new(
            BackendRegistry::new(Arc::new(ProcessBackend), Arc::new(ProcessBackend))
                .with_process(Arc::new(ProcessBackend)),
        )
    }

    #[tokio::test]
    async fn process_exits_and_reports_completion() {
        let manager = manager();
        let mut events = manager.subscribe();
        let snapshot = manager
            .start_plan(SessionPlan::Process(process_plan(ProcessKind::LocalApp)))
            .unwrap();
        assert_eq!(snapshot.backend, SessionBackend::LocalApp);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            assert!(!left.is_zero(), "process did not finish");
            if let Ok(Ok(crate::state::SessionEvent::State { session })) =
                tokio::time::timeout(left, events.recv()).await
                && session.state == SessionState::Disconnected
            {
                assert!(session.message.unwrap().contains("code 0"));
                break;
            }
        }
    }

    #[tokio::test]
    async fn missing_executable_fails_clearly() {
        let manager = manager();
        let mut plan = process_plan(ProcessKind::LocalApp);
        plan.program = "definitely-not-a-real-program".into();
        let mut events = manager.subscribe();
        manager.start_plan(SessionPlan::Process(plan)).unwrap();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            assert!(!left.is_zero(), "no failure event");
            if let Ok(Ok(crate::state::SessionEvent::State { session })) =
                tokio::time::timeout(left, events.recv()).await
                && session.state == SessionState::Failed
            {
                assert!(session.message.unwrap().contains("cannot launch"));
                break;
            }
        }
    }

    #[test]
    fn prepare_writes_and_returns_temporary_files() {
        let mut plan = process_plan(ProcessKind::RemoteApp);
        plan.temporary_content = Some(("rdp".into(), "full address:s:host".into()));
        let (_, arguments, temporary) = prepare(&plan).unwrap();
        assert_eq!(temporary.len(), 1);
        assert!(arguments.last().unwrap().ends_with(".rdp"));
        let content = std::fs::read_to_string(&temporary[0]).unwrap();
        assert!(content.contains("full address"));
        let _ = std::fs::remove_file(&temporary[0]);
    }

    #[test]
    fn prepare_writes_the_private_key_and_cleanup_removes_it() {
        let mut plan = process_plan(ProcessKind::Ssh);
        let path = std::env::temp_dir().join(format!("hremote-key-test-{}.pem", hr_core::new_id()));
        let path = path.display().to_string();
        plan.private_key_file = Some((path.clone(), "-----BEGIN PRIVATE KEY-----\n".into()));
        let (_, arguments, temporary) = prepare(&plan).unwrap();
        assert!(!arguments.iter().any(|argument| argument == &path));
        assert!(
            !arguments
                .iter()
                .any(|argument| argument.contains("BEGIN PRIVATE KEY"))
        );
        assert_eq!(temporary.len(), 1);
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("BEGIN PRIVATE KEY")
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(cleanup(&temporary));
        assert!(!std::path::Path::new(&path).exists());
    }

    #[test]
    fn runner_plans_do_not_leak_passwords_in_debug() {
        let mut runners = RunnersConfig::default();
        let mut runner = runners.runner(hr_config::PUTTY_SSH).unwrap().clone();
        runner.exe_path = "putty.exe".into();
        runners.upsert(runner).unwrap();
        let mut server = Server::new_ssh("Box", "host", "root");
        let Server::Ssh(ssh) = &mut server else {
            unreachable!()
        };
        ssh.credentials.password = "secret".into();
        let plan = ProcessPlan::from_server(&server, &runners).unwrap();
        let debug = format!("{plan:?}");
        assert!(!debug.contains("secret"));
        assert!(!debug.contains("root@host"));
    }

    #[test]
    fn registry_rejects_missing_vnc_backend() {
        let manager = SessionManager::new(BackendRegistry::new(
            Arc::new(ProcessBackend),
            Arc::new(ProcessBackend),
        ));
        let mut server = Server::new_vnc("Lab", "host", 5900);
        server.normalize();
        let _ = ReconnectPolicy::default();
        assert!(
            manager
                .start_plan(SessionPlan::Vnc(VncPlan::from_server(&server).unwrap()))
                .is_err()
        );
    }
}
