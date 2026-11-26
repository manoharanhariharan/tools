#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod activation;
mod commands;
mod dto;
mod hotkey;
mod integrations;
mod match_cache;
mod session_scripts;
mod state;
mod tls;
mod updates;
mod window;

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use hr_core::AppInfo;
use hr_observability::{StartupTrace, init_logging, install_panic_hook};
use tauri::{Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;

use state::AppState;

fn main() {
    tls::install_crypto_provider();
    let startup = StartupTrace::new();
    startup.mark("process");

    let exe_dir = executable_dir();
    let paths = match hr_config::resolve_default_profile(&exe_dir) {
        Ok(paths) => paths,
        Err(error) => {
            fatal(
                "H-Remote cannot choose a profile",
                &format!("{error}\n\nExecutable: {}", exe_dir.display()),
            );
            return;
        }
    };
    let log_guard = match init_logging(&paths.logs_dir, "info") {
        Ok(guard) => guard,
        Err(error) => {
            fatal(
                "H-Remote cannot start logging",
                &format!("{error}\n\nProfile: {}", paths.base_dir.display()),
            );
            return;
        }
    };
    install_panic_hook(paths.logs_dir.clone(), AppInfo::current());
    startup.mark("logging");
    tracing::info!(
        version = %AppInfo::current().version,
        profile = paths.mode.as_str(),
        base = %paths.base_dir.display(),
        "starting H-Remote"
    );

    let preflight = hr_config::preflight(&paths, true);
    if !preflight.ok {
        let details = preflight
            .failures()
            .map(|check| {
                format!(
                    "{}: {}",
                    check.path.display(),
                    check.error.as_deref().unwrap_or("not writable")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        fatal(
            "H-Remote cannot write its profile",
            &format!("{details}\n\nChoose a different profile location or fix the permissions."),
        );
        return;
    }
    startup.mark("preflight");

    let invocation = hr_platform::parse_cli_args(std::env::args().skip(1));
    let start_minimized = invocation.wants_start_minimized();

    let vault = hr_crypto::initialize_vault(
        paths.locality_dir.join(hr_crypto::SALT_FILE_NAME),
        paths.base_dir.join(hr_crypto::KEY_FILE_NAME),
    );

    let state = match tauri::async_runtime::block_on(AppState::initialize(
        paths, startup, log_guard, vault,
    )) {
        Ok(state) => state,
        Err(error) => {
            fatal("H-Remote cannot initialise", &error.to_string());
            return;
        }
    };
    if start_minimized {
        state.start_minimized.store(true, Ordering::SeqCst);
    }
    let log_level = state.config_snapshot().general.log_level;
    if let Err(error) = state.set_log_level(log_level) {
        tracing::warn!(%error, "cannot apply configured log level");
    }
    if !invocation.is_empty() || !invocation.errors.is_empty() {
        state.queue_actions(invocation.actions);
    }

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            let invocation = hr_platform::parse_cli_args(argv.iter().skip(1).cloned());
            if invocation.is_empty() {
                // Without a tray icon, relaunching the executable is the
                // natural way to bring a minimised catalog window back.
                window::show_main(app);
            } else {
                activation::handle(app, invocation);
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--start-minimized"]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event_state(&event) == ShortcutState::Pressed {
                        hotkey::on_pressed(app);
                    }
                })
                .build(),
        )
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::get_config,
            commands::update_config,
            commands::get_themes,
            commands::resolve_theme,
            commands::get_locales,
            commands::get_locality,
            commands::save_window_placement,
            commands::reset_main_window,
            commands::set_server_view,
            commands::get_connection_records,
            commands::record_connection,
            commands::recent_sessions,
            commands::get_tags,
            commands::set_pinned_tags,
            commands::set_tag_order,
            commands::open_log_folder,
            commands::open_config_folder,
            commands::open_url,
            commands::read_log_tail,
            commands::report_ui_error,
            commands::report_session_failure,
            commands::report_connection_failure,
            commands::save_error_report,
            commands::list_crash_reports,
            commands::read_crash_report,
            commands::trim_working_set,
            commands::flash_window,
            commands::show_main_window,
            commands::validate_hotkey,
            commands::app_info,
            commands::capabilities,
            commands::startup_checkpoints,
            commands::vault_status,
            commands::database_status,
            commands::repair_catalog_secrets,
            commands::set_log_level,
            commands::complete_guidance,
            commands::set_profile_mode,
            commands::detect_legacy_profile,
            commands::migrate_legacy_settings,
            commands::dismiss_legacy_migration,
            commands::install_desktop_shortcut,
            commands::uninstall_desktop_shortcut,
            commands::install_startup,
            commands::uninstall_startup,
            commands::startup_status,
            commands::ui_ready,
            commands::handle_cli,
            commands::list_protocols,
            commands::new_server,
            commands::list_servers,
            commands::list_servers_page,
            commands::get_server,
            commands::save_server,
            commands::save_servers,
            commands::delete_server,
            commands::list_credentials,
            commands::save_credential,
            commands::save_credentials,
            commands::delete_credential,
            commands::create_credential,
            commands::find_credential,
            commands::catalog_counts,
            commands::catalog_snapshot,
            commands::catalog_view,
            commands::list_folders,
            commands::create_folder,
            commands::ensure_folder,
            commands::rename_folder,
            commands::delete_folder,
            commands::move_folder,
            commands::set_folder_color,
            commands::duplicate_folder,
            commands::move_server_to_folder,
            commands::get_list_view,
            commands::update_list_view,
            commands::get_tree_view,
            commands::update_tree_view,
            commands::set_custom_order,
            commands::export_servers_json,
            commands::export_rdp_server,
            commands::import_json_content,
            commands::import_csv_content,
            commands::import_rdp_content,
            commands::import_database_file,
            commands::create_shortcut_for_servers,
            commands::create_shortcut_for_tag,
            commands::save_icon_to_library,
            commands::list_icon_library,
            commands::delete_icon_from_library,
            commands::fetch_icon_from_url,
            commands::test_script,
            commands::connect_server,
            commands::session_list,
            commands::session_snapshot,
            commands::session_cancel,
            commands::session_reconnect,
            commands::session_remove,
            commands::session_resize,
            commands::session_send_ctrl_alt_del,
            commands::session_send_special,
            commands::session_vnc_key,
            commands::session_vnc_pointer,
            commands::session_vnc_refresh,
            commands::session_vnc_text,
            commands::session_rdp_key,
            commands::session_rdp_pointer_move,
            commands::session_rdp_pointer_button,
            commands::session_rdp_wheel,
            commands::session_frame_ring,
            commands::session_frame_image,
            commands::open_session_window,
            commands::update_tab_workspace,
            commands::set_window_title,
            commands::file_list,
            commands::file_rename,
            commands::file_delete,
            commands::file_mkdir,
            commands::file_upload,
            commands::file_download,
            commands::file_upload_dialog,
            commands::file_download_dialog,
            commands::file_transfers,
            commands::file_cancel_transfer,
            commands::verify_sensitive_action,
            commands::verification_status,
            commands::check_server_availability,
            commands::data_source_list,
            commands::data_source_statuses,
            commands::data_source_save,
            commands::data_source_delete,
            commands::data_source_test,
            commands::data_source_refresh,
            commands::data_source_refresh_all,
            commands::change_local_database_path,
            commands::check_for_update,
            commands::update_check_status,
            commands::rating_decision,
            commands::legacy_database,
            commands::import_legacy_servers,
            commands::i18n_report,
            commands::show_launcher,
            commands::focus_server_in_catalog,
            commands::hide_launcher,
            commands::resize_launcher,
            commands::quick_connect,
            commands::quick_connect_history,
            commands::get_runners,
            commands::save_runner,
            commands::delete_runner,
            commands::set_default_runner,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let config = handle.state::<AppState>().config_snapshot();

            if config.general.run_at_startup {
                if let Err(error) = integrations::install_startup(&handle) {
                    tracing::warn!(%error, "cannot enforce startup integration");
                }
            }

            if let Err(error) =
                hotkey::apply(&handle, config.launcher.enabled, &config.launcher.hotkey)
            {
                tracing::warn!(%error, "launcher hotkey could not be registered");
                *handle
                    .state::<AppState>()
                    .hotkey_error
                    .lock()
                    .expect("hotkey error lock poisoned") = Some(error.to_string());
                let _ = handle.emit("hotkey:error", error.to_string());
            }

            if let Err(error) = window::restore_main(&handle) {
                tracing::error!(%error, "cannot restore the main window");
                return Err(Box::<dyn std::error::Error>::from(error));
            }
            handle.state::<AppState>().startup.mark("window");
            handle.state::<AppState>().startup.mark("ready");
            let _ = hr_platform::trim_working_set();

            spawn_config_watcher(handle.clone());
            spawn_session_events(handle.clone());
            spawn_file_events(handle.clone());
            spawn_source_watcher(handle.clone());
            spawn_update_watcher(handle);
            Ok(())
        })
        .on_window_event(|window, event| {
            let app = window.app_handle();
            let exiting = app.state::<AppState>().exiting.load(Ordering::SeqCst);
            if window.label() == window::LAUNCHER_WINDOW {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    if exiting {
                        return;
                    }
                    api.prevent_close();
                    window::save_launcher_placement(app);
                    let _ = window.hide();
                }
                return;
            }
            if window.label() == window::SESSION_WINDOW {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    if exiting {
                        return;
                    }
                    // Keep the pre-created webview alive; recreating it at
                    // runtime is what caused a blank, unresponsive window.
                    api.prevent_close();
                    window::save_session_placement(app, window.label());
                    let _ = window.hide();
                }
                return;
            }
            if window.label() != window::MAIN_WINDOW {
                return;
            }
            if let WindowEvent::CloseRequested { .. } = event {
                window::save_placement(app);
                // Closing the catalog is the user's quit action. Hidden
                // launcher/session windows must not keep the process (and its
                // DB/log/executable locks) alive.
                app.state::<AppState>()
                    .exiting
                    .store(true, Ordering::SeqCst);
                app.exit(0);
            }
        })
        .build(tauri::generate_context!());

    let app = match result {
        Ok(app) => app,
        Err(error) => {
            fatal("H-Remote cannot start", &error.to_string());
            return;
        }
    };

    app.run(|app, event| {
        if let RunEvent::ExitRequested { .. } = event {
            let state = app.state::<AppState>();
            if state.shutdown_started.swap(true, Ordering::SeqCst) {
                return;
            }
            window::save_placement(app);
            tauri::async_runtime::block_on(state.shutdown());
        }
    });
}

