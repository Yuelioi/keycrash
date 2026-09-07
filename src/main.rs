#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod detector;
mod i18n;
mod key_capture;
mod owner_probe;

use detector::{
    EvidenceSource, OwnerAttribution, ProbeReport, ProbeStatus, Shortcut, shortcut_from_parts,
    shortcut_from_virtual_key,
};
use owner_probe::OwnerProbeResult;
use slint::{Color, ComponentHandle, SharedString};
use std::cell::RefCell;
use std::ffi::OsString;
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::thread;
use std::time::Duration;

slint::include_modules!();

const MODE_IDLE: &str = "idle";
const MODE_WAITING_KEY: &str = "waiting-key";
const MODE_AVAILABLE: &str = "available";
const MODE_OCCUPIED: &str = "occupied";
const MODE_RESERVED: &str = "reserved";
const MODE_LOCATING: &str = "locating";
const MODE_OWNER_FOUND: &str = "owner-found";
const MODE_UNSUPPORTED: &str = "unsupported";
const MODE_ERROR: &str = "error";

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    ui.set_app_version(env!("CARGO_PKG_VERSION").into());
    let last_blocked_shortcut = Rc::new(RefCell::new(None));
    set_idle(&ui);

    {
        let weak_ui = ui.as_weak();
        ui.on_language_changed(move |code| {
            let Some(ui) = weak_ui.upgrade() else {
                return;
            };
            let language = i18n::Language::from_code(code.as_str());
            ui.set_language(code);
            relocalize_view(&ui, language);
        });
    }

    {
        let weak_ui = ui.as_weak();
        let last_blocked_shortcut = last_blocked_shortcut.clone();
        ui.on_begin_detection(move |ctrl, alt, shift| {
            if let Some(ui) = weak_ui.upgrade() {
                *last_blocked_shortcut.borrow_mut() = None;
                set_waiting_for_key(&ui, ctrl, alt, shift);
            }
        });
    }

    {
        let weak_ui = ui.as_weak();
        let last_blocked_shortcut = last_blocked_shortcut.clone();
        ui.on_action_key_entered(move |action_key, ctrl, alt, shift| {
            let Some(ui) = weak_ui.upgrade() else {
                return;
            };

            let shortcut = match shortcut_from_parts(ctrl, alt, shift, action_key.as_str()) {
                Ok(shortcut) => shortcut,
                Err(error) => {
                    set_view(
                        &ui,
                        MODE_UNSUPPORTED,
                        "没有识别到目标键",
                        &error.message(),
                        "请再试一次",
                        "本地输入 · 未发送完整组合",
                        "重新选择",
                        color("8B6B16"),
                    );
                    return;
                }
            };
            detect_shortcut(&ui, &last_blocked_shortcut, shortcut);
        });
    }

    {
        let weak_ui = ui.as_weak();
        let last_blocked_shortcut = last_blocked_shortcut.clone();
        ui.on_locate_owner(move || {
            let Some(ui) = weak_ui.upgrade() else {
                return;
            };
            let Some(shortcut) = last_blocked_shortcut.borrow().clone() else {
                set_owner_error(&ui, "没有可用于定位的软件冲突，请重新检测。");
                return;
            };

            let shortcut_label = shortcut.label();
            set_locating_owner(&ui, &shortcut_label);
            let result_ui = ui.as_weak();
            thread::spawn(move || {
                let report = owner_probe::locate_owner(&shortcut);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = result_ui.upgrade() {
                        apply_owner_report(&ui, &shortcut_label, report);
                    }
                });
            });
        });
    }

    {
        let weak_ui = ui.as_weak();
        ui.on_cancel_requested(move || {
            if let Some(ui) = weak_ui.upgrade() {
                set_idle(&ui);
            }
        });
    }

    {
        let weak_ui = ui.as_weak();
        ui.on_modifiers_changed(move || {
            if let Some(ui) = weak_ui.upgrade() {
                set_idle(&ui);
            }
        });
    }

    {
        let weak_ui = ui.as_weak();
        ui.on_open_owner_location(move || {
            let Some(ui) = weak_ui.upgrade() else {
                return;
            };
            let owner_path = ui.get_owner_path();
            if owner_path.is_empty() {
                return;
            }
            if open_file_location(Path::new(owner_path.as_str())).is_err() {
                let language = i18n::Language::from_code(ui.get_language().as_str());
                ui.set_evidence_text(SharedString::from(i18n::text(
                    language,
                    "无法打开文件位置 · 请按上方路径手动打开",
                )));
            }
        });
    }

    let _action_key_fallback = install_action_key_fallback(&ui, last_blocked_shortcut);

    ui.run()
}

