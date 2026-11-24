use std::sync::atomic::Ordering;

use hr_config::WindowPlacement;
use hr_core::{AppError, AppResult};
use hr_platform::{MonitorInfo, PlacementDecision, screen_layout_signature, validate_placement};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, UserAttentionType, WebviewWindow};

use crate::state::AppState;

pub const MAIN_WINDOW: &str = "main";
pub const LAUNCHER_WINDOW: &str = "launcher";
/// One shared window hosts every live session as a tab, including the grid
/// workspace. The main window is the catalog only.
pub const SESSION_WINDOW: &str = "sessions";
pub const LAUNCHER_WIDTH: u32 = 720;
pub const LAUNCHER_MIN_HEIGHT: u32 = 220;
pub const LAUNCHER_MAX_HEIGHT: u32 = 760;

pub fn main_window(app: &AppHandle) -> AppResult<WebviewWindow> {
    app.get_webview_window(MAIN_WINDOW)
        .ok_or_else(|| AppError::not_found("main window"))
}

pub fn show_main(app: &AppHandle) {
    if let Ok(window) = main_window(app) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn launcher_window(app: &AppHandle) -> AppResult<WebviewWindow> {
    app.get_webview_window(LAUNCHER_WINDOW)
        .ok_or_else(|| AppError::not_found("launcher window"))
}

/// Borderless quick window centred horizontally, one fifth down the monitor.
pub fn show_launcher(app: &AppHandle) -> AppResult<()> {
    use tauri::Emitter;
    let window = launcher_window(app)?;
    let state = app.state::<AppState>();
    let monitors = monitor_snapshot(&window);
    let stored = state.locality.get().ui.launcher_window.clone();
    let height = stored.as_ref().map_or(460, |placement| placement.height);
    let candidate = if state.config_snapshot().launcher.remember_position {
        stored
    } else {
        None
    };
    let placement = candidate.unwrap_or_else(|| launcher_placement(&monitors, height));
    let decision = validate_placement(&placement, &monitors);
    apply_placement(&window, &decision)?;
    window
        .show()
        .map_err(|error| AppError::platform(format!("cannot show the launcher: {error}")))?;
    window
        .set_focus()
        .map_err(|error| AppError::platform(format!("cannot focus the launcher: {error}")))?;
    let _ = app.emit("launcher:shown", ());
    Ok(())
}

pub fn hide_launcher(app: &AppHandle) -> AppResult<()> {
    if let Ok(window) = launcher_window(app) {
        save_launcher_placement(app);
        window
            .hide()
            .map_err(|error| AppError::platform(format!("cannot hide the launcher: {error}")))?;
    }
    Ok(())
}

pub fn toggle_launcher(app: &AppHandle) -> AppResult<()> {
    use tauri::Emitter;
    match launcher_window(app) {
        Ok(window) if window.is_visible().unwrap_or(false) => hide_launcher(app),
        Ok(_) => show_launcher(app),
        Err(error) => {
            let _ = app.emit("launcher:error", error.to_string());
            Err(error)
        }
    }
}

/// The launcher grows/shrinks with its result count without changing width.
pub fn resize_launcher(app: &AppHandle, height: u32) -> AppResult<()> {
    let window = launcher_window(app)?;
    let height = height.clamp(LAUNCHER_MIN_HEIGHT, LAUNCHER_MAX_HEIGHT);
    let width = window
        .outer_size()
        .map(|size| size.width.max(LAUNCHER_WIDTH))
        .unwrap_or(LAUNCHER_WIDTH);
    window
        .set_size(PhysicalSize::new(width, height))
        .map_err(|error| AppError::platform(format!("cannot resize the launcher: {error}")))?;
    Ok(())
}

pub fn save_launcher_placement(app: &AppHandle) {
    let Ok(window) = launcher_window(app) else {
        return;
    };
    if !window.is_visible().unwrap_or(false) {
        return;
    }
    let Some(placement) = capture_placement(&window) else {
        return;
    };
    app.state::<AppState>()
        .locality
        .update(|locality| locality.ui.launcher_window = Some(placement));
}

fn launcher_placement(monitors: &[MonitorInfo], height: u32) -> WindowPlacement {
    let monitor = monitors
        .iter()
        .find(|monitor| monitor.is_primary)
        .or_else(|| monitors.first());
    let height = height.clamp(LAUNCHER_MIN_HEIGHT, LAUNCHER_MAX_HEIGHT);
    match monitor {
        Some(monitor) => WindowPlacement {
            x: monitor.x + (monitor.width.saturating_sub(LAUNCHER_WIDTH) / 2) as i32,
            y: monitor.y + (monitor.height / 5) as i32,
            width: LAUNCHER_WIDTH.min(monitor.width),
            height: height.min(monitor.height),
            scale_factor: Some(monitor.scale_factor),
            monitor_name: monitor.name.clone(),
            layout_signature: Some(screen_layout_signature(monitors)),
            ..WindowPlacement::default()
        },
        None => WindowPlacement {
            width: LAUNCHER_WIDTH,
            height,
            ..WindowPlacement::default()
        },
    }
}

pub fn hide_main(app: &AppHandle) -> AppResult<()> {
    main_window(app)?
        .hide()
        .map_err(|error| AppError::platform(format!("cannot hide window: {error}")))
}

pub fn flash(app: &AppHandle) {
    if let Ok(window) = main_window(app) {
        let _ = window.request_user_attention(Some(UserAttentionType::Informational));
    }
}

/// Restores the stored placement after validating it against the current
/// monitors. Called once during setup (P0-42/P0-43).
pub fn restore_main(app: &AppHandle) -> AppResult<()> {
    let window = main_window(app)?;
    let state = app.state::<AppState>();
    let monitors = monitor_snapshot(&window);
    let stored = state.locality.get().ui.main_window.clone();
    let candidate = stored.unwrap_or_else(|| default_placement(&monitors));
    let decision = validate_placement(&candidate, &monitors);
    apply_placement(&window, &decision)?;

    if decision.adjusted {
        let placement = decision.placement.clone();
        state
            .locality
            .update(|locality| locality.ui.main_window = Some(placement));
    }

    if state.start_minimized.load(Ordering::SeqCst) {
        window
            .hide()
            .map_err(|error| AppError::platform(format!("cannot hide window: {error}")))?;
    } else {
        window
            .show()
            .map_err(|error| AppError::platform(format!("cannot show window: {error}")))?;
        let _ = window.set_focus();
    }
    Ok(())
}

pub fn reset_main_window(app: &AppHandle) -> AppResult<()> {
    let window = main_window(app)?;
    let state = app.state::<AppState>();
    let monitors = monitor_snapshot(&window);
    let decision = validate_placement(&default_placement(&monitors), &monitors);
    apply_placement(&window, &decision)?;
    state
        .locality
        .update(|locality| locality.ui.main_window = Some(decision.placement));
    let _ = window.show();
    let _ = window.set_focus();
    Ok(())
}

/// Persists the current geometry before hiding/closing.
pub fn save_placement(app: &AppHandle) {
    let Ok(window) = main_window(app) else {
        return;
    };
    let state = app.state::<AppState>();
    if let Some(placement) = capture_placement(&window) {
        state
            .locality
            .update(|locality| locality.ui.main_window = Some(placement));
    }
    for (label, window) in app.webview_windows() {
        if label == SESSION_WINDOW
            && window.is_visible().unwrap_or(false)
            && let Some(placement) = capture_placement(&window)
        {
            state.locality.update(|locality| {
                locality
                    .session
                    .session_windows
                    .insert(label.clone(), placement);
            });
        }
    }
}

pub fn save_session_placement(app: &AppHandle, label: &str) {
    let Some(window) = app.get_webview_window(label) else {
        return;
    };
    let Some(placement) = capture_placement(&window) else {
        return;
    };
    app.state::<AppState>().locality.update(|locality| {
        locality
            .session
            .session_windows
            .insert(label.to_string(), placement);
    });
}

/// Opens (or focuses) the shared session window. The size/position comes from
/// the last stored placement, then a centred default.
/// Shows the session window that Tauri creates (hidden) at startup. Building a
/// webview synchronously from a command used to leave a white window while the
/// main thread waited on the new webview's first navigation.
pub fn open_session_window(
    app: &AppHandle,
    _session_id: &str,
    requested: Option<WindowPlacement>,
) -> AppResult<String> {
    let label = SESSION_WINDOW.to_string();
    let window = app
        .get_webview_window(&label)
        .ok_or_else(|| AppError::not_found("session window"))?;
    let state = app.state::<AppState>();
    let stored = state
        .locality
        .get()
        .session
        .session_windows
        .get(&label)
        .cloned();
    let monitors = monitor_snapshot(&window);
    let placement = requested
        .or(stored)
        .unwrap_or_else(|| session_placement(&monitors));
    let decision = validate_placement(&placement, &monitors);
    apply_placement(&window, &decision)?;
    window
        .show()
        .map_err(|error| AppError::platform(format!("cannot show the session window: {error}")))?;
    let _ = window.unminimize();
    let _ = window.set_focus();
    Ok(label)
}

fn capture_placement(window: &WebviewWindow) -> Option<WindowPlacement> {
    let position = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    let monitors = monitor_snapshot(window);
    let current = window.current_monitor().ok().flatten();
    Some(WindowPlacement {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        maximized: window.is_maximized().unwrap_or(false),
        monitor_name: current.as_ref().and_then(|monitor| monitor.name().cloned()),
        scale_factor: current.as_ref().map(|monitor| monitor.scale_factor()),
        layout_signature: Some(screen_layout_signature(&monitors)),
    })
}

fn apply_placement(window: &WebviewWindow, decision: &PlacementDecision) -> AppResult<()> {
    let placement = &decision.placement;
    window
        .set_size(PhysicalSize::new(placement.width, placement.height))
        .map_err(|error| AppError::platform(format!("cannot resize window: {error}")))?;
    window
        .set_position(PhysicalPosition::new(placement.x, placement.y))
        .map_err(|error| AppError::platform(format!("cannot position window: {error}")))?;
    if placement.maximized {
        window
            .maximize()
            .map_err(|error| AppError::platform(format!("cannot maximize window: {error}")))?;
    } else {
        let _ = window.unmaximize();
    }
    Ok(())
}

fn monitor_snapshot(window: &WebviewWindow) -> Vec<MonitorInfo> {
    let primary = window.primary_monitor().ok().flatten();
    let primary_key = primary.map(|monitor| {
        (
            monitor.name().cloned(),
            monitor.position().x,
            monitor.position().y,
        )
    });
    window
        .available_monitors()
        .unwrap_or_default()
        .into_iter()
        .map(|monitor| {
            let is_primary = primary_key.as_ref().is_some_and(|(name, x, y)| {
                monitor.name() == name.as_ref()
                    && monitor.position().x == *x
                    && monitor.position().y == *y
            });
            MonitorInfo {
                name: monitor.name().cloned(),
                x: monitor.position().x,
                y: monitor.position().y,
                width: monitor.size().width,
                height: monitor.size().height,
                scale_factor: monitor.scale_factor(),
                is_primary,
            }
        })
        .collect()
}

fn session_placement(monitors: &[MonitorInfo]) -> WindowPlacement {
    let primary = monitors
        .iter()
        .find(|monitor| monitor.is_primary)
        .or_else(|| monitors.first());
    match primary {
        Some(monitor) => WindowPlacement {
            x: monitor.x + 80,
            y: monitor.y + 80,
            width: 980.min(monitor.width),
            height: 700.min(monitor.height),
            scale_factor: Some(monitor.scale_factor),
            monitor_name: monitor.name.clone(),
            layout_signature: Some(screen_layout_signature(monitors)),
            ..WindowPlacement::default()
        },
        None => WindowPlacement {
            width: 980,
            height: 700,
            ..WindowPlacement::default()
        },
    }
}

fn default_placement(monitors: &[MonitorInfo]) -> WindowPlacement {
    let primary = monitors
        .iter()
        .find(|monitor| monitor.is_primary)
        .or_else(|| monitors.first());
    let (width, height) = (1200u32, 800u32);
    match primary {
        Some(monitor) => WindowPlacement {
            x: monitor.x + (monitor.width.saturating_sub(width) / 2) as i32,
            y: monitor.y + (monitor.height.saturating_sub(height) / 2) as i32,
            width: width.min(monitor.width),
            height: height.min(monitor.height),
            scale_factor: Some(monitor.scale_factor),
            monitor_name: monitor.name.clone(),
            layout_signature: Some(screen_layout_signature(monitors)),
            ..WindowPlacement::default()
        },
        None => WindowPlacement {
            width,
            height,
            ..WindowPlacement::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_window_is_a_single_shared_label() {
        assert_eq!(SESSION_WINDOW, "sessions");
        assert!(!SESSION_WINDOW.starts_with("session-"));
    }

    #[test]
    fn session_placement_is_offset_and_clamped() {
        let monitors = vec![MonitorInfo {
            name: Some("primary".into()),
            x: 0,
            y: 0,
            width: 1024,
            height: 768,
            scale_factor: 1.0,
            is_primary: true,
        }];
        let placement = session_placement(&monitors);
        assert_eq!(placement.x, 80);
        assert_eq!(placement.y, 80);
        assert_eq!(placement.width, 980);
        assert_eq!(placement.height, 700);
    }

    #[test]
    fn launcher_placement_is_centred_and_clamped() {
        let monitors = vec![MonitorInfo {
            name: Some("primary".into()),
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
            is_primary: true,
        }];
        let placement = launcher_placement(&monitors, 10_000);
        assert_eq!(placement.width, LAUNCHER_WIDTH);
        assert_eq!(placement.height, LAUNCHER_MAX_HEIGHT);
        assert_eq!(placement.x, ((1920 - LAUNCHER_WIDTH) / 2) as i32);
        assert_eq!(placement.y, 216);
        let tiny = launcher_placement(&monitors, 1);
        assert_eq!(tiny.height, LAUNCHER_MIN_HEIGHT);
    }
}
