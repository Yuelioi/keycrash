use crate::mac_keys;
pub use crate::report::*;

#[path = "carbon.rs"]
pub mod carbon;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub modifiers: u32,
    pub key_code: u16,
    key_label: String,
}

impl Shortcut {
    pub fn from_native(
        ctrl: bool,
        option: bool,
        shift: bool,
        command: bool,
        key_code: u16,
        characters: &str,
    ) -> Self {
        Self {
            modifiers: mac_keys::modifiers(ctrl, option, shift, command),
            key_code,
            key_label: mac_keys::key_label(key_code, characters),
        }
    }

    pub fn label(&self) -> String {
        mac_keys::shortcut_label(self.modifiers, &self.key_label)
    }
}

pub fn probe_shortcut(shortcut: &Shortcut) -> ProbeReport {
    match carbon::system_shortcut_enabled(shortcut.key_code, shortcut.modifiers) {
        Ok(true) => {
            return ProbeReport {
                status: ProbeStatus::Blocked,
                source: EvidenceSource::SystemRule,
                owner: OwnerAttribution::KnownSystem,
                code: None,
                system_rule: Some(SystemRule::MacSymbolicHotKey),
                detail: None,
            };
        }
        Err(code) => return error_report(code, Some("无法读取 macOS 系统快捷键，请重试。")),
        Ok(false) => {}
    }
    match carbon::Registration::new(shortcut.key_code, shortcut.modifiers) {
        Ok(registration) => match registration.release() {
            Ok(()) => ProbeReport {
                status: ProbeStatus::Available,
                source: EvidenceSource::RuntimeProbe,
                owner: OwnerAttribution::NotApplicable,
                code: None,
                system_rule: None,
                detail: None,
            },
            Err(code) => error_report(code, Some("临时热键释放失败，请退出 KeyCrash 后重试。")),
        },
        Err(carbon::HOT_KEY_EXISTS) => ProbeReport {
            status: ProbeStatus::Blocked,
            source: EvidenceSource::RuntimeProbe,
            owner: OwnerAttribution::Unknown,
            code: Some(i64::from(carbon::HOT_KEY_EXISTS)),
            system_rule: None,
            detail: None,
        },
        Err(code) => error_report(code, Some("macOS 无法注册此组合；请换用 ⌘ 或 ⌃ 后重试。")),
    }
}

fn error_report(code: i32, detail: Option<&'static str>) -> ProbeReport {
    ProbeReport {
        status: ProbeStatus::Error,
        source: EvidenceSource::SystemError,
        owner: OwnerAttribution::NotApplicable,
        code: Some(i64::from(code)),
        system_rule: None,
        detail,
    }
}