fn install_action_key_fallback(
    ui: &AppWindow,
    last_blocked_shortcut: Rc<RefCell<Option<Shortcut>>>,
) -> slint::Timer {
    let timer = slint::Timer::default();
    let weak_ui = ui.as_weak();
    let virtual_keys = key_capture::supported_virtual_keys();
    let mut capture = key_capture::ActionKeyCapture::default();

    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(8),
        move || {
            let Some(ui) = weak_ui.upgrade() else {
                return;
            };

            let armed =
                ui.get_mode() == MODE_WAITING_KEY && key_capture::application_is_foreground();
            if !armed {
                capture.observe(false, false, std::iter::empty());
                return;
            }

            let forbidden_modifier_down = key_capture::physical_modifier_is_down();
            let key_states = virtual_keys
                .iter()
                .copied()
                .map(|virtual_key| (virtual_key, key_capture::key_is_down(virtual_key)));
            if let Some(virtual_key) = capture.observe(true, forbidden_modifier_down, key_states) {
                if virtual_key == key_capture::VK_ESCAPE_CODE {
                    ui.invoke_cancel_requested();
                    return;
                }

                let shortcut = shortcut_from_virtual_key(
                    ui.get_ctrl_selected(),
                    ui.get_alt_selected(),
                    ui.get_shift_selected(),
                    virtual_key,
                );
                detect_shortcut(&ui, &last_blocked_shortcut, shortcut);
            }
        },
    );

    timer
}

fn detect_shortcut(
    ui: &AppWindow,
    last_blocked_shortcut: &RefCell<Option<Shortcut>>,
    shortcut: Shortcut,
) {
    let report = detector::probe_shortcut(&shortcut);
    if report.status == ProbeStatus::Blocked && report.system_rule.is_none() {
        *last_blocked_shortcut.borrow_mut() = Some(shortcut.clone());
    } else {
        *last_blocked_shortcut.borrow_mut() = None;
    }
    apply_report(ui, &shortcut.label(), report);
}

fn apply_report(ui: &AppWindow, shortcut_label: &str, report: ProbeReport) {
    match report.status {
        ProbeStatus::Available => set_view(
            ui,
            MODE_AVAILABLE,
            "未发现注册冲突",
            "其他软件仍可能监听此键；本次临时注册已释放。",
            shortcut_label,
            &evidence_label(&report),
            "再测一次",
            color("007A7E"),
        ),
        ProbeStatus::Blocked => {
            let mode = if report.system_rule.is_some() {
                MODE_RESERVED
            } else {
                MODE_OCCUPIED
            };
            set_view(
                ui,
                mode,
                if report.system_rule.is_some() {
                    "系统保留快捷键"
                } else {
                    "已发现注册冲突"
                },
                blocked_detail(&report),
                shortcut_label,
                &evidence_label(&report),
                if report.system_rule.is_some() {
                    "再测一次"
                } else {
                    "定位占用软件"
                },
                if report.system_rule.is_some() {
                    color("8B6B16")
                } else {
                    color("C9342A")
                },
            );
        }
        ProbeStatus::Error => {
            let code = report.code.unwrap_or_default();
            set_view(
                ui,
                MODE_ERROR,
                "检测没有完成",
                &format!("Windows 返回了系统错误 {code}，请再试一次。"),
                shortcut_label,
                &evidence_label(&report),
                "重新检测",
                color("A33B32"),
            );
        }
    }
}

fn blocked_detail(report: &ProbeReport) -> &'static str {
    report.system_rule.map_or(
        "冲突已确认；定位会真实触发组合，原动作可能执行。",
        |rule| rule.explanation(),
    )
}

fn set_locating_owner(ui: &AppWindow, shortcut_label: &str) {
    set_view(
        ui,
        MODE_LOCATING,
        "正在定位占用软件",
        "先检查普通软件；未命中时再请求管理员权限。",
        shortcut_label,
        "普通 / 管理员 × x64 / x86",
        "定位中…",
        color("C9342A"),
    );
}

