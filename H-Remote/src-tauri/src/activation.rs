use std::sync::atomic::Ordering;

use hr_core::AppResult;
use hr_platform::{CliAction, CliInvocation};
use tauri::{AppHandle, Emitter, Manager};

use crate::integrations;
use crate::{AppState, window};

/// Entry point for the second-instance callback and manually supplied CLI args.
/// Actions are queued until the UI signals readiness (P0-57).
pub fn handle(app: &AppHandle, invocation: CliInvocation) {
    if !invocation.errors.is_empty() {
        tracing::warn!(errors = ?invocation.errors, "invalid command-line arguments");
        let _ = app.emit("cli:errors", &invocation.errors);
    }
    let state = app.state::<AppState>();
    if !state.data_ready.load(Ordering::SeqCst) {
        state.queue_actions(invocation.actions);
        return;
    }
    dispatch(app, &invocation.actions);
}

/// Called once the Svelte shell has finished its first render.
pub fn ui_ready(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.data_ready.store(true, Ordering::SeqCst);
    let pending = state.take_pending_actions();
    if !pending.is_empty() {
        dispatch(app, &pending);
    }
    let _ = app.emit("app:ready", ());
}

pub fn dispatch(app: &AppHandle, actions: &[CliAction]) {
    let state = app.state::<AppState>();
    for action in actions {
        match action {
            CliAction::Activate => window::show_main(app),
            CliAction::StartMinimized => {
                let _ = window::hide_main(app);
            }
            CliAction::Install => {
                let mut warnings = Vec::new();
                if let Err(error) = install_shortcut(&mut warnings) {
                    warnings.push(error.to_string());
                }
                if let Err(error) = integrations::install_startup(app) {
                    warnings.push(error.to_string());
                } else {
                    state
                        .config
                        .update(|config| config.general.run_at_startup = true);
                    let _ = state.config.flush();
                }
                emit_warnings(app, warnings);
            }
            CliAction::Uninstall => {
                let mut warnings = Vec::new();
                if let Err(error) = uninstall_shortcut(&mut warnings) {
                    warnings.push(error.to_string());
                }
                if let Err(error) = integrations::uninstall_startup(app) {
                    warnings.push(error.to_string());
                } else {
                    state
                        .config
                        .update(|config| config.general.run_at_startup = false);
                    let _ = state.config.flush();
                }
                emit_warnings(app, warnings);
            }
            CliAction::InstallDesktopShortcut => {
                let mut warnings = Vec::new();
                if let Err(error) = install_shortcut(&mut warnings) {
                    warnings.push(error.to_string());
                }
                emit_warnings(app, warnings);
            }
            CliAction::UninstallDesktopShortcut => {
                let mut warnings = Vec::new();
                if let Err(error) = uninstall_shortcut(&mut warnings) {
                    warnings.push(error.to_string());
                }
                emit_warnings(app, warnings);
            }
            CliAction::InstallStartup => {
                if let Err(error) = integrations::install_startup(app) {
                    emit_warnings(app, vec![error.to_string()]);
                } else {
                    state
                        .config
                        .update(|config| config.general.run_at_startup = true);
                    let _ = state.config.flush();
                }
            }
            CliAction::UninstallStartup => {
                if let Err(error) = integrations::uninstall_startup(app) {
                    emit_warnings(app, vec![error.to_string()]);
                } else {
                    state
                        .config
                        .update(|config| config.general.run_at_startup = false);
                    let _ = state.config.flush();
                }
            }
            CliAction::ConnectById(_)
            | CliAction::ConnectByTag(_)
            | CliAction::ConnectByName(_) => {
                // Connections open in the shared session window, so the catalog
                // window stays hidden when launched from a shortcut.
                let _ = app.emit("activation:connect", action);
            }
        }
    }
}

fn install_shortcut(warnings: &mut Vec<String>) -> AppResult<()> {
    let desktop = match hr_platform::default_desktop_dir() {
        Ok(desktop) => desktop,
        Err(error) => {
            warnings.push(error.to_string());
            return Ok(());
        }
    };
    match integrations::install_desktop_shortcut(&desktop) {
        Ok(_) => Ok(()),
        Err(error) => {
            warnings.push(error.to_string());
            Ok(())
        }
    }
}

fn uninstall_shortcut(warnings: &mut Vec<String>) -> AppResult<()> {
    let desktop = match hr_platform::default_desktop_dir() {
        Ok(desktop) => desktop,
        Err(error) => {
            warnings.push(error.to_string());
            return Ok(());
        }
    };
    match integrations::uninstall_desktop_shortcut(&desktop) {
        Ok(_) => Ok(()),
        Err(error) => {
            warnings.push(error.to_string());
            Ok(())
        }
    }
}

fn emit_warnings(app: &AppHandle, warnings: Vec<String>) {
    if warnings.is_empty() {
        return;
    }
    for warning in &warnings {
        tracing::warn!(%warning, "command-line integration warning");
    }
    let _ = app.emit("integration:warnings", &warnings);
}