fn spawn_file_events(app: tauri::AppHandle) {
    let mut receiver = app.state::<AppState>().files.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    let _ = app.emit("file:event", event);
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

fn spawn_session_events(app: tauri::AppHandle) {
    let mut receiver = app.state::<AppState>().sessions.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    let _ = app.emit("session:event", event);
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
        tracing::debug!("session event watcher stopped");
    });
}

/// Refreshes additional sources in the background and re-checks their health on
/// the configured cadence (P8-07). Failed sources keep their cached catalog and
/// are retried on the reconnect interval; nothing here blocks the UI (P8-13).
fn spawn_source_watcher(app: tauri::AppHandle) {
    let state = app.state::<AppState>();
    let Some(sources) = state.sources.clone() else {
        return;
    };
    let config = state.config.clone();
    tauri::async_runtime::spawn(async move {
        sources.refresh_all().await;
        let _ = app.emit("source:statuses", sources.statuses().await);
        let mut since_check = 0u64;
        let mut since_reconnect = 0u64;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if app
                .state::<AppState>()
                .shutdown_started
                .load(Ordering::SeqCst)
            {
                tracing::debug!("data source watcher stopped");
                break;
            }
            let snapshot = config.get();
            since_check += 1;
            since_reconnect += 1;
            let mut changed = false;
            if snapshot.database_check_period > 0
                && since_check >= u64::from(snapshot.database_check_period)
            {
                since_check = 0;
                for status in sources.statuses().await {
                    if status.state == "ok" {
                        match sources.check(&status.id).await {
                            Ok(next) if next.state != status.state => changed = true,
                            Ok(_) => {}
                            Err(error) => tracing::debug!(%error, "source check failed"),
                        }
                    }
                }
            }
            if snapshot.database_reconnect_period > 0
                && since_reconnect >= u64::from(snapshot.database_reconnect_period)
            {
                since_reconnect = 0;
                for status in sources.statuses().await {
                    if status.state != "ok" {
                        if let Err(error) = sources.refresh(&status.id).await {
                            tracing::debug!(%error, "source reconnect failed");
                        } else {
                            changed = true;
                        }
                    }
                }
            }
            if changed {
                let _ = app.emit("source:statuses", sources.statuses().await);
            }
        }
    });
}

