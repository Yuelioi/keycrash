//! Bounded private contract for an explicitly selected keyboard sequence.
#![allow(dead_code)] // GUI and helper use different halves of the wire contract.
use std::{
    collections::BTreeSet,
    io::{Read, Write},
};

pub const MAX_STEPS: usize = 32;
pub const MAX_MS: u32 = 5_000;
pub const GAP_MS: u32 = 30;
pub const PLAN_MAGIC: [u8; 4] = *b"KSP1";
pub const RESULT_MAGIC: [u8; 4] = *b"KST1";
pub const READY: u32 = 1;
pub const SENT: u32 = 2;
pub const END: u32 = 3;
pub const STEP_DONE: u32 = 5;
pub const DOWN: u32 = 1;
pub const CLEANUP: u32 = 2;
pub const COMPLETE: u32 = 0;
pub const CANCELLED: u32 = 1;
pub const SEND_FAILED: u32 = 2;
pub const KEYS_HELD: u32 = 3;
pub const INVALID_PLAN: u32 = 4;
pub const PERMISSION_REQUIRED: u32 = 5;
pub const RELEASE_UNCONFIRMED: u32 = 6;

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Tap = 0,
    Down = 1,
    Up = 2,
    Wait = 3,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub action: Action,
    pub key: u32,
    pub delay_ms: u32,
}

pub const NUM_ENTER: u32 = 0x1000d;
#[cfg(target_os = "macos")]
#[path = "../src/macos/sequence_keys.rs"]
mod mac_keys;
#[cfg(target_os = "macos")]
pub use mac_keys::{event_modifier, keys, letter_key, modifier, reserved};
#[cfg(windows)]
pub fn letter_key(letter: u8) -> u32 {
    u32::from(letter)
}
#[cfg(windows)]
pub fn keys() -> Vec<(u32, String)> {
    let mut keys: Vec<_> = [
        (0xa2, "Ctrl"),
        (0xa4, "Alt"),
        (0xa0, "Shift"),
        (0x5b, "Win"),
    ]
    .into_iter()
    .map(|(key, label)| (key, label.into()))
    .collect();
    keys.extend((b'A'..=b'Z').map(|key| (u32::from(key), char::from(key).to_string())));
    keys.extend((b'0'..=b'9').map(|key| (u32::from(key), char::from(key).to_string())));
    keys.extend((1..=24).map(|n| (0x6f + n, format!("F{n}"))));
    keys.extend((0..=9).map(|n| (0x60 + n, format!("Num {n}"))));
    keys.extend(
        [
            (13, "Enter"),
            (27, "Esc"),
            (32, "Space"),
            (9, "Tab"),
            (8, "Backspace"),
            (46, "Delete"),
            (45, "Insert"),
            (36, "Home"),
            (35, "End"),
            (33, "Page Up"),
            (34, "Page Down"),
            (37, "Left"),
            (38, "Up"),
            (39, "Right"),
            (40, "Down"),
            (44, "Print Screen"),
            (0xa3, "RCtrl"),
            (0xa5, "RAlt"),
            (0xa1, "RShift"),
            (0x5c, "RWin"),
            (NUM_ENTER, "Num Enter"),
            (0x6a, "Num *"),
            (0x6b, "Num +"),
            (0x6c, "Num Separator"),
            (0x6d, "Num -"),
            (0x6e, "Num ."),
            (0x6f, "Num /"),
            (0xba, "; :"),
            (0xbb, "= +"),
            (0xbc, ", <"),
            (0xbd, "- _"),
            (0xbe, ". >"),
            (0xbf, "/ ?"),
            (0xc0, "` ~"),
            (0xdb, "[ {"),
            (0xdc, "\\ |"),
            (0xdd, "] }"),
            (0xde, "' \""),
            (0xe2, "OEM 102"),
            (0x14, "Caps Lock"),
            (0x90, "Num Lock"),
            (0x91, "Scroll Lock"),
            (0x13, "Pause"),
            (3, "Break"),
            (0x0c, "Clear"),
            (0x2f, "Help"),
            (0x5d, "Menu"),
            (0xad, "Volume Mute"),
            (0xae, "Volume Down"),
            (0xaf, "Volume Up"),
            (0xb0, "Media Next"),
            (0xb1, "Media Previous"),
            (0xb2, "Media Stop"),
            (0xb3, "Media Play/Pause"),
            (0xa6, "Browser Back"),
            (0xa7, "Browser Forward"),
            (0xa8, "Browser Refresh"),
            (0xa9, "Browser Stop"),
            (0xaa, "Browser Search"),
            (0xab, "Browser Favorites"),
            (0xac, "Browser Home"),
            (0xb4, "Launch Mail"),
            (0xb5, "Launch Media"),
            (0xb6, "Launch App 1"),
            (0xb7, "Launch App 2"),
            (0x15, "IME Kana/Hangul"),
            (0x19, "IME Hanja/Kanji"),
            (0x16, "IME On"),
            (0x1a, "IME Off"),
            (0x1c, "IME Convert"),
            (0x1d, "IME Nonconvert"),
            (0x1f, "IME Mode Change"),
        ]
        .into_iter()
        .map(|(key, label)| (key, label.into())),
    );
    keys
}
pub fn key_label(code: u32) -> String {
    keys()
        .into_iter()
        .find(|(key, _)| *key == code)
        .map(|(_, label)| label)
        .unwrap_or_else(|| format!("VK {code}"))
}
#[cfg(windows)]
pub fn modifier(key: u32) -> u32 {
    match key {
        0xa2 | 0xa3 => 2,
        0xa4 | 0xa5 => 1,
        0xa0 | 0xa1 => 4,
        0x5b | 0x5c => 8,
        _ => 0,
    }
}
pub fn extended(key: u32) -> bool {
    matches!(key,NUM_ENTER|3|0xa3|0xa5|0x5b..=0x5d|33..=40|44..=46|0x6f|0x90|0xa6..=0xb7)
}
pub fn virtual_key(key: u32) -> u32 {
    if key == NUM_ENTER { 13 } else { key }
}
#[cfg(windows)]
pub fn reserved(key: u32, modifiers: u32) -> bool {
    key == 0x7b || (key == 0x4c && modifiers == 8) || (key == 46 && modifiers == 3)
}

