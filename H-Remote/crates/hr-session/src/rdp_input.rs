//! Browser/desktop input mapping for the embedded RDP engine (P3 input fix).
//!
//! The webview reports physical `KeyboardEvent.code` names and DOM mouse
//! buttons; IronRDP expects PS/2 scan codes and `ironrdp_input::Operation`s.
//! Keeping the mapping here makes it testable without a live session.

use ironrdp_input::{MouseButton, MousePosition, Operation, Scancode, WheelRotations};

/// Maps a DOM `KeyboardEvent.code` to a PS/2 set-1 scan code.
///
/// Only physical keys are mapped; layout-dependent characters fall back to
/// [`Operation::UnicodeKeyPressed`] in [`key_operation`].
pub fn scancode_for_code(code: &str) -> Option<Scancode> {
    let plain = |code: u8| Some(Scancode::from_u8(false, code));
    let extended = |code: u8| Some(Scancode::from_u8(true, code));
    match code {
        "Escape" => plain(0x01),
        "Digit1" => plain(0x02),
        "Digit2" => plain(0x03),
        "Digit3" => plain(0x04),
        "Digit4" => plain(0x05),
        "Digit5" => plain(0x06),
        "Digit6" => plain(0x07),
        "Digit7" => plain(0x08),
        "Digit8" => plain(0x09),
        "Digit9" => plain(0x0A),
        "Digit0" => plain(0x0B),
        "Minus" => plain(0x0C),
        "Equal" => plain(0x0D),
        "Backspace" => plain(0x0E),
        "Tab" => plain(0x0F),
        "KeyQ" => plain(0x10),
        "KeyW" => plain(0x11),
        "KeyE" => plain(0x12),
        "KeyR" => plain(0x13),
        "KeyT" => plain(0x14),
        "KeyY" => plain(0x15),
        "KeyU" => plain(0x16),
        "KeyI" => plain(0x17),
        "KeyO" => plain(0x18),
        "KeyP" => plain(0x19),
        "BracketLeft" => plain(0x1A),
        "BracketRight" => plain(0x1B),
        "Enter" => plain(0x1C),
        "ControlLeft" => plain(0x1D),
        "KeyA" => plain(0x1E),
        "KeyS" => plain(0x1F),
        "KeyD" => plain(0x20),
        "KeyF" => plain(0x21),
        "KeyG" => plain(0x22),
        "KeyH" => plain(0x23),
        "KeyJ" => plain(0x24),
        "KeyK" => plain(0x25),
        "KeyL" => plain(0x26),
        "Semicolon" => plain(0x27),
        "Quote" => plain(0x28),
        "Backquote" => plain(0x29),
        "ShiftLeft" => plain(0x2A),
        "Backslash" => plain(0x2B),
        "KeyZ" => plain(0x2C),
        "KeyX" => plain(0x2D),
        "KeyC" => plain(0x2E),
        "KeyV" => plain(0x2F),
        "KeyB" => plain(0x30),
        "KeyN" => plain(0x31),
        "KeyM" => plain(0x32),
        "Comma" => plain(0x33),
        "Period" => plain(0x34),
        "Slash" => plain(0x35),
        "ShiftRight" => plain(0x36),
        "NumpadMultiply" => plain(0x37),
        "AltLeft" => plain(0x38),
        "Space" => plain(0x39),
        "CapsLock" => plain(0x3A),
        "F1" => plain(0x3B),
        "F2" => plain(0x3C),
        "F3" => plain(0x3D),
        "F4" => plain(0x3E),
        "F5" => plain(0x3F),
        "F6" => plain(0x40),
        "F7" => plain(0x41),
        "F8" => plain(0x42),
        "F9" => plain(0x43),
        "F10" => plain(0x44),
        "NumLock" => plain(0x45),
        "ScrollLock" => plain(0x46),
        "Numpad7" => plain(0x47),
        "Numpad8" => plain(0x48),
        "Numpad9" => plain(0x49),
        "NumpadSubtract" => plain(0x4A),
        "Numpad4" => plain(0x4B),
        "Numpad5" => plain(0x4C),
        "Numpad6" => plain(0x4D),
        "NumpadAdd" => plain(0x4E),
        "Numpad1" => plain(0x4F),
        "Numpad2" => plain(0x50),
        "Numpad3" => plain(0x51),
        "Numpad0" => plain(0x52),
        "NumpadDecimal" => plain(0x53),
        "F11" => plain(0x57),
        "F12" => plain(0x58),
        "IntlBackslash" => plain(0x56),
        "ContextMenu" => extended(0x5D),
        "ControlRight" => extended(0x1D),
        "AltRight" => extended(0x38),
        "PrintScreen" => extended(0x37),
        "ArrowLeft" => extended(0x4B),
        "ArrowRight" => extended(0x4D),
        "ArrowUp" => extended(0x48),
        "ArrowDown" => extended(0x50),
        "Home" => extended(0x47),
        "End" => extended(0x4F),
        "PageUp" => extended(0x49),
        "PageDown" => extended(0x51),
        "Insert" => extended(0x52),
        "Delete" => extended(0x53),
        "MetaLeft" => extended(0x5B),
        "MetaRight" => extended(0x5C),
        "NumpadDivide" => extended(0x35),
        "NumpadEnter" => extended(0x1C),
        _ => None,
    }
}

