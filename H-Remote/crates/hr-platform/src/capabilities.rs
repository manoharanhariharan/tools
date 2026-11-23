use serde::{Deserialize, Serialize};

/// Cross-platform view of what the current OS can do. The UI uses this to
/// hide or explain unavailable features (never to silently mislead).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub os: String,
    pub arch: String,
    pub os_keyring: bool,
    pub windows_hello: bool,
    pub registry_autostart: bool,
    pub shortcut_flavor: ShortcutFlavor,
    pub hwnd_embedding: bool,
    pub native_rdp_fallback: bool,
    pub embedded_rdp: bool,
    pub global_hotkey: bool,
    pub tray: bool,
    pub working_set_trim: bool,
    pub supports_multiple_monitors: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShortcutFlavor {
    Lnk,
    DesktopEntry,
    CommandScript,
}

impl ShortcutFlavor {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Lnk => "lnk",
            Self::DesktopEntry => "desktop",
            Self::CommandScript => "command",
        }
    }
}

pub fn detect_capabilities() -> Capabilities {
    Capabilities {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        os_keyring: true,
        windows_hello: cfg!(windows),
        registry_autostart: cfg!(windows),
        shortcut_flavor: shortcut_flavor(),
        hwnd_embedding: cfg!(windows),
        native_rdp_fallback: cfg!(windows),
        embedded_rdp: true,
        global_hotkey: true,
        tray: true,
        working_set_trim: cfg!(any(windows, target_os = "linux")),
        supports_multiple_monitors: true,
    }
}

pub const fn shortcut_flavor() -> ShortcutFlavor {
    #[cfg(windows)]
    {
        ShortcutFlavor::Lnk
    }
    #[cfg(target_os = "macos")]
    {
        ShortcutFlavor::CommandScript
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        ShortcutFlavor::DesktopEntry
    }
    #[cfg(not(any(windows, unix)))]
    {
        ShortcutFlavor::CommandScript
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_are_serialisable_and_platform_aware() {
        let capabilities = detect_capabilities();
        let json = serde_json::to_value(&capabilities).unwrap();
        assert_eq!(json["os"], std::env::consts::OS);
        assert_eq!(json["embeddedRdp"], true);
        assert_eq!(json["globalHotkey"], true);
        assert_eq!(capabilities.hwnd_embedding, cfg!(windows));
        assert_eq!(capabilities.windows_hello, cfg!(windows));
        assert_eq!(
            capabilities.shortcut_flavor.extension(),
            shortcut_flavor().extension()
        );
    }

    #[test]
    fn shortcut_extension_matches_platform() {
        let extension = shortcut_flavor().extension();
        assert!(["lnk", "desktop", "command"].contains(&extension));
    }
}
