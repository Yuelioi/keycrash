use windows::Win32::Foundation::GetLastError;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey,
    UnregisterHotKey,
};

const PROBE_ID: i32 = 0x4B43;
const ERROR_HOTKEY_ALREADY_REGISTERED_CODE: u32 = 1409;

const VK_BACK: u32 = 0x08;
const VK_TAB: u32 = 0x09;
const VK_RETURN: u32 = 0x0D;
const VK_PAUSE: u32 = 0x13;
const VK_CAPITAL: u32 = 0x14;
const VK_ESCAPE: u32 = 0x1B;
const VK_SPACE: u32 = 0x20;
const VK_PRIOR: u32 = 0x21;
const VK_NEXT: u32 = 0x22;
const VK_END: u32 = 0x23;
const VK_HOME: u32 = 0x24;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;
const VK_SNAPSHOT: u32 = 0x2C;
const VK_INSERT: u32 = 0x2D;
const VK_DELETE: u32 = 0x2E;
const VK_NUMPAD0: u32 = 0x60;
const VK_NUMPAD9: u32 = 0x69;
const VK_MULTIPLY: u32 = 0x6A;
const VK_ADD: u32 = 0x6B;
const VK_SUBTRACT: u32 = 0x6D;
const VK_DECIMAL: u32 = 0x6E;
const VK_DIVIDE: u32 = 0x6F;
const VK_F1: u32 = 0x70;
const VK_F24: u32 = 0x87;
const VK_NUMLOCK: u32 = 0x90;
const VK_SCROLL: u32 = 0x91;
const VK_OEM_1: u32 = 0xBA;
const VK_OEM_PLUS: u32 = 0xBB;
const VK_OEM_COMMA: u32 = 0xBC;
const VK_OEM_MINUS: u32 = 0xBD;
const VK_OEM_PERIOD: u32 = 0xBE;
const VK_OEM_2: u32 = 0xBF;
const VK_OEM_3: u32 = 0xC0;
const VK_OEM_4: u32 = 0xDB;
const VK_OEM_5: u32 = 0xDC;
const VK_OEM_6: u32 = 0xDD;
const VK_OEM_7: u32 = 0xDE;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub modifiers: u32,
    pub virtual_key: u32,
}

