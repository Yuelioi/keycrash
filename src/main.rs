#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod detector;
mod owner_probe;

use detector::{EvidenceSource, OwnerAttribution, ProbeReport, ProbeStatus, shortcut_from_parts};
use owner_probe::OwnerProbeResult;
use slint::{Color, ComponentHandle, SharedString};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::thread;

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
    let last_blocked_shortcut = Rc::new(RefCell::new(None));
    set_idle(&ui);

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

            let report = detector::probe_shortcut(&shortcut);
            if report.status == ProbeStatus::Blocked && report.system_rule.is_none() {
                *last_blocked_shortcut.borrow_mut() = Some(shortcut.clone());
            } else {
                *last_blocked_shortcut.borrow_mut() = None;
            }
            apply_report(&ui, &shortcut.label(), report);
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

    ui.run()
}

fn apply_report(ui: &AppWindow, shortcut_label: &str, report: ProbeReport) {
    match report.status {
        ProbeStatus::Available => set_view(
            ui,
            MODE_AVAILABLE,
            "这个组合当前可用",
            "Windows 接受了临时注册，KeyCrash 已立即释放它。",
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
                "已被占用或系统保留",
                blocked_detail(&report),
                shortcut_label,
                &evidence_label(&report),
                if report.system_rule.is_some() {
                    "再测一次"
                } else {
                    "定位占用软件"
                },
                color("C9342A"),
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
        "冲突已确认；定位软件最多触发两次组合，并尽量拦截原动作。",
        |rule| rule.explanation(),
    )
}

fn set_locating_owner(ui: &AppWindow, shortcut_label: &str) {
    set_view(
        ui,
        MODE_LOCATING,
        "正在定位占用软件",
        "观察目标进程的 WM_HOTKEY；通常在一秒内完成。",
        shortcut_label,
        "x64 + x86 进程内消息 Hook · 原动作尽量拦截",
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
            suppressed,
            architecture,
        } => {
            let process_name = Path::new(&path)
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .unwrap_or("未知进程");
            let title = format!("{process_name} 占用了它");
            let detail = if path.is_empty() {
                format!("已在进程 {pid} 的消息队列中观察到匹配事件。")
            } else {
                path
            };
            let suppression = if suppressed {
                "原动作已拦截"
            } else {
                "原动作可能已执行"
            };
            set_view(
                ui,
                MODE_OWNER_FOUND,
                &title,
                &detail,
                shortcut_label,
                &format!(
                    "WM_HOTKEY · {architecture} · PID {pid} / TID {thread_id} · {suppression}"
                ),
                "再测一次",
                color("007A7E"),
            );
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
    set_view(
        ui,
        MODE_ERROR,
        "定位没有完成",
        error,
        "占用软件未知",
        "深度定位组件错误",
        "重新检测",
        color("A33B32"),
    );
}

fn evidence_label(report: &ProbeReport) -> String {
    match (report.source, report.owner) {
        (EvidenceSource::RuntimeProbe, OwnerAttribution::NotApplicable) => {
            "运行时探测 · 可直接注册".into()
        }
        (EvidenceSource::RuntimeProbe, OwnerAttribution::Unknown) => {
            let code = report.code.unwrap_or_default();
            format!("运行时探测 · 错误 {code} · owner 未知")
        }
        (EvidenceSource::RuntimeProbeWithSystemRule, OwnerAttribution::KnownSystem) => {
            "系统规则 + 运行时探测".into()
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
        "只按 A、F12、Space 等目标键；不要再按修饰键。",
        &modifier_preview(ctrl, alt, shift),
        "只捕获 KeyCrash 当前窗口",
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
    ui.set_mode(SharedString::from(mode));
    ui.set_state_title(SharedString::from(title));
    ui.set_state_detail(SharedString::from(detail));
    ui.set_shortcut_text(SharedString::from(shortcut));
    ui.set_evidence_text(SharedString::from(evidence));
    ui.set_action_text(SharedString::from(action));
    ui.set_action_enabled(mode != MODE_LOCATING);
    ui.set_accent(accent);
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
            source: EvidenceSource::RuntimeProbeWithSystemRule,
            owner: OwnerAttribution::KnownSystem,
            code: Some(1409),
            system_rule: Some(SystemRule::LockWorkstation),
            detail: None,
        };
        assert_eq!(evidence_label(&report), "系统规则 + 运行时探测");
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
            "冲突已确认；定位软件最多触发两次组合，并尽量拦截原动作。"
        );
    }
}
