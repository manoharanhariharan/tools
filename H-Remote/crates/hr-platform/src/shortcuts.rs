use std::path::{Path, PathBuf};

use hr_core::{AppError, AppResult};
use serde::{Deserialize, Serialize};

use crate::capabilities::shortcut_flavor;

/// Everything needed to create a desktop shortcut for a server, tag or the app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutSpec {
    pub name: String,
    pub target: PathBuf,
    pub arguments: Vec<String>,
    pub working_dir: Option<PathBuf>,
    pub icon: Option<PathBuf>,
    pub description: String,
    /// Terminal applications should open in a terminal on Linux.
    pub terminal: bool,
}

impl ShortcutSpec {
    pub fn new(name: impl Into<String>, target: impl Into<PathBuf>) -> Self {
        Self {
            name: sanitize_name(&name.into()),
            target: target.into(),
            arguments: Vec::new(),
            working_dir: None,
            icon: None,
            description: String::new(),
            terminal: false,
        }
    }

    pub fn with_arguments(mut self, arguments: impl IntoIterator<Item = String>) -> Self {
        self.arguments = arguments.into_iter().collect();
        self
    }

    pub fn with_working_dir(mut self, directory: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(directory.into());
        self
    }

    pub fn with_icon(mut self, icon: impl Into<PathBuf>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
}

/// Sanitises a display name into a cross-platform file name stem.
pub fn sanitize_name(name: &str) -> String {
    let mut cleaned: String = name
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect();
    cleaned = cleaned.trim().trim_end_matches('.').to_string();
    while cleaned.ends_with(' ') {
        cleaned.pop();
    }
    if cleaned.is_empty() {
        "H-Remote".to_string()
    } else {
        cleaned
    }
}

pub fn shortcut_file_name(name: &str) -> String {
    format!("{}.{}", sanitize_name(name), shortcut_flavor().extension())
}

pub fn default_desktop_dir() -> AppResult<PathBuf> {
    dirs::desktop_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Desktop")))
        .ok_or_else(|| AppError::platform("desktop directory is unavailable"))
}

/// Creates or replaces a desktop shortcut and returns its path.
pub fn write_desktop_shortcut(spec: &ShortcutSpec, desktop_dir: &Path) -> AppResult<PathBuf> {
    std::fs::create_dir_all(desktop_dir).map_err(|error| {
        AppError::permission_denied(format!(
            "cannot create desktop directory {}: {error}",
            desktop_dir.display()
        ))
    })?;
    let path = desktop_dir.join(shortcut_file_name(&spec.name));

    #[cfg(windows)]
    {
        let mut link = mslnk::ShellLink::new(&spec.target).map_err(|error| {
            AppError::platform(format!(
                "cannot create shortcut for {}: {error}",
                spec.target.display()
            ))
        })?;
        link.set_name(Some(spec.name.clone()));
        if !spec.arguments.is_empty() {
            link.set_arguments(Some(join_arguments(&spec.arguments)));
        }
        if let Some(directory) = &spec.working_dir {
            link.set_working_dir(Some(directory.display().to_string()));
        }
        if let Some(icon) = &spec.icon {
            link.set_icon_location(Some(icon.display().to_string()));
        }
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        link.create_lnk(&path).map_err(|error| {
            AppError::platform(format!("cannot write shortcut {}: {error}", path.display()))
        })?;
        Ok(path)
    }

    #[cfg(target_os = "macos")]
    {
        let content = command_script_content(spec);
        std::fs::write(&path, content)?;
        set_executable(&path)?;
        Ok(path)
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::fs::write(&path, desktop_entry_content(spec))?;
        set_executable(&path)?;
        Ok(path)
    }

    #[cfg(not(any(windows, unix)))]
    {
        std::fs::write(&path, command_script_content(spec))?;
        Ok(path)
    }
}

pub fn remove_desktop_shortcut(name: &str, desktop_dir: &Path) -> AppResult<bool> {
    let path = desktop_dir.join(shortcut_file_name(name));
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(AppError::from(error)),
    }
}