impl Shortcut {
    pub fn label(&self) -> String {
        format_shortcut(self.modifiers, self.virtual_key)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionKeyParseError {
    Empty,
    ModifierInInput,
    Unknown(String),
}

impl ActionKeyParseError {
    pub fn message(&self) -> String {
        match self {
            Self::Empty => "请按一个目标键，例如 A、F12 或 Space。".into(),
            Self::ModifierInInput => "Ctrl、Alt、Shift 请使用上方标签选择，只按目标键。".into(),
            Self::Unknown(value) => format!("无法识别目标键“{value}”，请换一个键。"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeStatus {
    Available,
    Blocked,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceSource {
    RuntimeProbe,
    RuntimeProbeWithSystemRule,
    SystemError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerAttribution {
    NotApplicable,
    KnownSystem,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemRule {
    SecureAttention,
    LockWorkstation,
    DebuggerF12,
}

impl SystemRule {
    pub fn explanation(self) -> &'static str {
        match self {
            Self::SecureAttention => "Ctrl + Alt + Delete 由 Windows 安全桌面保留",
            Self::LockWorkstation => "Win + L 由 Windows 锁屏功能保留",
            Self::DebuggerF12 => "F12 始终为调试器保留",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeReport {
    pub status: ProbeStatus,
    pub source: EvidenceSource,
    pub owner: OwnerAttribution,
    pub code: Option<u32>,
    pub system_rule: Option<SystemRule>,
    pub detail: Option<&'static str>,
}

pub fn shortcut_from_parts(
    ctrl: bool,
    alt: bool,
    shift: bool,
    action_key: &str,
) -> Result<Shortcut, ActionKeyParseError> {
    Ok(shortcut_from_virtual_key(
        ctrl,
        alt,
        shift,
        parse_action_key(action_key)?,
    ))
}

pub fn shortcut_from_virtual_key(ctrl: bool, alt: bool, shift: bool, virtual_key: u32) -> Shortcut {
    let mut modifiers = 0;
    if ctrl {
        modifiers |= MOD_CONTROL.0;
    }
    if shift {
        modifiers |= MOD_SHIFT.0;
    }
    if alt {
        modifiers |= MOD_ALT.0;
    }

    Shortcut {
        modifiers,
        virtual_key,
    }
}

pub fn parse_action_key(input: &str) -> Result<u32, ActionKeyParseError> {
    let mut raw_characters = input.chars();
    if let (Some(character), None) = (raw_characters.next(), raw_characters.next()) {
        if matches!(character as u32, 0x10..=0x18) {
            return Err(ActionKeyParseError::ModifierInInput);
        }
        if let Some(key) = slint_key_code(character) {
            return Ok(key);
        }
    }

    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ActionKeyParseError::Empty);
    }

    let lowered = trimmed.to_lowercase();
    if (lowered.contains('+') && trimmed != "+")
        || matches!(
            lowered.as_str(),
            "ctrl" | "control" | "alt" | "shift" | "win" | "windows"
        )
    {
        return Err(ActionKeyParseError::ModifierInInput);
    }

    let mut characters = trimmed.chars();
    if let (Some(character), None) = (characters.next(), characters.next()) {
        if character.is_ascii_alphabetic() {
            return Ok(character.to_ascii_uppercase() as u32);
        }
        if character.is_ascii_digit() {
            return Ok(character as u32);
        }
        if let Some(key) = punctuation_key(character) {
            return Ok(key);
        }
    }

    let compact: String = lowered
        .chars()
        .filter(|character| !character.is_whitespace() && *character != '_' && *character != '-')
        .collect();

    if let Some(number) = compact
        .strip_prefix('f')
        .and_then(|value| value.parse::<u32>().ok())
        && (1..=24).contains(&number)
    {
        return Ok(VK_F1 + number - 1);
    }

    if let Some(number) = compact
        .strip_prefix("numpad")
        .and_then(|value| value.parse::<u32>().ok())
        && number <= 9
    {
        return Ok(VK_NUMPAD0 + number);
    }

    match compact.as_str() {
        "space" | "空格" => Ok(VK_SPACE),
        "enter" | "return" | "回车" => Ok(VK_RETURN),
        "tab" => Ok(VK_TAB),
        "backspace" | "back" | "退格" => Ok(VK_BACK),
        "esc" | "escape" => Ok(VK_ESCAPE),
        "delete" | "del" | "删除" => Ok(VK_DELETE),
        "insert" | "ins" => Ok(VK_INSERT),
        "home" => Ok(VK_HOME),
        "end" => Ok(VK_END),
        "pageup" | "pgup" => Ok(VK_PRIOR),
        "pagedown" | "pgdn" => Ok(VK_NEXT),
        "left" | "左" => Ok(VK_LEFT),
        "up" | "上" => Ok(VK_UP),
        "right" | "右" => Ok(VK_RIGHT),
        "down" | "下" => Ok(VK_DOWN),
        "printscreen" | "prtsc" => Ok(VK_SNAPSHOT),
        "pause" => Ok(VK_PAUSE),
        "capslock" => Ok(VK_CAPITAL),
        "numlock" => Ok(VK_NUMLOCK),
        "scrolllock" => Ok(VK_SCROLL),
        "numpadmultiply" => Ok(VK_MULTIPLY),
        "numpadadd" => Ok(VK_ADD),
        "numpadsubtract" => Ok(VK_SUBTRACT),
        "numpaddecimal" => Ok(VK_DECIMAL),
        "numpaddivide" => Ok(VK_DIVIDE),
        _ => Err(ActionKeyParseError::Unknown(trimmed.into())),
    }
}

fn slint_key_code(character: char) -> Option<u32> {
    Some(match character as u32 {
        0x0008 => VK_BACK,
        0x0009 => VK_TAB,
        0x000A => VK_RETURN,
        0x001B => VK_ESCAPE,
        0x0020 => VK_SPACE,
        0x007F => VK_DELETE,
        0xF700 => VK_UP,
        0xF701 => VK_DOWN,
        0xF702 => VK_LEFT,
        0xF703 => VK_RIGHT,
        code @ 0xF704..=0xF71B => VK_F1 + code - 0xF704,
        0xF727 => VK_INSERT,
        0xF729 => VK_HOME,
        0xF72B => VK_END,
        0xF72C => VK_PRIOR,
        0xF72D => VK_NEXT,
        0xF72F => VK_SCROLL,
        0xF730 => VK_PAUSE,
        0xF731 => VK_SNAPSHOT,
        _ => return None,
    })
}

fn punctuation_key(character: char) -> Option<u32> {
    Some(match character {
        ';' => VK_OEM_1,
        '=' | '+' => VK_OEM_PLUS,
        ',' => VK_OEM_COMMA,
        '-' => VK_OEM_MINUS,
        '.' => VK_OEM_PERIOD,
        '/' => VK_OEM_2,
        '`' => VK_OEM_3,
        '[' => VK_OEM_4,
        '\\' => VK_OEM_5,
        ']' => VK_OEM_6,
        '\'' => VK_OEM_7,
        _ => return None,
    })
}

pub fn probe_shortcut(shortcut: &Shortcut) -> ProbeReport {
    let modifiers = HOT_KEY_MODIFIERS(shortcut.modifiers | MOD_NOREPEAT.0);
    let registration = unsafe { RegisterHotKey(None, PROBE_ID, modifiers, shortcut.virtual_key) };

    match registration {
        Ok(()) => {
            unsafe {
                let _ = UnregisterHotKey(None, PROBE_ID);
            }
            ProbeReport {
                status: ProbeStatus::Available,
                source: EvidenceSource::RuntimeProbe,
                owner: OwnerAttribution::NotApplicable,
                code: None,
                system_rule: None,
                detail: None,
            }
        }
        Err(_) => {
            let code = unsafe { GetLastError().0 };
            if code == ERROR_HOTKEY_ALREADY_REGISTERED_CODE {
                let system_rule = match_system_rule(shortcut);
                ProbeReport {
                    status: ProbeStatus::Blocked,
                    source: if system_rule.is_some() {
                        EvidenceSource::RuntimeProbeWithSystemRule
                    } else {
                        EvidenceSource::RuntimeProbe
                    },
                    owner: if system_rule.is_some() {
                        OwnerAttribution::KnownSystem
                    } else {
                        OwnerAttribution::Unknown
                    },
                    code: Some(code),
                    system_rule,
                    detail: None,
                }
            } else {
                ProbeReport {
                    status: ProbeStatus::Error,
                    source: EvidenceSource::SystemError,
                    owner: OwnerAttribution::NotApplicable,
                    code: Some(code),
                    system_rule: None,
                    detail: None,
                }
            }
        }
    }
}

fn match_system_rule(shortcut: &Shortcut) -> Option<SystemRule> {
    let ctrl_alt = MOD_CONTROL.0 | MOD_ALT.0;
    if shortcut.virtual_key == VK_DELETE && shortcut.modifiers == ctrl_alt {
        return Some(SystemRule::SecureAttention);
    }
    if shortcut.virtual_key == VK_F1 + 11 {
        return Some(SystemRule::DebuggerF12);
    }
    if shortcut.virtual_key == 0x4C && shortcut.modifiers == MOD_WIN.0 {
        return Some(SystemRule::LockWorkstation);
    }
    None
}

pub fn format_shortcut(modifiers: u32, virtual_key: u32) -> String {
    let mut parts = Vec::with_capacity(5);
    if modifiers & MOD_CONTROL.0 != 0 {
        parts.push("Ctrl".to_string());
    }
    if modifiers & MOD_ALT.0 != 0 {
        parts.push("Alt".to_string());
    }
    if modifiers & MOD_SHIFT.0 != 0 {
        parts.push("Shift".to_string());
    }
    if modifiers & MOD_WIN.0 != 0 {
        parts.push("Win".to_string());
    }
    parts.push(key_name(virtual_key));
    parts.join(" + ")
}

fn key_name(virtual_key: u32) -> String {
    match virtual_key {
        0x30..=0x39 | 0x41..=0x5A => char::from_u32(virtual_key)
            .map(|value| value.to_string())
            .unwrap_or_else(|| format!("VK_{virtual_key:02X}")),
        VK_F1..=VK_F24 => format!("F{}", virtual_key - VK_F1 + 1),
        VK_NUMPAD0..=VK_NUMPAD9 => format!("Num {}", virtual_key - VK_NUMPAD0),
        VK_BACK => "Backspace".into(),
        VK_TAB => "Tab".into(),
        VK_RETURN => "Enter".into(),
        VK_PAUSE => "Pause".into(),
        VK_CAPITAL => "Caps Lock".into(),
        VK_ESCAPE => "Esc".into(),
        VK_SPACE => "Space".into(),
        VK_PRIOR => "Page Up".into(),
        VK_NEXT => "Page Down".into(),
        VK_END => "End".into(),
        VK_HOME => "Home".into(),
        VK_LEFT => "←".into(),
        VK_UP => "↑".into(),
        VK_RIGHT => "→".into(),
        VK_DOWN => "↓".into(),
        VK_SNAPSHOT => "Print Screen".into(),
        VK_INSERT => "Insert".into(),
        VK_DELETE => "Delete".into(),
        VK_MULTIPLY => "Num *".into(),
        VK_ADD => "Num +".into(),
        VK_SUBTRACT => "Num -".into(),
        VK_DECIMAL => "Num .".into(),
        VK_DIVIDE => "Num /".into(),
        VK_NUMLOCK => "Num Lock".into(),
        VK_SCROLL => "Scroll Lock".into(),
        VK_OEM_1 => ";".into(),
        VK_OEM_PLUS => "=".into(),
        VK_OEM_COMMA => ",".into(),
        VK_OEM_MINUS => "-".into(),
        VK_OEM_PERIOD => ".".into(),
        VK_OEM_2 => "/".into(),
        VK_OEM_3 => "`".into(),
        VK_OEM_4 => "[".into(),
        VK_OEM_5 => "\\".into(),
        VK_OEM_6 => "]".into(),
        VK_OEM_7 => "'".into(),
        _ => format!("VK_{virtual_key:02X}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;

    #[test]
    fn parses_letters_function_keys_and_aliases() {
        assert_eq!(parse_action_key("a"), Ok(0x41));
        assert_eq!(parse_action_key("F12"), Ok(VK_F1 + 11));
        assert_eq!(parse_action_key("Space"), Ok(VK_SPACE));
        assert_eq!(parse_action_key("回车"), Ok(VK_RETURN));
        assert_eq!(parse_action_key("Page Down"), Ok(VK_NEXT));
        assert_eq!(parse_action_key("Numpad 7"), Ok(VK_NUMPAD0 + 7));
        assert_eq!(parse_action_key("\u{F70F}"), Ok(VK_F1 + 11));
        assert_eq!(parse_action_key("PrtSc"), Ok(VK_SNAPSHOT));
        assert_eq!(parse_action_key("\u{F731}"), Ok(VK_SNAPSHOT));
        assert_eq!(parse_action_key("\n"), Ok(VK_RETURN));
        assert_eq!(parse_action_key(" "), Ok(VK_SPACE));
        assert_eq!(parse_action_key("+"), Ok(VK_OEM_PLUS));
    }

    #[test]
    fn rejects_empty_modifier_and_full_combo_input() {
        assert_eq!(parse_action_key(""), Err(ActionKeyParseError::Empty));
        assert_eq!(
            parse_action_key("Ctrl"),
            Err(ActionKeyParseError::ModifierInInput)
        );
        assert_eq!(
            parse_action_key("\u{0011}"),
            Err(ActionKeyParseError::ModifierInInput)
        );
        assert_eq!(
            parse_action_key("Ctrl+Alt+A"),
            Err(ActionKeyParseError::ModifierInInput)
        );
        assert!(matches!(
            parse_action_key("not-a-key"),
            Err(ActionKeyParseError::Unknown(_))
        ));
    }

    #[test]
    fn builds_a_shortcut_from_virtual_modifier_tags() {
        let shortcut = shortcut_from_parts(true, true, false, "a").expect("build shortcut");
        assert_eq!(shortcut.label(), "Ctrl + Alt + A");

        let all = shortcut_from_parts(true, true, true, "F2").expect("build all modifiers");
        assert_eq!(all.label(), "Ctrl + Alt + Shift + F2");
    }

    #[test]
    fn identifies_known_reserved_shortcuts() {
        let secure_attention = Shortcut {
            modifiers: MOD_CONTROL.0 | MOD_ALT.0,
            virtual_key: VK_DELETE,
        };
        assert_eq!(
            match_system_rule(&secure_attention),
            Some(SystemRule::SecureAttention)
        );

        let lock_workstation = Shortcut {
            modifiers: MOD_WIN.0,
            virtual_key: 0x4C,
        };
        assert_eq!(
            match_system_rule(&lock_workstation),
            Some(SystemRule::LockWorkstation)
        );
    }

    #[test]
    fn detects_a_real_register_hot_key_conflict() {
        let (chosen_sender, chosen_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();

        let owner = thread::spawn(move || {
            let modifiers = HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_SHIFT.0 | MOD_ALT.0);
            for virtual_key in (VK_F1 + 12)..=VK_F24 {
                if unsafe { RegisterHotKey(None, PROBE_ID + 1, modifiers, virtual_key) }.is_ok() {
                    chosen_sender
                        .send(Some(virtual_key))
                        .expect("send registered test key");
                    let _ = release_receiver.recv();
                    unsafe {
                        let _ = UnregisterHotKey(None, PROBE_ID + 1);
                    }
                    return;
                }
            }
            chosen_sender
                .send(None)
                .expect("report unavailable test key range");
        });

        let virtual_key = chosen_receiver
            .recv()
            .expect("receive registered test key")
            .expect("at least one F13-F24 test shortcut should be free");
        let shortcut = Shortcut {
            modifiers: MOD_CONTROL.0 | MOD_SHIFT.0 | MOD_ALT.0,
            virtual_key,
        };

        assert_eq!(probe_shortcut(&shortcut).status, ProbeStatus::Blocked);
        release_sender.send(()).expect("release test hotkey owner");
        owner.join().expect("join test hotkey owner");
        assert_eq!(probe_shortcut(&shortcut).status, ProbeStatus::Available);
    }
}
