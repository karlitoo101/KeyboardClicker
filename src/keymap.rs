//! Maps `rdev::Key` variants to the numeric key codes used by Mechvibes sound
//! packs (which are libuiohook-style PC/XT scan codes).
//!
//! The codes here match the `defines` keys in a Mechvibes `config.json`
//! (e.g. Space -> 57, Enter -> 28, A -> 30). Any key without a mapping falls
//! back to a default slice chosen by the caller.

use rdev::Key;

/// Return the Mechvibes key code for a given `rdev` key, if one is defined.
pub fn code_for(key: Key) -> Option<u32> {
    let code = match key {
        Key::Escape => 1,
        Key::Num1 => 2,
        Key::Num2 => 3,
        Key::Num3 => 4,
        Key::Num4 => 5,
        Key::Num5 => 6,
        Key::Num6 => 7,
        Key::Num7 => 8,
        Key::Num8 => 9,
        Key::Num9 => 10,
        Key::Num0 => 11,
        Key::Minus => 12,
        Key::Equal => 13,
        Key::Backspace => 14,
        Key::Tab => 15,
        Key::KeyQ => 16,
        Key::KeyW => 17,
        Key::KeyE => 18,
        Key::KeyR => 19,
        Key::KeyT => 20,
        Key::KeyY => 21,
        Key::KeyU => 22,
        Key::KeyI => 23,
        Key::KeyO => 24,
        Key::KeyP => 25,
        Key::LeftBracket => 26,
        Key::RightBracket => 27,
        Key::Return => 28,
        Key::ControlLeft => 29,
        Key::KeyA => 30,
        Key::KeyS => 31,
        Key::KeyD => 32,
        Key::KeyF => 33,
        Key::KeyG => 34,
        Key::KeyH => 35,
        Key::KeyJ => 36,
        Key::KeyK => 37,
        Key::KeyL => 38,
        Key::SemiColon => 39,
        Key::Quote => 40,
        Key::BackQuote => 41,
        Key::ShiftLeft => 42,
        Key::BackSlash => 43,
        Key::KeyZ => 44,
        Key::KeyX => 45,
        Key::KeyC => 46,
        Key::KeyV => 47,
        Key::KeyB => 48,
        Key::KeyN => 49,
        Key::KeyM => 50,
        Key::Comma => 51,
        Key::Dot => 52,
        Key::Slash => 53,
        Key::ShiftRight => 54,
        Key::KpMultiply => 55,
        Key::Alt => 56,
        Key::Space => 57,
        Key::CapsLock => 58,
        Key::F1 => 59,
        Key::F2 => 60,
        Key::F3 => 61,
        Key::F4 => 62,
        Key::F5 => 63,
        Key::F6 => 64,
        Key::F7 => 65,
        Key::F8 => 66,
        Key::F9 => 67,
        Key::F10 => 68,
        Key::NumLock => 69,
        Key::ScrollLock => 70,
        Key::Kp7 => 71,
        Key::Kp8 => 72,
        Key::Kp9 => 73,
        Key::KpMinus => 74,
        Key::Kp4 => 75,
        Key::Kp5 => 76,
        Key::Kp6 => 77,
        Key::KpPlus => 78,
        Key::Kp1 => 79,
        Key::Kp2 => 80,
        Key::Kp3 => 81,
        Key::Kp0 => 82,
        Key::KpDelete => 83,
        Key::F11 => 87,
        Key::F12 => 88,
        // Extended keys use Mechvibes' e0-prefixed style codes (3600 + base).
        Key::KpReturn => 3612,
        Key::ControlRight => 3613,
        Key::KpDivide => 3637,
        Key::PrintScreen => 3639,
        Key::AltGr => 3640,
        Key::Home => 3653,
        Key::UpArrow => 3655,
        Key::PageUp => 3657,
        Key::LeftArrow => 3663,
        Key::RightArrow => 3665,
        Key::End => 3666,
        Key::DownArrow => 3667,
        Key::PageDown => 3657,
        Key::Insert => 3666,
        Key::Delete => 3675,
        Key::MetaLeft => 3675,
        Key::MetaRight => 3676,
        _ => return None,
    };
    Some(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_keys_map_to_expected_codes() {
        assert_eq!(code_for(Key::Space), Some(57));
        assert_eq!(code_for(Key::Return), Some(28));
        assert_eq!(code_for(Key::KeyA), Some(30));
        assert_eq!(code_for(Key::Backspace), Some(14));
        assert_eq!(code_for(Key::Escape), Some(1));
        assert_eq!(code_for(Key::Num1), Some(2));
    }

    #[test]
    fn letter_row_is_contiguous() {
        // Q..P should be 16..=25.
        assert_eq!(code_for(Key::KeyQ), Some(16));
        assert_eq!(code_for(Key::KeyP), Some(25));
    }
}