fn apply_owner_report(ui: &AppWindow, shortcut_label: &str, report: OwnerProbeResult) {
    match report {
        OwnerProbeResult::Found {
            pid,
            thread_id,
            path,
            suppressed: _,
            architecture,
        } => {
            let process_name = owner_display_name(&path);
            let title = format!("{process_name}占用了它");
            let detail = if path.is_empty() {
                format!("已在进程 {pid} 的消息队列中观察到匹配事件。")
            } else {
                path.clone()
            };
            set_view(
                ui,
                MODE_OWNER_FOUND,
                &title,
                &detail,
                shortcut_label,
                &owner_evidence(&architecture, pid, thread_id),
                "再测一次",
                color("007A7E"),
            );
            if !path.is_empty() {
                ui.set_owner_path(SharedString::from(path));
            }
        }
        OwnerProbeResult::NotFound => set_view(
            ui,
            MODE_ERROR,
            "已占用，但没有观察到软件",
            "可能来自低级 Hook、Raw Input、驱动，或目标忽略了模拟输入。",
            shortcut_label,
            "x64 + x86 深度定位 · 未观察到 WM_HOTKEY",
            "再试一次",
            color("8B6B16"),
        ),
        OwnerProbeResult::Error(error) => set_owner_error(ui, &error),
    }
}

fn set_owner_error(ui: &AppWindow, error: &str) {
    let detail = if i18n::Language::from_code(ui.get_language().as_str()) == i18n::Language::Zh {
        error
    } else {
        "深度定位组件返回错误，请再试一次。"
    };
    set_view(
        ui,
        MODE_ERROR,
        "定位没有完成",
        detail,
        "占用软件未知",
        "深度定位组件错误",
        "重新检测",
        color("A33B32"),
    );
}

fn evidence_label(report: &ProbeReport) -> String {
    match (report.source, report.owner) {
        (EvidenceSource::RuntimeProbe, OwnerAttribution::NotApplicable) => {
            "注册检测 · 不涵盖所有键盘监听".into()
        }
        (EvidenceSource::RuntimeProbe, OwnerAttribution::Unknown) => {
            let code = report.code.unwrap_or_default();
            format!("运行时探测 · 错误 {code} · owner 未知")
        }
        (EvidenceSource::SystemRule, OwnerAttribution::KnownSystem) => {
            "Windows 系统规则 · 未尝试注册".into()
        }
        (EvidenceSource::SystemError, _) => {
            let code = report.code.unwrap_or_default();
            format!("Windows 系统错误 · {code}")
        }
        _ => "检测证据不可用".into(),
    }
}

fn set_idle(ui: &AppWindow) {
    set_view(
        ui,
        MODE_IDLE,
        "选择修饰键",
        "单击 Ctrl、Alt、Shift；开始后只按一个目标键。",
        "等待目标键",
        "安全输入 · 不发送完整组合",
        "开始检测",
        color("C9342A"),
    );
}

fn set_waiting_for_key(ui: &AppWindow, ctrl: bool, alt: bool, shift: bool) {
    set_view(
        ui,
        MODE_WAITING_KEY,
        "请按一个目标键",
        "只按 A、F9、Space 等目标键；不要再按修饰键。",
        &modifier_preview(ctrl, alt, shift),
        "",
        "取消",
        color("C9342A"),
    );
}

fn modifier_preview(ctrl: bool, alt: bool, shift: bool) -> String {
    let mut parts = Vec::new();
    if ctrl {
        parts.push("Ctrl");
    }
    if alt {
        parts.push("Alt");
    }
    if shift {
        parts.push("Shift");
    }
    if parts.is_empty() {
        "按目标键…".into()
    } else {
        format!("{} + …", parts.join(" + "))
    }
}

#[allow(clippy::too_many_arguments)]
fn set_view(
    ui: &AppWindow,
    mode: &str,
    title: &str,
    detail: &str,
    shortcut: &str,
    evidence: &str,
    action: &str,
    accent: Color,
) {
    let language = i18n::Language::from_code(ui.get_language().as_str());
    ui.set_mode(SharedString::from(mode));
    ui.set_state_title(SharedString::from(i18n::text(language, title)));
    ui.set_state_detail(SharedString::from(i18n::text(language, detail)));
    ui.set_shortcut_text(SharedString::from(i18n::text(language, shortcut)));
    ui.set_evidence_text(SharedString::from(i18n::text(language, evidence)));
    ui.set_action_text(SharedString::from(i18n::text(language, action)));
    ui.set_action_enabled(mode != MODE_LOCATING);
    ui.set_accent(accent);
    ui.set_owner_path(SharedString::new());
}

