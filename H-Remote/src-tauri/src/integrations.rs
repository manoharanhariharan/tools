use std::path::{Path, PathBuf};

use hr_config::{MARKER_APPDATA, MARKER_PORTABLE, ProfileMode};
use hr_core::{AppError, AppResult};
use hr_platform::{ShortcutSpec, write_desktop_shortcut};
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

pub const APP_SHORTCUT_NAME: &str = "H-Remote";
pub const FEEDBACK_URL: &str = "https://github.com/h-remote/h-remote/issues";

pub fn desktop_shortcut_spec() -> AppResult<ShortcutSpec> {
    let executable = std::env::current_exe().map_err(|error| {
        AppError::platform(format!("cannot locate the H-Remote executable: {error}"))
    })?;
    let working_dir = executable.parent().map(Path::to_path_buf);
    let mut spec = ShortcutSpec::new(APP_SHORTCUT_NAME, executable)
        .with_description("H-Remote remote session manager");
    if let Some(working_dir) = working_dir {
        spec = spec.with_working_dir(working_dir);
    }
    Ok(spec)
}

pub fn install_desktop_shortcut(desktop_dir: &Path) -> AppResult<PathBuf> {
    let spec = desktop_shortcut_spec()?;
    write_desktop_shortcut(&spec, desktop_dir)
}

pub fn uninstall_desktop_shortcut(desktop_dir: &Path) -> AppResult<bool> {
    hr_platform::remove_desktop_shortcut(APP_SHORTCUT_NAME, desktop_dir)
}

pub fn install_startup(app: &AppHandle) -> AppResult<()> {
    app.autolaunch()
        .enable()
        .map_err(|error| AppError::platform(format!("cannot enable startup integration: {error}")))
}

pub fn uninstall_startup(app: &AppHandle) -> AppResult<()> {
    app.autolaunch()
        .disable()
        .map_err(|error| AppError::platform(format!("cannot disable startup integration: {error}")))
}

pub fn startup_enabled(app: &AppHandle) -> AppResult<bool> {
    app.autolaunch()
        .is_enabled()
        .map_err(|error| AppError::platform(format!("cannot query startup integration: {error}")))
}

/// Writes the marker that pins the profile mode for the next launch and removes
/// conflicting markers from both candidate locations.
pub fn write_profile_marker(
    mode: ProfileMode,
    executable_dir: &Path,
    config_dir: &Path,
) -> AppResult<PathBuf> {
    let appdata_base = config_dir.join(hr_config::APP_DIR_NAME);
    std::fs::create_dir_all(&appdata_base)?;
    for directory in [executable_dir, appdata_base.as_path()] {
        for marker in [MARKER_PORTABLE, MARKER_APPDATA] {
            let path = directory.join(marker);
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
        }
    }

    let (directory, marker) = match mode {
        ProfileMode::Portable => (executable_dir, MARKER_PORTABLE),
        ProfileMode::AppData => (appdata_base.as_path(), MARKER_APPDATA),
    };
    std::fs::create_dir_all(directory)?;
    let path = directory.join(marker);
    std::fs::write(
        &path,
        b"Managed by H-Remote. Delete to reset profile detection.\n",
    )?;
    Ok(path)
}

pub fn executable_dir() -> AppResult<PathBuf> {
    std::env::current_exe()
        .map_err(|error| AppError::platform(format!("cannot locate executable: {error}")))?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| AppError::platform("executable has no parent directory"))
}

pub fn open_in_file_manager(path: &Path) -> AppResult<()> {
    open::that_detached(path)
        .map_err(|error| AppError::platform(format!("cannot open {}: {error}", path.display())))
}

pub fn open_url(url: &str) -> AppResult<()> {
    open::that_detached(url)
        .map_err(|error| AppError::platform(format!("cannot open {url}: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn profile_markers_replace_conflicting_modes() {
        let exe = TempDir::new().unwrap();
        let config = TempDir::new().unwrap();
        let appdata = config.path().join(hr_config::APP_DIR_NAME);
        std::fs::create_dir_all(&appdata).unwrap();
        std::fs::write(exe.path().join(MARKER_PORTABLE), b"").unwrap();
        std::fs::write(appdata.join(MARKER_APPDATA), b"").unwrap();

        let marker = write_profile_marker(ProfileMode::AppData, exe.path(), config.path()).unwrap();
        assert!(marker.ends_with(MARKER_APPDATA));
        assert!(!exe.path().join(MARKER_PORTABLE).exists());
        assert!(!exe.path().join(MARKER_APPDATA).exists());
        assert!(appdata.join(MARKER_APPDATA).is_file());

        let marker =
            write_profile_marker(ProfileMode::Portable, exe.path(), config.path()).unwrap();
        assert!(marker.ends_with(MARKER_PORTABLE));
        assert!(!appdata.join(MARKER_APPDATA).exists());
        assert!(exe.path().join(MARKER_PORTABLE).is_file());
    }
}
