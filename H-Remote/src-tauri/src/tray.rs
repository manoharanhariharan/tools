use std::sync::atomic::Ordering;

use hr_config::{Configuration, ConnectionRecords};
use hr_core::{AppError, AppResult};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

use crate::integrations::{FEEDBACK_URL, open_in_file_manager, open_url};
use crate::{AppState, window};

pub const TRAY_ID: &str = "main";

/// Row contents for the "recently used" tray submenu. Kept pure so it can be
/// tested without a window system.
pub fn recent_tray_entries(records: &ConnectionRecords, limit: u8) -> Vec<(String, String)> {
    records
        .recent_server_ids(usize::from(limit))
        .into_iter()
        .map(|id| {
            let short = id.get(id.len().saturating_sub(8)..).unwrap_or(&id);
            (format!("recent:{id}"), short.to_string())
        })
        .collect()
}

pub fn setup(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let config = state.config_snapshot();
    let menu = build_menu(app, &config)?;
    let tooltip = state.i18n.t("tray.tooltip");

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip(tooltip)
        .on_menu_event(|app, event| handle_menu_event(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                window::show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder
        .build(app)
        .map_err(|error| AppError::platform(format!("cannot create tray icon: {error}")))?;
    Ok(())
}

pub fn rebuild(app: &AppHandle, config: &Configuration) -> AppResult<()> {
    let state = app.state::<AppState>();
    let menu = build_menu(app, config)?;
    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| AppError::not_found("tray icon"))?;
    tray.set_menu(Some(menu))
        .map_err(|error| AppError::platform(format!("cannot update tray menu: {error}")))?;
    tray.set_tooltip(Some(state.i18n.t("tray.tooltip")))
        .map_err(|error| AppError::platform(format!("cannot update tray tooltip: {error}")))?;
    Ok(())
}

fn build_menu(app: &AppHandle, config: &Configuration) -> AppResult<Menu<tauri::Wry>> {
    let state = app.state::<AppState>();
    let t = |key: &str| state.i18n.t(key);

    let show =
        MenuItem::with_id(app, "show", t("tray.show"), true, None::<&str>).map_err(menu_error)?;
    let hide =
        MenuItem::with_id(app, "hide", t("tray.hide"), true, None::<&str>).map_err(menu_error)?;
    let recent = Submenu::with_id(app, "recent", t("tray.recent"), true).map_err(menu_error)?;

    if config.general.show_recent_in_tray {
        let entries = recent_tray_entries(
            &state.connection_records.get(),
            config.launcher.recent_limit,
        );
        if entries.is_empty() {
            let empty =
                MenuItem::with_id(app, "recent-empty", t("tray.noRecent"), false, None::<&str>)
                    .map_err(menu_error)?;
            recent.append(&empty).map_err(menu_error)?;
        } else {
            for (id, label) in entries {
                let item =
                    MenuItem::with_id(app, id, label, true, None::<&str>).map_err(menu_error)?;
                recent.append(&item).map_err(menu_error)?;
            }
        }
    }

    let toggle_recent = CheckMenuItem::with_id(
        app,
        "toggle-recent",
        t("tray.showRecent"),
        true,
        config.general.show_recent_in_tray,
        None::<&str>,
    )
    .map_err(menu_error)?;
    let reset = MenuItem::with_id(
        app,
        "reset-window",
        t("tray.resetWindow"),
        true,
        None::<&str>,
    )
    .map_err(menu_error)?;
    let open_logs = MenuItem::with_id(app, "open-logs", t("tray.openLogs"), true, None::<&str>)
        .map_err(menu_error)?;
    let feedback = MenuItem::with_id(app, "feedback", t("tray.feedback"), true, None::<&str>)
        .map_err(menu_error)?;
    let about =
        MenuItem::with_id(app, "about", t("tray.about"), true, None::<&str>).map_err(menu_error)?;
    let exit =
        MenuItem::with_id(app, "exit", t("tray.exit"), true, None::<&str>).map_err(menu_error)?;
    let separator_one = PredefinedMenuItem::separator(app).map_err(menu_error)?;
    let separator_two = PredefinedMenuItem::separator(app).map_err(menu_error)?;
    let separator_three = PredefinedMenuItem::separator(app).map_err(menu_error)?;

    let menu = if config.general.show_recent_in_tray {
        Menu::with_items(
            app,
            &[
                &show,
                &hide,
                &recent,
                &separator_one,
                &toggle_recent,
                &reset,
                &open_logs,
                &separator_two,
                &feedback,
                &about,
                &separator_three,
                &exit,
            ],
        )
    } else {
        Menu::with_items(
            app,
            &[
                &show,
                &hide,
                &separator_one,
                &toggle_recent,
                &reset,
                &open_logs,
                &separator_two,
                &feedback,
                &about,
                &separator_three,
                &exit,
            ],
        )
    }
    .map_err(menu_error)?;
    Ok(menu)
}

fn menu_error(error: tauri::Error) -> AppError {
    AppError::platform(format!("cannot build tray menu: {error}"))
}

pub fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        "show" => window::show_main(app),
        "hide" => {
            if let Err(error) = window::hide_main(app) {
                tracing::warn!(%error, "cannot hide the main window");
            }
        }
        "exit" => quit(app),
        "reset-window" => {
            if let Err(error) = window::reset_main_window(app) {
                tracing::error!(%error, "cannot reset main window");
            }
        }
        "about" => {
            window::show_main(app);
            let _ = app.emit("ui:navigate", "about");
        }
        "open-logs" => {
            let state = app.state::<AppState>();
            if let Err(error) = open_in_file_manager(&state.paths.logs_dir) {
                tracing::error!(%error, "cannot open log folder");
            }
        }
        "feedback" => {
            if let Err(error) = open_url(FEEDBACK_URL) {
                tracing::error!(%error, "cannot open feedback link");
            }
        }
        "toggle-recent" => {
            let state = app.state::<AppState>();
            let updated = state.config.update(|config| {
                config.general.show_recent_in_tray = !config.general.show_recent_in_tray;
            });
            if let Err(error) = state.config.flush() {
                tracing::error!(%error, "cannot persist tray preference");
            }
            if let Err(error) = rebuild(app, &updated) {
                tracing::error!(%error, "cannot rebuild tray menu");
            }
        }
        id if id.starts_with("recent:") => {
            let server_id = id.trim_start_matches("recent:").to_string();
            window::show_main(app);
            let _ = app.emit(
                "activation:connect",
                hr_platform::CliAction::ConnectById(server_id),
            );
        }
        _ => {}
    }
}

fn quit(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.quitting.store(true, Ordering::SeqCst);
    window::save_placement(app);
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use hr_config::QuickConnectEntry;

    #[test]
    fn recent_entries_are_capped_and_labelled() {
        let mut records = ConnectionRecords::default();
        records.record_connection("01ARZ3NDEKTSV4RRFFQ69G5FAV", 100);
        records.record_connection("01ARZ3NDEKTSV4RRFFQ69G5FAA", 200);
        records.record_connection("01ARZ3NDEKTSV4RRFFQ69G5FAB", 300);
        let entries = recent_tray_entries(&records, 2);
        assert_eq!(entries.len(), 2);
        assert!(entries[0].0.starts_with("recent:"));
        assert_eq!(entries[0].1.len(), 8);
        assert_eq!(entries[1].1.len(), 8);
    }

    #[test]
    fn empty_history_produces_no_entries() {
        let mut records = ConnectionRecords::default();
        records.record_quick_connect(
            QuickConnectEntry {
                protocol: "rdp".into(),
                address: "host".into(),
                port: 3389,
                username: "u".into(),
                last_used_at: 1,
            },
            10,
        );
        assert!(recent_tray_entries(&records, 20).is_empty());
    }
}