pub fn validate(steps: &[Step]) -> Result<(), &'static str> {
    if steps.is_empty() {
        return Err("请先添加按键步骤。");
    }
    if steps.len() > MAX_STEPS {
        return Err("最多编排 32 个步骤。");
    }
    let allowed = keys();
    let mut held = BTreeSet::new();
    let mut duration = 0;
    let mut has_key = false;
    for step in steps {
        if step.action == Action::Wait {
            if step.key != 0 || !(20..=2000).contains(&step.delay_ms) {
                return Err("等待时间应为 20 至 2000 毫秒。");
            }
            duration += step.delay_ms;
        } else {
            has_key = true;
            if step.delay_ms != 0 || !allowed.iter().any(|(key, _)| *key == step.key) {
                return Err("请选择有效按键。");
            }
            match step.action {
                Action::Tap | Action::Down => {
                    if held.contains(&step.key) {
                        return Err("该按键已按住，请先添加松开步骤。");
                    }
                    let mods = held.iter().fold(0, |bits, key| bits | modifier(*key));
                    if reserved(step.key, mods) {
                        return Err("系统保留组合不能自动测试。");
                    }
                    if step.action == Action::Down {
                        held.insert(step.key);
                        duration += GAP_MS;
                    } else {
                        duration += GAP_MS * 2;
                    }
                }
                Action::Up => {
                    if !held.remove(&step.key) {
                        return Err("松开前需要先按下该键。");
                    }
                    duration += GAP_MS;
                }
                Action::Wait => unreachable!(),
            }
        }
        if duration > MAX_MS {
            return Err("序列总时长不能超过 5 秒。");
        }
    }
    if !has_key {
        return Err("至少添加一个按键步骤。");
    }
    if !held.is_empty() {
        return Err("请补上松开步骤，避免按键保持按下。");
    }
    Ok(())
}
pub fn write_plan(steps: &[Step], output: &mut impl Write) -> Result<(), String> {
    validate(steps).map_err(str::to_owned)?;
    output.write_all(&PLAN_MAGIC).map_err(|e| e.to_string())?;
    output
        .write_all(&(steps.len() as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;
    for step in steps {
        for value in [step.action as u32, step.key, step.delay_ms] {
            output
                .write_all(&value.to_le_bytes())
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
pub fn read_plan(input: &mut impl Read) -> Result<Vec<Step>, String> {
    fn word(input: &mut impl Read) -> Result<u32, String> {
        let mut bytes = [0; 4];
        input.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        Ok(u32::from_le_bytes(bytes))
    }
    let mut magic = [0; 4];
    input.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if magic != PLAN_MAGIC {
        return Err("Invalid sequence plan".into());
    }
    let count = word(input)? as usize;
    if count == 0 || count > MAX_STEPS {
        return Err("Invalid sequence length".into());
    }
    let mut steps = Vec::with_capacity(count);
    for _ in 0..count {
        let action = match word(input)? {
            0 => Action::Tap,
            1 => Action::Down,
            2 => Action::Up,
            3 => Action::Wait,
            _ => return Err("Invalid sequence action".into()),
        };
        steps.push(Step {
            action,
            key: word(input)?,
            delay_ms: word(input)?,
        });
    }
    validate(&steps).map_err(str::to_owned)?;
    Ok(steps)
}

#[derive(Clone, Copy, Default)]
pub struct Record {
    pub kind: u32,
    pub key: u32,
    pub flags: u32,
    pub tick: u32,
    pub observed: u32,
}
impl Record {
    pub fn write(self, output: &mut impl Write) -> std::io::Result<()> {
        for value in [self.kind, self.key, self.flags, self.tick, self.observed] {
            output.write_all(&value.to_le_bytes())?;
        }
        output.flush()
    }
    pub fn read(input: &mut impl Read) -> std::io::Result<Self> {
        let mut bytes = [0; 20];
        input.read_exact(&mut bytes)?;
        let mut values = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .copied()
            .map(u32::from_le_bytes);
        Ok(Self {
            kind: values.next().unwrap(),
            key: values.next().unwrap(),
            flags: values.next().unwrap(),
            tick: values.next().unwrap(),
            observed: values.next().unwrap(),
        })
    }
}
