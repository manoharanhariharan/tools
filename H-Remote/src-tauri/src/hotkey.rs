use std::str::FromStr;

use hr_core::{AppError, AppResult};
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::window;

/// Applies launcher hotkey configuration. Unregisters first so changing the
/// shortcut never leaks a registration or leaves a stale one.
pub fn apply(app: &AppHandle, enabled: bool, hotkey: &str) -> AppResult<Option<String>> {
    let manager = app.global_shortcut();
    manager
        .unregister_all()
        .map_err(|error| AppError::platform(format!("cannot reset hotkeys: {error}")))?;

    if !enabled {
        return Ok(None);
    }

    let canonical = hr_platform::canonical_hotkey(hotkey)
        .map_err(|error| AppError::validation(error.to_string()))?;
    let shortcut = Shortcut::from_str(&canonical)
        .map_err(|error| AppError::validation(format!("unsupported hotkey: {error}")))?;
    manager.register(shortcut).map_err(|error| {
        let message = error.to_string();
        if message.to_ascii_lowercase().contains("already") {
            AppError::conflict(format!("{canonical} is already registered: {message}"))
        } else {
            AppError::platform(format!("cannot register {canonical}: {message}"))
        }
    })?;
    Ok(Some(canonical))
}

pub fn on_pressed(app: &AppHandle) {
    if let Err(error) = window::toggle_launcher(app) {
        tracing::warn!(%error, "cannot toggle the launcher window");
        window::show_main(app);
    }
    let _ = app.emit("launcher:toggle", ());
}