/// Periodically checks the configured update feed (P9-10). Disabled by the
/// "do not check" setting or a zero-hour period; manual checks are unaffected.
fn spawn_update_watcher(app: tauri::AppHandle) {
    let state = app.state::<AppState>();
    let config = state.config.clone();
    let version = state.app_info.version.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            if app
                .state::<AppState>()
                .shutdown_started
                .load(Ordering::SeqCst)
            {
                tracing::debug!("update watcher stopped");
                break;
            }
            let snapshot = config.get();
            if snapshot.general.do_not_check_for_updates
                || snapshot.general.update_check_period_hours == 0
            {
                continue;
            }
            let Some(feed) = snapshot.general.update_feed_url.clone() else {
                continue;
            };
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_secs() as i64)
                .unwrap_or_default();
            let period = i64::from(snapshot.general.update_check_period_hours) * 3600;
            let last = snapshot.engagement.last_update_check_at.unwrap_or(0);
            if now.saturating_sub(last) < period {
                continue;
            }
            let status = crate::updates::check(
                &feed,
                &version,
                snapshot.general.include_prerelease_updates,
                true,
                now,
            )
            .await;
            let state = app.state::<AppState>();
            *state
                .update_status
                .lock()
                .expect("update status lock poisoned") = Some(status.clone());
            config.update(|config| config.engagement.last_update_check_at = Some(now));
            if let Err(error) = config.flush() {
                tracing::warn!(%error, "cannot persist the update-check timestamp");
            }
            if status.available {
                let _ = app.emit("update:available", &status);
            }
        }
    });
}

fn spawn_config_watcher(app: tauri::AppHandle) {
    let mut receiver = app.state::<AppState>().config.subscribe();
    tauri::async_runtime::spawn(async move {
        while receiver.changed().await.is_ok() {
            let config = receiver.borrow_and_update().clone();
            let _ = app.emit("config:changed", &config);
        }
        tracing::debug!("configuration watcher stopped");
    });
}

fn event_state(event: &tauri_plugin_global_shortcut::ShortcutEvent) -> ShortcutState {
    event.state
}

fn executable_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn fatal(title: &str, message: &str) {
    eprintln!("{title}\n\n{message}");
    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(title)
        .set_description(message)
        .show();
}
