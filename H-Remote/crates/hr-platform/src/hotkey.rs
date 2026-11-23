use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HotkeyModifier {
    Control,
    Shift,
    Alt,
    Super,
}

impl HotkeyModifier {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => Some(Self::Control),
            "shift" => Some(Self::Shift),
            "alt" | "option" | "opt" => Some(Self::Alt),
            "super" | "win" | "windows" | "meta" | "cmd" | "command" => Some(Self::Super),
            _ => None,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Control => "Ctrl",
            Self::Shift => "Shift",
            Self::Alt => "Alt",
            Self::Super => "Super",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeySpec {
    pub modifiers: Vec<HotkeyModifier>,
    pub key: String,
}

impl std::fmt::Display for HotkeySpec {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts: Vec<&str> = self
            .modifiers
            .iter()
            .map(|modifier| modifier.label())
            .collect();
        parts.push(&self.key);
        write!(formatter, "{}", parts.join("+"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HotkeyError {
    #[error("hotkey is empty")]
    Empty,
    #[error("unknown modifier '{0}'")]
    UnknownModifier(String),
    #[error("hotkey needs a key after the modifiers")]
    MissingKey,
    #[error("hotkey has more than one key")]
    MultipleKeys,
    #[error("unknown key '{0}'")]
    UnknownKey(String),
    #[error("'{0}' is reserved by the operating system")]
    ReservedByOs(String),
    #[error("'{0}' is already registered")]
    Conflict(String),
    #[error("a modifier is required for '{0}' (function keys may be used alone)")]
    MissingModifier(String),
}

/// Parses a hotkey such as `Alt+M`, `ctrl + shift + F12` or `Super+Space`.
pub fn parse_hotkey(input: &str) -> Result<HotkeySpec, HotkeyError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(HotkeyError::Empty);
    }

    let mut modifiers: Vec<HotkeyModifier> = Vec::new();
    let mut key: Option<String> = None;
    for part in trimmed.split('+') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(modifier) = HotkeyModifier::parse(part) {
            if !modifiers.contains(&modifier) {
                modifiers.push(modifier);
            }
            continue;
        }
        let normalized = normalize_key(part)?;
        if key.replace(normalized).is_some() {
            return Err(HotkeyError::MultipleKeys);
        }
    }

