use hr_core::{is_valid_id, new_id};
use serde::{Deserialize, Serialize};

/// One command-line/shortcut instruction. Parsing is case-insensitive and
/// accepts both `--flag` and `flag` forms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum CliAction {
    Activate,
    StartMinimized,
    Install,
    Uninstall,
    InstallDesktopShortcut,
    UninstallDesktopShortcut,
    InstallStartup,
    UninstallStartup,
    ConnectById(String),
    ConnectByTag(String),
    ConnectByName(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliInvocation {
    pub actions: Vec<CliAction>,
    pub errors: Vec<String>,
    pub raw: Vec<String>,
}

impl CliInvocation {
    pub fn wants_start_minimized(&self) -> bool {
        self.actions.contains(&CliAction::StartMinimized)
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Connection targets in the order supplied, suitable for deferred
    /// processing after data has loaded (P0-57).
    pub fn connection_actions(&self) -> impl Iterator<Item = &CliAction> {
        self.actions.iter().filter(|action| {
            matches!(
                action,
                CliAction::ConnectById(_)
                    | CliAction::ConnectByTag(_)
                    | CliAction::ConnectByName(_)
            )
        })
    }
}

/// Parses arguments excluding the executable path.
///
/// Multiple values may be passed as separate arguments or comma/semicolon
/// separated within one argument. Invalid values are reported but do not
/// discard the valid actions.
pub fn parse_cli_args<I, S>(args: I) -> CliInvocation
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut invocation = CliInvocation::default();
    for argument in args {
        let argument = argument.into();
        invocation.raw.push(argument.clone());
        for token in argument.split([',', ';']) {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }
            match parse_token(token) {
                Ok(Some(action)) => {
                    if !invocation.actions.contains(&action) {
                        invocation.actions.push(action);
                    }
                }
                Ok(None) => {}
                Err(error) => invocation.errors.push(error),
            }
        }
    }
    invocation
}

fn parse_token(token: &str) -> Result<Option<CliAction>, String> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let lower = trimmed.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("ulid:") {
        let raw_id = trimmed[5..].trim();
        let _ = rest;
        if !is_valid_id(&raw_id.to_ascii_uppercase()) {
            return Err(format!("invalid server id: {raw_id}"));
        }
        return Ok(Some(CliAction::ConnectById(raw_id.to_ascii_uppercase())));
    }

    if let Some(tag) = trimmed.strip_prefix('#') {
        let tag = tag.trim().trim_start_matches('#').to_lowercase();
        if tag.is_empty() {
            return Err("empty tag argument".to_string());
        }
        return Ok(Some(CliAction::ConnectByTag(tag)));
    }

    let switch = lower.trim_start_matches('-');
    let action = match switch {
        "activate" => Some(CliAction::Activate),
        "start-minimized" | "startminimized" | "start-minimised" | "startminimised" => {
            Some(CliAction::StartMinimized)
        }
        "install" => Some(CliAction::Install),
        "uninstall" => Some(CliAction::Uninstall),
        "install-desktop-shortcut" | "installdesktopshortcut" => {
            Some(CliAction::InstallDesktopShortcut)
        }
        "uninstall-desktop-shortcut" | "uninstalldesktopshortcut" => {
            Some(CliAction::UninstallDesktopShortcut)
        }
        "install-startup" | "installstartup" => Some(CliAction::InstallStartup),
        "uninstall-startup" | "uninstallstartup" => Some(CliAction::UninstallStartup),
        _ => None,
    };
    if let Some(action) = action {
        return Ok(Some(action));
    }

    if trimmed.starts_with('-') {
        return Err(format!("unknown argument: {trimmed}"));
    }

    Ok(Some(CliAction::ConnectByName(trimmed.to_string())))
}

/// Helper used by tests and diagnostics to build a ULID-prefixed action.
pub fn ulid_argument(id: &str) -> String {
    format!("ULID:{id}")
}

/// Generates a sample invocation for the About/diagnostics page.
pub fn example_invocation() -> CliInvocation {
    parse_cli_args([
        "--activate",
        &ulid_argument(&new_id()),
        "#prod",
        "--start-minimized",
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_documented_switch_case_insensitively() {
        let invocation = parse_cli_args([
            "ACTIVATE",
            "--Start-Minimized",
            "install",
            "UNINSTALL",
            "install-desktop-shortcut",
            "uninstall-desktop-shortcut",
            "install-startup",
            "uninstall-startup",
        ]);
        assert!(invocation.errors.is_empty());
        assert_eq!(
            invocation.actions,
            vec![
                CliAction::Activate,
                CliAction::StartMinimized,
                CliAction::Install,
                CliAction::Uninstall,
                CliAction::InstallDesktopShortcut,
                CliAction::UninstallDesktopShortcut,
                CliAction::InstallStartup,
                CliAction::UninstallStartup,
            ]
        );
        assert!(invocation.wants_start_minimized());
    }

    #[test]
    fn parses_ulid_tag_and_display_name_connections() {
        let id = new_id();
        let invocation = parse_cli_args([
            format!("ulid:{id}"),
            "#Prod".to_string(),
            "My Workstation".to_string(),
        ]);
        assert!(invocation.errors.is_empty());
        assert_eq!(
            invocation.actions,
            vec![
                CliAction::ConnectById(id.to_ascii_uppercase()),
                CliAction::ConnectByTag("prod".to_string()),
                CliAction::ConnectByName("My Workstation".to_string()),
            ]
        );
        assert_eq!(invocation.connection_actions().count(), 3);
    }

    #[test]
    fn splits_comma_and_semicolon_lists_and_deduplicates() {
        let invocation = parse_cli_args(["--activate, --start-minimized; --activate".to_string()]);
        assert_eq!(
            invocation.actions,
            vec![CliAction::Activate, CliAction::StartMinimized]
        );
    }

    #[test]
    fn invalid_ulid_and_unknown_switches_are_reported_but_do_not_drop_valid_actions() {
        let invocation = parse_cli_args(["ULID:not-an-id", "--nonsense", "activate"]);
        assert_eq!(invocation.errors.len(), 2);
        assert_eq!(invocation.actions, vec![CliAction::Activate]);
    }

    #[test]
    fn empty_and_whitespace_arguments_are_ignored() {
        let invocation = parse_cli_args(["", "   ", ",", ";"]);
        assert!(invocation.is_empty());
        assert!(invocation.errors.is_empty());
    }

    #[test]
    fn start_minimized_aliases_are_accepted() {
        for alias in [
            "--start-minimized",
            "startminimized",
            "Start-Minimised",
            "startminimised",
        ] {
            let invocation = parse_cli_args([alias]);
            assert_eq!(
                invocation.actions,
                vec![CliAction::StartMinimized],
                "{alias}"
            );
        }
    }

    #[test]
    fn invocation_round_trips_through_json() {
        let invocation = parse_cli_args(["#prod", "--activate"]);
        let encoded = serde_json::to_string(&invocation).unwrap();
        let decoded: CliInvocation = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, invocation);
    }

    #[test]
    fn example_invocation_is_valid() {
        let invocation = example_invocation();
        assert!(invocation.errors.is_empty());
        assert_eq!(invocation.actions.len(), 4);
    }
}
