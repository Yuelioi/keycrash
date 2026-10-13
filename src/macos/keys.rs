// Carbon modifier bits and NSEvent hardware key codes, not Windows VK values.
const FUNCTION_KEYS: [u16; 20] = [
    0x7A, 0x78, 0x63, 0x76, 0x60, 0x61, 0x62, 0x64, 0x65, 0x6D, 0x67, 0x6F, 0x69, 0x6B, 0x71, 0x6A,
    0x40, 0x4F, 0x50, 0x5A,
];

pub fn is_function_key(code: u16) -> bool {
    FUNCTION_KEYS.contains(&code)
}

pub fn modifiers(ctrl: bool, option: bool, shift: bool, command: bool) -> u32 {
    (u32::from(ctrl) << 12)
        | (u32::from(option) << 11)
        | (u32::from(shift) << 9)
        | (u32::from(command) << 8)
}

pub fn shortcut_label(modifiers: u32, key: &str) -> String {
    let mut parts = Vec::new();
    for (bit, name) in [(12, "⌃"), (11, "⌥"), (9, "⇧"), (8, "⌘")] {
        if modifiers & (1 << bit) != 0 {
            parts.push(name);
        }
    }
    parts.push(key);
    parts.join(" + ")
}

pub fn key_label(code: u16, characters: &str) -> String {
    let special = match code {
        0x24 => "Return",
        0x30 => "Tab",
        0x31 => "Space",
        0x33 => "Delete",
        0x35 => "Esc",
        0x75 => "Forward Delete",
        0x73 => "Home",
        0x77 => "End",
        0x74 => "Page Up",
        0x79 => "Page Down",
        0x7B => "←",
        0x7C => "→",
        0x7D => "↓",
        0x7E => "↑",
        0x47 => "Clear",
        0x4C => "Num Enter",
        0x41 => "Num .",
        0x43 => "Num *",
        0x45 => "Num +",
        0x4B => "Num /",
        0x4E => "Num -",
        0x51 => "Num =",
        0x52 => "Num 0",
        0x53 => "Num 1",
        0x54 => "Num 2",
        0x55 => "Num 3",
        0x56 => "Num 4",
        0x57 => "Num 5",
        0x58 => "Num 6",
        0x59 => "Num 7",
        0x5B => "Num 8",
        0x5C => "Num 9",
        _ => "",
    };
    if !special.is_empty() {
        return special.into();
    }
    // F1..F20 are not contiguous in the macOS key-code table.
    if let Some(index) = FUNCTION_KEYS.iter().position(|key| *key == code) {
        return format!("F{}", index + 1);
    }
    if !characters.is_empty()
        && characters
            .chars()
            .all(|c| !c.is_control() && !(0xF700..=0xF8FF).contains(&(c as u32)))
    {
        return characters.to_uppercase();
    }
    // Dead keys / IMEs need not produce text. Keep the actual physical identity.
    format!("Key 0x{code:02X}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_modifiers_and_labels_preserve_command_and_control() {
        assert_eq!(modifiers(true, true, true, true), 0x1B00);
        assert_eq!(
            shortcut_label(modifiers(true, false, false, true), "K"),
            "⌃ + ⌘ + K"
        );
        assert_eq!(shortcut_label(0, "A"), "A");
    }

    #[test]
    fn uses_layout_text_without_changing_the_hardware_key() {
        assert!(is_function_key(0x7A));
        assert!(!is_function_key(0x33)); // Fn+Delete must not become plain Delete.
        assert_eq!(key_label(0x00, "q"), "Q"); // AZERTY
        assert_eq!(key_label(0x00, "a"), "A"); // QWERTY
        assert_eq!(key_label(0x21, ""), "Key 0x21"); // dead key
        assert_eq!(key_label(0x6F, "\u{F70F}"), "F12");
        assert_eq!(key_label(0x33, "\u{7F}"), "Delete");
        assert_eq!(key_label(0x75, "\u{F728}"), "Forward Delete");
        assert_eq!(key_label(0x53, "1"), "Num 1");
    }
}
