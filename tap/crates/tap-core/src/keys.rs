//! Canonical key-name vocabulary.
//!
//! Recording hooks (rdev / Core Graphics) and hand-written macros all spell
//! keys differently (`ControlLeft`, `lctrl`, `Return`, `Kp5`, ...). The domain
//! stores a single canonical token per key so a macro captured on one OS
//! replays the same on another. Both the recorder (capture time) and the
//! injector (replay time) funnel key names through [`normalize_key`].

/// Normalize a key name from any platform hook or user input into a canonical
/// token.
///
/// Modifier and named keys collapse to a stable spelling (`Ctrl`, `Shift`,
/// `Meta`, `Enter`, ...); keypad keys map to their plain equivalents (`Kp5` →
/// `5`); single printable characters and already-canonical names pass through
/// unchanged. Normalization is idempotent: `normalize_key(normalize_key(k)) ==
/// normalize_key(k)`.
pub fn normalize_key(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        // Whitespace-only historically meant the space bar.
        return if raw.is_empty() {
            String::new()
        } else {
            "Space".to_string()
        };
    }

    match trimmed.to_lowercase().as_str() {
        // Modifiers
        "controlleft" | "lcontrol" | "lctrl" | "control" | "ctrl" => "Ctrl".to_string(),
        "controlright" | "rcontrol" | "rctrl" => "RCtrl".to_string(),
        "shiftleft" | "lshift" | "shift" => "Shift".to_string(),
        "shiftright" | "rshift" => "RShift".to_string(),
        "alt" | "lalt" | "altleft" | "option" => "Alt".to_string(),
        "altgr" | "ralt" | "altright" => "Alt".to_string(),
        "metaleft" | "metaright" | "meta" | "command" | "cmd" | "win" | "super" => {
            "Meta".to_string()
        }
        // Navigation / editing
        "return" | "enter" | "kpreturn" => "Enter".to_string(),
        "escape" | "esc" => "Escape".to_string(),
        "up" | "uparrow" => "Up".to_string(),
        "down" | "downarrow" => "Down".to_string(),
        "left" | "leftarrow" => "Left".to_string(),
        "right" | "rightarrow" => "Right".to_string(),
        "pageup" | "pgup" => "PageUp".to_string(),
        "pagedown" | "pgdn" => "PageDown".to_string(),
        "backspace" | "back" => "Backspace".to_string(),
        "delete" | "del" | "kpdelete" => "Delete".to_string(),
        "space" => "Space".to_string(),
        "tab" => "Tab".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "capslock" | "caps" => "CapsLock".to_string(),
        // Keypad digits / operators map to their plain equivalents.
        "kp0" => "0".to_string(),
        "kp1" => "1".to_string(),
        "kp2" => "2".to_string(),
        "kp3" => "3".to_string(),
        "kp4" => "4".to_string(),
        "kp5" => "5".to_string(),
        "kp6" => "6".to_string(),
        "kp7" => "7".to_string(),
        "kp8" => "8".to_string(),
        "kp9" => "9".to_string(),
        "kpminus" => "-".to_string(),
        "kpplus" => "+".to_string(),
        "kpmultiply" => "*".to_string(),
        "kpdivide" => "/".to_string(),
        // Function keys, punctuation, single letters, and anything unknown:
        // keep the original token (trimmed).
        _ => trimmed.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_maps_hook_modifier_names() {
        assert_eq!(normalize_key("ControlLeft"), "Ctrl");
        assert_eq!(normalize_key("ControlRight"), "RCtrl");
        assert_eq!(normalize_key("ShiftLeft"), "Shift");
        assert_eq!(normalize_key("ShiftRight"), "RShift");
        assert_eq!(normalize_key("MetaLeft"), "Meta");
        assert_eq!(normalize_key("AltGr"), "Alt");
        assert_eq!(normalize_key("Return"), "Enter");
        assert_eq!(normalize_key("Kp5"), "5");
        // Single chars and function keys pass through.
        assert_eq!(normalize_key("a"), "a");
        assert_eq!(normalize_key("F5"), "F5");
        // Whitespace-only is treated as the space bar.
        assert_eq!(normalize_key(" "), "Space");
    }

    #[test]
    fn normalize_is_idempotent() {
        for k in ["ControlLeft", "Kp5", "Return", "a", "F5", " ", "Shift"] {
            let once = normalize_key(k);
            assert_eq!(normalize_key(&once), once, "not idempotent for {k:?}");
        }
    }
}