#[cfg(windows)]
fn join_arguments(arguments: &[String]) -> String {
    arguments
        .iter()
        .map(|argument| {
            if argument.contains([' ', '\t', '"']) {
                format!("\"{}\"", argument.replace('"', "\\\""))
            } else {
                argument.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(unix)]
fn set_executable(path: &Path) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

fn quote_exec(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('$', "\\$");
    format!("\"{escaped}\"")
}

fn desktop_value(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

/// Freedesktop `.desktop` content (Linux, and autostart uses the same format).
pub fn desktop_entry_content(spec: &ShortcutSpec) -> String {
    let mut exec = vec![quote_exec(&spec.target.display().to_string())];
    exec.extend(spec.arguments.iter().map(|argument| quote_exec(argument)));
    let mut content = format!(
        "[Desktop Entry]\n\
         Version=1.0\n\
         Type=Application\n\
         Name={}\n\
         Comment={}\n\
         Exec={}\n\
         Terminal={}\n\
         Categories=Network;\n\
         StartupNotify=true\n",
        desktop_value(&spec.name),
        desktop_value(&spec.description),
        exec.join(" "),
        if spec.terminal { "true" } else { "false" }
    );
    if let Some(directory) = &spec.working_dir {
        content.push_str(&format!(
            "Path={}\n",
            desktop_value(&directory.display().to_string())
        ));
    }
    if let Some(icon) = &spec.icon {
        content.push_str(&format!(
            "Icon={}\n",
            desktop_value(&icon.display().to_string())
        ));
    }
    content
}

/// macOS `.command` fallback used where a signed `.app` bundle is unavailable.
pub fn command_script_content(spec: &ShortcutSpec) -> String {
    let mut command = format!("exec {}", shell_quote(&spec.target.display().to_string()));
    for argument in &spec.arguments {
        command.push(' ');
        command.push_str(&shell_quote(argument));
    }
    if let Some(directory) = &spec.working_dir {
        command = format!(
            "cd {} || exit 1\n{}",
            shell_quote(&directory.display().to_string()),
            command
        );
    }
    format!("#!/bin/sh\n# Generated by H-Remote\n{command}\n")
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn names_are_sanitised_and_never_empty() {
        assert_eq!(sanitize_name("My Server"), "My Server");
        assert_eq!(sanitize_name("a/b:c*?\"<>|"), "a_b_c______");
        assert_eq!(sanitize_name("   "), "H-Remote");
        assert_eq!(sanitize_name("trailing. "), "trailing");
    }

    #[test]
    fn desktop_entry_quotes_and_escapes_arguments() {
        let spec = ShortcutSpec::new("Prod Server", "/opt/H-Remote/hremote")
            .with_arguments(vec![
                "ULID:01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
                "--start-minimized".to_string(),
                "name with spaces".to_string(),
                "$(danger)".to_string(),
            ])
            .with_working_dir("/opt/H-Remote")
            .with_description("Production");
        let content = desktop_entry_content(&spec);
        assert!(content.contains("Name=Prod Server"));
        assert!(content.contains("Exec=\"/opt/H-Remote/hremote\""));
        assert!(content.contains("\"name with spaces\""));
        assert!(content.contains("\\$(danger)"));
        assert!(content.contains("Path=/opt/H-Remote"));
        assert!(content.contains("Terminal=false"));
    }

    #[test]
    fn command_script_quotes_single_quotes() {
        let spec =
            ShortcutSpec::new("x", "/Applications/My App").with_arguments(vec!["it's".to_string()]);
        let content = command_script_content(&spec);
        assert!(content.starts_with("#!/bin/sh"));
        assert!(content.contains("'it'\\''s'"));
    }

    #[cfg(unix)]
    #[test]
    fn writes_an_executable_shortcut_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let desktop = TempDir::new().unwrap();
        let spec = ShortcutSpec::new("Test Server", std::env::current_exe().unwrap());
        let path = write_desktop_shortcut(&spec, desktop.path()).unwrap();
        assert!(path.is_file());
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111);
        assert!(remove_desktop_shortcut("Test Server", desktop.path()).unwrap());
        assert!(!remove_desktop_shortcut("Test Server", desktop.path()).unwrap());
    }

    #[cfg(windows)]
    #[test]
    fn writes_a_lnk_shortcut_on_windows() {
        let desktop = TempDir::new().unwrap();
        let spec = ShortcutSpec::new("Test Server", std::env::current_exe().unwrap())
            .with_arguments(vec!["--activate".to_string()]);
        let path = write_desktop_shortcut(&spec, desktop.path()).unwrap();
        assert!(path.is_file());
        assert!(path.extension().is_some_and(|extension| extension == "lnk"));
        assert!(remove_desktop_shortcut("Test Server", desktop.path()).unwrap());
    }
}