    let key = key.ok_or(HotkeyError::MissingKey)?;
    modifiers.sort();
    let spec = HotkeySpec { modifiers, key };
    validate_spec(&spec)?;
    Ok(spec)
}

fn normalize_key(value: &str) -> Result<String, HotkeyError> {
    let trimmed = value.trim();
    if trimmed.chars().count() == 1 {
        let character = trimmed.chars().next().unwrap_or_default();
        if character.is_ascii_alphanumeric() {
            return Ok(character.to_ascii_uppercase().to_string());
        }
    }

    if let Some(number) = trimmed
        .to_ascii_lowercase()
        .strip_prefix('f')
        .and_then(|number| number.parse::<u8>().ok())
        .filter(|number| (1..=24).contains(number))
    {
        return Ok(format!("F{number}"));
    }

    let named = match trimmed.to_ascii_lowercase().as_str() {
        "space" | "spacebar" => "Space",
        "tab" => "Tab",
        "enter" | "return" => "Enter",
        "escape" | "esc" => "Escape",
        "backspace" | "back" => "Backspace",
        "delete" | "del" => "Delete",
        "insert" | "ins" => "Insert",
        "home" => "Home",
        "end" => "End",
        "pageup" | "pgup" => "PageUp",
        "pagedown" | "pgdn" => "PageDown",
        "arrowup" | "up" => "ArrowUp",
        "arrowdown" | "down" => "ArrowDown",
        "arrowleft" | "left" => "ArrowLeft",
        "arrowright" | "right" => "ArrowRight",
        "printscreen" | "prtsc" => "PrintScreen",
        "pause" => "Pause",
        "capslock" => "CapsLock",
        "numlock" => "NumLock",
        "scrolllock" => "ScrollLock",
        "minus" => "Minus",
        "equal" => "Equal",
        "comma" => "Comma",
        "period" => "Period",
        "slash" => "Slash",
        "backslash" => "Backslash",
        "semicolon" => "Semicolon",
        "quote" => "Quote",
        "bracketleft" => "BracketLeft",
        "bracketright" => "BracketRight",
        "grave" => "Grave",
        _ => return Err(HotkeyError::UnknownKey(trimmed.to_string())),
    };
    Ok(named.to_string())
}

fn validate_spec(spec: &HotkeySpec) -> Result<(), HotkeyError> {
    let canonical = spec.to_string();
    if is_reserved(&canonical) {
        return Err(HotkeyError::ReservedByOs(canonical));
    }
    let is_function_key = spec
        .key
        .strip_prefix('F')
        .and_then(|number| number.parse::<u8>().ok())
        .is_some_and(|number| (13..=24).contains(&number));
    if spec.modifiers.is_empty() && !is_function_key {
        return Err(HotkeyError::MissingModifier(canonical));
    }
    Ok(())
}

fn is_reserved(canonical: &str) -> bool {
    matches!(
        canonical,
        "Ctrl+Alt+Delete" | "Super+L" | "Alt+Tab" | "Super+Tab" | "Ctrl+Shift+Escape"
    )
}

/// Canonical string suitable for persisting in configuration.
pub fn canonical_hotkey(input: &str) -> Result<String, HotkeyError> {
    parse_hotkey(input).map(|spec| spec.to_string())
}

/// Checks a candidate against reserved combinations and already-registered
/// hotkeys (P0-48/P5-03).
pub fn check_hotkey_conflict(existing: &[String], candidate: &str) -> Result<String, HotkeyError> {
    let canonical = canonical_hotkey(candidate)?;
    if existing
        .iter()
        .filter_map(|value| canonical_hotkey(value).ok())
        .any(|registered| registered == canonical)
    {
        return Err(HotkeyError::Conflict(canonical));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_and_aliases() {
        let spec = parse_hotkey("alt+m").unwrap();
        assert_eq!(spec.modifiers, vec![HotkeyModifier::Alt]);
        assert_eq!(spec.key, "M");
        assert_eq!(spec.to_string(), "Alt+M");

        assert_eq!(
            canonical_hotkey("control + shift + f12").unwrap(),
            "Ctrl+Shift+F12"
        );
        assert_eq!(canonical_hotkey("win+space").unwrap(), "Super+Space");
        assert_eq!(
            canonical_hotkey("cmd+option+up").unwrap(),
            "Alt+Super+ArrowUp"
        );
        assert_eq!(canonical_hotkey("Ctrl+Ctrl+M").unwrap(), "Ctrl+M");
    }

    #[test]
    fn canonical_modifier_order_is_stable() {
        assert_eq!(
            canonical_hotkey("Super+Alt+Shift+Ctrl+K").unwrap(),
            "Ctrl+Shift+Alt+Super+K"
        );
    }

    #[test]
    fn rejects_invalid_specs() {
        assert_eq!(parse_hotkey("").unwrap_err(), HotkeyError::Empty);
        assert!(matches!(
            parse_hotkey("Hyper+M"),
            Err(HotkeyError::UnknownKey(_))
        ));
        assert_eq!(
            parse_hotkey("Ctrl+Shift").unwrap_err(),
            HotkeyError::MissingKey
        );
        assert_eq!(
            parse_hotkey("Ctrl+M+N").unwrap_err(),
            HotkeyError::MultipleKeys
        );
        assert!(matches!(
            parse_hotkey("Ctrl+NotAKey"),
            Err(HotkeyError::UnknownKey(_))
        ));
        assert!(matches!(
            parse_hotkey("M"),
            Err(HotkeyError::MissingModifier(_))
        ));
    }

    #[test]
    fn function_keys_may_be_used_alone_but_not_f1_to_f12() {
        assert!(parse_hotkey("F13").is_ok());
        assert!(parse_hotkey("F24").is_ok());
        assert!(parse_hotkey("F1").is_err());
    }

    #[test]
    fn reserved_combinations_are_rejected() {
        assert!(matches!(
            parse_hotkey("Ctrl+Alt+Delete"),
            Err(HotkeyError::ReservedByOs(_))
        ));
        assert!(parse_hotkey("Super+L").is_err());
        assert!(parse_hotkey("Alt+Tab").is_err());
    }

    #[test]
    fn conflict_detection_is_case_insensitive_and_alias_aware() {
        let existing = vec!["Alt+M".to_string(), "control+shift+f12".to_string()];
        assert!(matches!(
            check_hotkey_conflict(&existing, "alt + m"),
            Err(HotkeyError::Conflict(_))
        ));
        assert!(check_hotkey_conflict(&existing, "Ctrl+Alt+M").is_ok());
        assert!(check_hotkey_conflict(&[], "Alt+M").is_ok());
    }

    #[test]
    fn spec_round_trips_through_json() {
        let spec = parse_hotkey("Alt+M").unwrap();
        let decoded: HotkeySpec =
            serde_json::from_str(&serde_json::to_string(&spec).unwrap()).unwrap();
        assert_eq!(decoded, spec);
    }
}