fn relocalize_view(ui: &AppWindow, language: i18n::Language) {
    let owner_error =
        i18n::text(i18n::Language::Zh, ui.get_state_title().as_str()) == "定位没有完成";
    ui.set_state_title(SharedString::from(i18n::text(
        language,
        ui.get_state_title().as_str(),
    )));
    let current_detail = ui.get_state_detail();
    let detail = if owner_error {
        "深度定位组件返回错误，请再试一次。"
    } else {
        current_detail.as_str()
    };
    ui.set_state_detail(SharedString::from(i18n::text(language, detail)));
    ui.set_shortcut_text(SharedString::from(i18n::text(
        language,
        ui.get_shortcut_text().as_str(),
    )));
    ui.set_evidence_text(SharedString::from(i18n::text(
        language,
        ui.get_evidence_text().as_str(),
    )));
    ui.set_action_text(SharedString::from(i18n::text(
        language,
        ui.get_action_text().as_str(),
    )));
}

fn owner_display_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("未知进程")
        .into()
}

fn owner_evidence(architecture: &str, pid: u32, thread_id: u32) -> String {
    format!("WM_HOTKEY · {architecture} · PID {pid} / TID {thread_id}")
}

fn explorer_select_argument(path: &Path) -> OsString {
    let mut argument = OsString::from("/select,");
    argument.push(path.as_os_str());
    argument
}

fn open_file_location(path: &Path) -> Result<(), String> {
    Command::new("explorer.exe")
        .arg(explorer_select_argument(path))
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn color(hex: &str) -> Color {
    let value = u32::from_str_radix(hex, 16).expect("valid color literal");
    Color::from_rgb_u8(
        ((value >> 16) & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        (value & 0xFF) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detector::SystemRule;

    #[test]
    fn modifier_preview_uses_the_same_order_as_shortcut_labels() {
        assert_eq!(modifier_preview(true, true, true), "Ctrl + Alt + Shift + …");
        assert_eq!(modifier_preview(false, false, false), "按目标键…");
    }

    #[test]
    fn evidence_label_keeps_unknown_owner_explicit() {
        let report = ProbeReport {
            status: ProbeStatus::Blocked,
            source: EvidenceSource::RuntimeProbe,
            owner: OwnerAttribution::Unknown,
            code: Some(1409),
            system_rule: None,
            detail: None,
        };
        assert_eq!(
            evidence_label(&report),
            "运行时探测 · 错误 1409 · owner 未知"
        );
    }

    #[test]
    fn evidence_label_distinguishes_system_rules() {
        let report = ProbeReport {
            status: ProbeStatus::Blocked,
            source: EvidenceSource::SystemRule,
            owner: OwnerAttribution::KnownSystem,
            code: None,
            system_rule: Some(SystemRule::LockWorkstation),
            detail: None,
        };
        assert_eq!(evidence_label(&report), "Windows 系统规则 · 未尝试注册");
    }

    #[test]
    fn blocked_detail_does_not_imply_that_elevation_reveals_the_owner() {
        let report = ProbeReport {
            status: ProbeStatus::Blocked,
            source: EvidenceSource::RuntimeProbe,
            owner: OwnerAttribution::Unknown,
            code: Some(1409),
            system_rule: None,
            detail: None,
        };

        assert_eq!(
            blocked_detail(&report),
            "冲突已确认；定位会真实触发组合，原动作可能执行。"
        );
    }

    #[test]
    fn owner_title_omits_only_the_executable_suffix() {
        assert_eq!(owner_display_name("C:\\Apps\\Snipaste.exe"), "Snipaste");
        assert_eq!(owner_display_name("C:\\Apps\\tool.beta.exe"), "tool.beta");
    }

    #[test]
    fn owner_evidence_does_not_claim_that_the_action_was_blocked() {
        assert_eq!(
            owner_evidence("x64", 42, 7),
            "WM_HOTKEY · x64 · PID 42 / TID 7"
        );
    }

    #[test]
    fn explorer_argument_selects_the_owner_executable() {
        assert_eq!(
            explorer_select_argument(Path::new("C:\\Apps\\Snipaste.exe")),
            OsString::from("/select,C:\\Apps\\Snipaste.exe")
        );
    }
}
