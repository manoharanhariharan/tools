//! Autostart specifications.
//!
//! The Tauri shell applies these through `tauri-plugin-autostart`, which uses
//! the registry on Windows, a LaunchAgent on macOS and XDG autostart on Linux.
//! The pure generators below are unit-tested and used for diagnostics/export.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::shortcuts::ShortcutSpec;

pub const AUTOSTART_LABEL: &str = "H-Remote";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutostartSpec {
    pub name: String,
    pub target: PathBuf,
    pub arguments: Vec<String>,
    pub hidden: bool,
}

impl AutostartSpec {
    pub fn new(name: impl Into<String>, target: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            target: target.into(),
            arguments: vec!["--start-minimized".to_string()],
            hidden: true,
        }
    }

    pub fn from_shortcut(spec: &ShortcutSpec) -> Self {
        Self {
            name: spec.name.clone(),
            target: spec.target.clone(),
            arguments: if spec.arguments.is_empty() {
                vec!["--start-minimized".to_string()]
            } else {
                spec.arguments.clone()
            },
            hidden: true,
        }
    }
}

pub fn autostart_file_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "com.hremote.app.plist"
    }
    #[cfg(not(target_os = "macos"))]
    {
        "h-remote.desktop"
    }
}

/// XDG autostart entry (Linux).
pub fn linux_autostart_content(spec: &AutostartSpec) -> String {
    let exec = std::iter::once(spec.target.display().to_string())
        .chain(spec.arguments.iter().cloned())
        .map(|value| {
            let escaped = value
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('`', "\\`")
                .replace('$', "\\$");
            format!("\"{escaped}\"")
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name={}\n\
         Comment=Start H-Remote with the desktop session\n\
         Exec={exec}\n\
         Terminal=false\n\
         Hidden=false\n\
         X-GNOME-Autostart-enabled=true\n",
        spec.name.replace(['\r', '\n'], " ")
    )
}

/// macOS LaunchAgent property list.
pub fn macos_launch_agent_plist(spec: &AutostartSpec, label: &str) -> String {
    let mut arguments = format!(
        "        <string>{}</string>\n",
        xml_escape(&spec.target.display().to_string())
    );
    for argument in &spec.arguments {
        arguments.push_str(&format!(
            "        <string>{}</string>\n",
            xml_escape(argument)
        ));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
           <key>Label</key>\n\
           <string>{}</string>\n\
           <key>ProgramArguments</key>\n\
           <array>\n{}  </array>\n\
           <key>RunAtLoad</key>\n\
           <true/>\n\
           <key>KeepAlive</key>\n\
           <false/>\n\
         </dict>\n\
         </plist>\n",
        xml_escape(label),
        arguments
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_autostart_entry_quotes_paths() {
        let spec = AutostartSpec::new("H-Remote", "/opt/My App/hremote");
        let content = linux_autostart_content(&spec);
        assert!(content.contains("Type=Application"));
        assert!(content.contains("Exec=\"/opt/My App/hremote\" \"--start-minimized\""));
        assert!(content.contains("X-GNOME-Autostart-enabled=true"));
    }

    #[test]
    fn macos_plist_escapes_xml() {
        let mut spec = AutostartSpec::new("H-Remote", "/Applications/H & R/hremote");
        spec.arguments = vec!["--ULID:<&>".to_string()];
        let plist = macos_launch_agent_plist(&spec, "com.hremote.app");
        assert!(plist.contains("com.hremote.app"));
        assert!(plist.contains("H &amp; R"));
        assert!(plist.contains("&lt;&amp;&gt;"));
        assert!(plist.contains("<key>RunAtLoad</key>"));
    }

    #[test]
    fn shortcut_conversion_produces_startup_args() {
        let shortcut = ShortcutSpec::new("Prod", "/bin/hremote");
        let spec = AutostartSpec::from_shortcut(&shortcut);
        assert_eq!(spec.arguments, vec!["--start-minimized"]);
        assert!(spec.hidden);
    }

    #[test]
    fn file_name_matches_platform() {
        let name = autostart_file_name();
        if cfg!(target_os = "macos") {
            assert!(name.ends_with(".plist"));
        } else {
            assert!(name.ends_with(".desktop"));
        }
    }
}