/// Builds the operation for a key transition. Unmapped printable characters use
/// Unicode input so international layouts still reach the remote.
pub fn key_operation(code: &str, key: &str, down: bool) -> Option<Operation> {
    if let Some(scancode) = scancode_for_code(code) {
        return Some(if down {
            Operation::KeyPressed(scancode)
        } else {
            Operation::KeyReleased(scancode)
        });
    }
    let mut characters = key.chars();
    let character = characters.next()?;
    if characters.next().is_some() {
        return None;
    }
    Some(if down {
        Operation::UnicodeKeyPressed(character)
    } else {
        Operation::UnicodeKeyReleased(character)
    })
}

pub fn pointer_move(x: u16, y: u16) -> Operation {
    Operation::MouseMove(MousePosition { x, y })
}

pub fn button_operation(button: u8, down: bool) -> Option<Operation> {
    let button = MouseButton::from_web_button(button)?;
    Some(if down {
        Operation::MouseButtonPressed(button)
    } else {
        Operation::MouseButtonReleased(button)
    })
}

pub fn wheel_operation(vertical: bool, units: i16) -> Operation {
    Operation::WheelRotations(WheelRotations {
        is_vertical: vertical,
        rotation_units: units,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_letters_digits_and_modifiers() {
        assert_eq!(scancode_for_code("KeyA").unwrap().as_u8(), (false, 0x1E));
        assert_eq!(scancode_for_code("Digit1").unwrap().as_u8(), (false, 0x02));
        assert_eq!(
            scancode_for_code("ControlRight").unwrap().as_u8(),
            (true, 0x1D)
        );
        assert_eq!(scancode_for_code("F11").unwrap().as_u8(), (false, 0x57));
        assert_eq!(
            scancode_for_code("ArrowLeft").unwrap().as_u8(),
            (true, 0x4B)
        );
    }

    #[test]
    fn keys_round_trip_through_press_and_release() {
        let press = key_operation("KeyQ", "q", true).unwrap();
        let release = key_operation("KeyQ", "q", false).unwrap();
        assert!(matches!(press, Operation::KeyPressed(_)));
        assert!(matches!(release, Operation::KeyReleased(_)));
    }

    #[test]
    fn unmapped_printable_keys_fall_back_to_unicode() {
        assert!(matches!(
            key_operation("Unidentified", "é", true),
            Some(Operation::UnicodeKeyPressed('é'))
        ));
        assert!(key_operation("Dead", "Dead", true).is_none());
        assert!(key_operation("ShiftLeft", "Shift", true).is_some());
    }

    #[test]
    fn mouse_operations_cover_buttons_and_wheel() {
        assert!(matches!(
            button_operation(0, true),
            Some(Operation::MouseButtonPressed(MouseButton::Left))
        ));
        assert!(matches!(
            button_operation(2, false),
            Some(Operation::MouseButtonReleased(MouseButton::Right))
        ));
        assert!(button_operation(9, true).is_none());
        assert!(matches!(
            wheel_operation(true, -120),
            Operation::WheelRotations(WheelRotations {
                is_vertical: true,
                rotation_units: -120
            })
        ));
    }
}
