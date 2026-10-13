#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod detector;
#[cfg(target_os = "macos")]
#[path = "macos/detector.rs"]
mod detector;
#[cfg(target_os = "macos")]
#[path = "macos/capture.rs"]
mod mac_capture;
#[cfg(any(target_os = "macos", test))]
#[path = "macos/keys.rs"]
mod mac_keys;
mod report;
#[cfg(not(any(windows, target_os = "macos")))]
compile_error!("KeyCrash supports Windows and macOS");
mod i18n;
#[cfg(windows)]
mod key_capture;
#[cfg(windows)]
mod owner_probe;

#[cfg(target_os = "macos")]
#[path = "macos/filters.rs"]
mod mac_filters;
#[cfg(target_os = "macos")]
#[path = "macos/owner_probe.rs"]
mod owner_probe;

use detector::{EvidenceSource, OwnerAttribution, ProbeReport, ProbeStatus, Shortcut};
#[cfg(windows)]
use detector::{shortcut_from_parts, shortcut_from_virtual_key};
use owner_probe::OwnerProbeResult;
use slint::{Color, ComponentHandle, SharedString};
use std::cell::RefCell;
#[cfg(windows)]
use std::ffi::OsString;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
#[cfg(windows)]
use std::thread;
#[cfg(windows)]
use std::time::Duration;

slint::include_modules!();

const MODE_IDLE: &str = "idle";
const MODE_WAITING_KEY: &str = "waiting-key";
const MODE_AVAILABLE: &str = "available";
const MODE_OCCUPIED: &str = "occupied";
const MODE_RESERVED: &str = "reserved";
const MODE_LOCATING: &str = "locating";
const MODE_OWNER_FOUND: &str = "owner-found";
#[cfg(windows)]
const MODE_UNSUPPORTED: &str = "unsupported";
const MODE_ERROR: &str = "error";
const PROJECT_URL: &str = "https://github.com/Yuelioi/keycrash";

fn main() -> Result<(), slint::PlatformError> {
    #[cfg(target_os = "macos")]
    if let Some(code) = mac_filters::guardian_entry() {
        std::process::exit(code);
    }
    let ui = AppWindow::new()?;
    ui.set_is_macos(cfg!(target_os = "macos"));
    ui.set_app_version(env!("CARGO_PKG_VERSION").into());
    let last_blocked_shortcut = Rc::new(RefCell::new(None::<Shortcut>));
    #[cfg(target_os = "macos")]
    let active_probe = Rc::new(RefCell::new(
        None::<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    ));
    #[cfg(target_os = "macos")]
    let probe_worker = Rc::new(RefCell::new(None::<std::thread::JoinHandle<()>>));
    set_idle(&ui);

    #[cfg(target_os = "macos")]
    install_filter_actions(
        &ui,
        last_blocked_shortcut.clone(),
        active_probe.clone(),
        probe_worker.clone(),
    );

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
        ui.on_begin_detection(move |ctrl, alt, shift, win| {
            if let Some(ui) = weak_ui.upgrade() {
                *last_blocked_shortcut.borrow_mut() = None;
                set_waiting_for_key(&ui, ctrl, alt, shift, win);
            }
        });
    }

    #[cfg(windows)]
    {
        let weak_ui = ui.as_weak();
        let last_blocked_shortcut = last_blocked_shortcut.clone();
        ui.on_action_key_entered(move |action_key, ctrl, alt, shift, win| {
            let Some(ui) = weak_ui.upgrade() else {
                return;
            };

            let shortcut = match shortcut_from_parts(ctrl, alt, shift, win, action_key.as_str()) {
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

    #[cfg(windows)]
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

    #[cfg(target_os = "macos")]
    {
        let weak_ui = ui.as_weak();
        let last_blocked_shortcut = last_blocked_shortcut.clone();
        let active_probe = active_probe.clone();
        let probe_worker = probe_worker.clone();
        ui.on_locate_owner(move || {
            let Some(ui) = weak_ui.upgrade() else { return; };
            if ui.get_mode() == MODE_LOCATING { return; }
            let Some(shortcut) = last_blocked_shortcut.borrow().clone() else { return; };
            if !owner_probe::has_permission() {
                owner_probe::request_permission();
                set_view(&ui, MODE_OCCUPIED, "需要辅助功能权限",
                    "在系统设置中允许 KeyCrash，然后返回点击定位。",
                    &shortcut.label(), "授权前不会发送快捷键", "定位占用软件", color("8B6B16"));
                let _ = Command::new("/usr/bin/open")
                    .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                    .spawn();
                return;
            }
            let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            *active_probe.borrow_mut() = Some(cancelled.clone());
            let shortcut_label = shortcut.label();
            set_view(&ui, MODE_LOCATING, "正在定位占用软件",
                "正在观察此组合的接收进程，原动作可能执行。",
                &shortcut_label, "macOS 快捷键事件", "取消", color("C9342A"));
            let result_ui = ui.as_weak();
            if let Some(previous) = probe_worker.borrow_mut().take() {
                let _ = previous.join();
            }
            *probe_worker.borrow_mut() = Some(std::thread::spawn(move || {
                let report = owner_probe::locate_owner(&shortcut, &cancelled);
                let _ = slint::invoke_from_event_loop(move || {
                    if !cancelled.load(std::sync::atomic::Ordering::Relaxed)
                        && let Some(ui) = result_ui.upgrade() {
                        apply_owner_report(&ui, &shortcut_label, report);
                    }
                });
            }));
        });
    }

    {
        let weak_ui = ui.as_weak();
        #[cfg(target_os = "macos")]
        let active_probe = active_probe.clone();
        ui.on_cancel_requested(move || {
            #[cfg(target_os = "macos")]
            if let Some(cancelled) = active_probe.borrow_mut().take() {
                cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            if let Some(ui) = weak_ui.upgrade() {
                if ui.get_filter_running() {
                    ui.set_state_title(
                        i18n::text(
                            i18n::Language::from_code(ui.get_language().as_str()),
                            "正在恢复全局监听",
                        )
                        .into(),
                    );
                    ui.set_action_enabled(false);
                } else {
                    set_idle(&ui);
                }
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

    ui.on_open_github(|| {
        if let Err(error) = open_project_url() {
            eprintln!("Could not open GitHub: {error}");
        }
    });

    #[cfg(windows)]
    let _action_key_fallback = install_action_key_fallback(&ui, last_blocked_shortcut);

    #[cfg(target_os = "macos")]
    let _capture = {
        let armed_ui = ui.as_weak();
        let result_ui = ui.as_weak();
        mac_capture::install(
            move || {
                armed_ui
                    .upgrade()
                    .is_some_and(|ui| ui.get_mode() == MODE_WAITING_KEY)
            },
            move |key_code, characters| {
                let Some(ui) = result_ui.upgrade() else {
                    return;
                };
                if key_code == 0x35 {
                    ui.invoke_cancel_requested();
                    return;
                }
                let shortcut = detector::Shortcut::from_native(
                    ui.get_ctrl_selected(),
                    ui.get_alt_selected(),
                    ui.get_shift_selected(),
                    ui.get_win_selected(),
                    key_code,
                    characters,
                );
                detect_shortcut(&ui, &last_blocked_shortcut, shortcut);
            },
        )
        .ok_or_else(|| slint::PlatformError::Other("Could not install local key capture".into()))?
    };

    #[cfg(debug_assertions)]
    let _diagnostic_snapshots = diagnostic_snapshots(&ui);
    let result = ui.run();
    #[cfg(target_os = "macos")]
    if let Some(cancelled) = active_probe.borrow_mut().take() {
        cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    #[cfg(target_os = "macos")]
    if let Some(worker) = probe_worker.borrow_mut().take() {
        // Finish any paired key-up and tear down the event tap before exit.
        let _ = worker.join();
    }
    result
}

// Debug-only visual acceptance: capture this renderer, without screen-recording
// permission or reading other windows. No snapshot option exists in release builds.
#[cfg(debug_assertions)]
fn diagnostic_snapshots(ui: &AppWindow) -> Option<slint::Timer> {
    use std::io::Write;
    let args: Vec<_> = std::env::args().collect();
    let index = args.iter().position(|a| a == "--diagnostic-snapshots")?;
    let directory = std::path::PathBuf::from(args.get(index + 1)?);
    if !directory.is_dir() {
        return None;
    }
    let weak = ui.as_weak();
    let seen = RefCell::new(std::collections::BTreeSet::new());
    let previous = RefCell::new(String::new());
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(400),
        move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let name = format!(
                "{}-{}-{}-{}",
                ui.get_mode(),
                ui.get_language(),
                ui.get_filter_visible(),
                ui.get_filter_selected()
            );
            // Wait one rendering interval after a state/language transition.
            if *previous.borrow() != name {
                *previous.borrow_mut() = name;
                return;
            }
            if seen.borrow().len() >= 24 || seen.borrow().contains(&name) {
                return;
            }
            let Ok(pixels) = ui.window().take_snapshot() else {
                return;
            };
            let Ok(file) = std::fs::File::create(directory.join(format!("{name}.ppm"))) else {
                return;
            };
            let mut file = std::io::BufWriter::new(file);
            if write!(file, "P6\n{} {}\n255\n", pixels.width(), pixels.height()).is_err() {
                return;
            }
            for pixel in pixels.as_bytes().as_chunks::<4>().0 {
                if file.write_all(&pixel[..3]).is_err() {
                    return;
                }
            }
            if file.flush().is_ok() {
                seen.borrow_mut().insert(name);
            }
        },
    );
    Some(timer)
}

#[cfg(target_os = "macos")]
type ActiveProbe = Rc<RefCell<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>>>;
#[cfg(target_os = "macos")]
type ProbeWorker = Rc<RefCell<Option<std::thread::JoinHandle<()>>>>;

#[cfg(target_os = "macos")]
fn install_filter_actions(
    ui: &AppWindow,
    shortcut: Rc<RefCell<Option<Shortcut>>>,
    active: ActiveProbe,
    worker: ProbeWorker,
) {
    use slint::Model;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let candidates = Rc::new(RefCell::new(Vec::<mac_filters::Candidate>::new()));
    let choices = Rc::new(slint::VecModel::<FilterChoice>::default());
    ui.set_filter_choices(choices.clone().into());
    {
        let weak = ui.as_weak();
        let candidates = candidates.clone();
        let choices = choices.clone();
        ui.on_show_filters(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if ui.get_mode() == MODE_LOCATING || !ui.get_filter_available() {
                return;
            }
            let language = i18n::Language::from_code(ui.get_language().as_str());
            ui.set_filter_visible(true);
            ui.set_filter_selected(0);
            choices.set_vec(Vec::new());
            candidates.borrow_mut().clear();
            match mac_filters::candidates() {
                Ok(found) => {
                    let message = if found.is_empty() {
                        "没有找到可参与检查的全局监听软件。"
                    } else {
                        "这里只列出候选软件，尚未确认它们使用了此组合。"
                    };
                    ui.set_filter_message(i18n::text(language, message).into());
                    choices.set_vec(
                        found
                            .iter()
                            .map(|c| FilterChoice {
                                name: c.name.clone().into(),
                                path: c.path.clone().into(),
                                pid: c.pid as i32,
                                selected: false,
                            })
                            .collect::<Vec<_>>(),
                    );
                    *candidates.borrow_mut() = found;
                }
                Err(error) => ui.set_filter_message(i18n::text(language, error).into()),
            }
        });
    }
    {
        let weak = ui.as_weak();
        let choices = choices.clone();
        ui.on_toggle_filter(move |index, selected| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let Ok(index) = usize::try_from(index) else {
                return;
            };
            if let Some(mut row) = choices.row_data(index) {
                if selected && ui.get_filter_selected() >= 6 {
                    return;
                }
                row.selected = selected;
                choices.set_row_data(index, row);
                ui.set_filter_selected(
                    (0..choices.row_count())
                        .filter(|&i| choices.row_data(i).is_some_and(|c| c.selected))
                        .count() as i32,
                );
            }
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_start_filters(move || {
            let Some(ui)=weak.upgrade() else { return; };
            if ui.get_mode()==MODE_LOCATING || !ui.get_filter_visible() { return; }
            let Some(shortcut)=shortcut.borrow().clone() else { return; };
            let language=i18n::Language::from_code(ui.get_language().as_str());
            if !owner_probe::has_permission() {
                owner_probe::request_permission();
                ui.set_filter_message(i18n::text(language,"在系统设置中允许 KeyCrash，然后返回点击开始检查。").into());
                let _=Command::new("/usr/bin/open").arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility").spawn();
                return;
            }
            let selected:Vec<_>=candidates.borrow().iter().enumerate()
                .filter(|(i,_)|choices.row_data(*i).is_some_and(|c|c.selected))
                .map(|(_,c)|c.clone()).collect();
            if selected.is_empty() || selected.len()>6 { return; }
            ui.set_filter_visible(false);
            ui.set_filter_running(true);
            let label=shortcut.label();
            set_view(&ui,MODE_LOCATING,"正在检查全局监听","将多次触发此组合，结束或取消后恢复监听。",
                &label,"仅检查所选软件","取消",color("8B6B16"));
            let cancelled=Arc::new(AtomicBool::new(false));
            *active.borrow_mut()=Some(cancelled.clone());
            let result_ui=ui.as_weak();
            if let Some(previous)=worker.borrow_mut().take() { let _=previous.join(); }
            *worker.borrow_mut()=Some(std::thread::spawn(move || {
                let report=mac_filters::locate(&shortcut,&selected,&cancelled);
                let _=slint::invoke_from_event_loop(move || {
                    let Some(ui)=result_ui.upgrade() else { return; };
                    ui.set_filter_running(false);
                    if cancelled.load(Ordering::Relaxed) && !matches!(&report,Err(e) if *e==mac_filters::RESTORE) {
                        set_idle(&ui); return;
                    }
                    match report {
                        Ok(findings)=>apply_filter_report(&ui,&label,findings),
                        Err(error)=>set_view(&ui,MODE_ERROR,"全局监听检查未完成",error,&label,"未输出定位结论","重新检测",color("A33B32")),
                    }
                });
            }));
        });
    }
}

#[cfg(target_os = "macos")]
fn apply_filter_report(ui: &AppWindow, label: &str, findings: mac_filters::Findings) {
    let language = i18n::Language::from_code(ui.get_language().as_str());
    let mut lines = Vec::new();
    for (heading, owners) in [
        ("改变按键传递的软件", findings.filters.as_slice()),
        ("热键接收者", findings.receivers.as_slice()),
    ] {
        if owners.is_empty() {
            continue;
        }
        lines.push(i18n::text(language, heading).to_string());
        for owner in owners {
            let path = Path::new(&owner.path);
            let name = path
                .ancestors()
                .find(|p| p.extension().is_some_and(|e| e == "app"))
                .and_then(Path::file_stem)
                .or_else(|| path.file_name())
                .unwrap_or_default()
                .to_string_lossy();
            lines.push(format!("{name} · PID {}\n{}", owner.pid, owner.path));
        }
        lines.push(String::new());
    }
    let found = !lines.is_empty();
    if !found {
        lines.push(
            i18n::text(
                language,
                "所选软件未改变此次按键传递，不能排除未参与检查的软件。",
            )
            .to_string(),
        );
    }
    set_view(
        ui,
        "filter-result",
        if found {
            "全局监听检查完成"
        } else {
            "未确认影响此组合的软件"
        },
        "检查已结束，所选监听已恢复。",
        label,
        "仅检查所选软件",
        "重新检测",
        color("007A7E"),
    );
    ui.set_filter_report(lines.join("\n").into());
}

#[cfg(windows)]
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
                    ui.get_win_selected(),
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
    if (report.status == ProbeStatus::Blocked
        || (cfg!(target_os = "macos") && report.status == ProbeStatus::Available))
        && (cfg!(target_os = "macos") || report.system_rule.is_none())
    {
        *last_blocked_shortcut.borrow_mut() = Some(shortcut.clone());
    } else {
        *last_blocked_shortcut.borrow_mut() = None;
    }
    apply_report(ui, &shortcut.label(), report);
    #[cfg(target_os = "macos")]
    ui.set_filter_available(last_blocked_shortcut.borrow().is_some());
}

fn apply_report(ui: &AppWindow, shortcut_label: &str, report: ProbeReport) {
    match report.status {
        ProbeStatus::Available => set_view(
            ui,
            MODE_AVAILABLE,
            if cfg!(target_os = "macos") {
                "此组合可临时注册"
            } else {
                "未发现注册冲突"
            },
            if cfg!(target_os = "macos") {
                "可临时注册；定位会触发组合，用于检查共享热键。"
            } else {
                "其他软件仍可能监听此键；本次临时注册已释放。"
            },
            shortcut_label,
            &evidence_label(&report),
            if cfg!(target_os = "macos") {
                "定位占用软件"
            } else {
                "再测一次"
            },
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
                if cfg!(target_os = "macos") && report.system_rule.is_some() {
                    "系统设置中有此组合"
                } else if report.system_rule.is_some() {
                    "系统保留快捷键"
                } else {
                    "已发现注册冲突"
                },
                blocked_detail(&report),
                shortcut_label,
                &evidence_label(&report),
                if !cfg!(target_os = "macos") && report.system_rule.is_some() {
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
                &report.detail.map(str::to_owned).unwrap_or_else(|| {
                    format!(
                        "{} 返回了系统错误 {code}，请再试一次。",
                        if cfg!(target_os = "macos") {
                            "macOS"
                        } else {
                            "Windows"
                        }
                    )
                }),
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

#[cfg(windows)]
fn set_locating_owner(ui: &AppWindow, shortcut_label: &str) {
    set_view(
        ui,
        MODE_LOCATING,
        "正在定位占用软件",
        "正在自动检查占用软件；如需更高权限，Windows 会请求一次确认。",
        shortcut_label,
        "自动深度定位 · x64 / x86",
        "定位中…",
        color("C9342A"),
    );
}

#[cfg(windows)]
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

#[cfg(windows)]
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

#[cfg(target_os = "macos")]
fn apply_owner_report(ui: &AppWindow, shortcut_label: &str, report: OwnerProbeResult) {
    match report {
        OwnerProbeResult::Found(owners) => {
            let names = owners
                .iter()
                .map(|owner| {
                    let path = Path::new(&owner.path);
                    path.ancestors()
                        .find(|p| p.extension().is_some_and(|e| e == "app"))
                        .and_then(Path::file_stem)
                        .or_else(|| path.file_name())
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| format!("PID {}", owner.pid))
                })
                .collect::<Vec<_>>()
                .join("、");
            let detail = owners
                .iter()
                .map(|o| format!("PID {} · {}", o.pid, o.path))
                .collect::<Vec<_>>()
                .join("\n");
            set_view(
                ui,
                MODE_OWNER_FOUND,
                &format!("{names}占用了它"),
                &detail,
                shortcut_label,
                "macOS 热键接收事件 · 已匹配本次组合",
                "再测一次",
                color("007A7E"),
            );
            if owners.len() == 1 {
                ui.set_owner_path(owners[0].path.clone().into());
            }
        }
        OwnerProbeResult::NotFound => set_view(
            ui,
            MODE_ERROR,
            "没有观察到占用软件",
            "未收到匹配的热键事件；目标可能忽略模拟输入，或占用已释放。",
            shortcut_label,
            "macOS 快捷键事件 · 未匹配",
            "重新检测",
            color("8B6B16"),
        ),
        OwnerProbeResult::PermissionRequired => set_view(
            ui,
            MODE_OCCUPIED,
            "需要辅助功能权限",
            "在系统设置中允许 KeyCrash，然后返回点击定位。",
            shortcut_label,
            "授权前不会发送快捷键",
            "定位占用软件",
            color("8B6B16"),
        ),
        OwnerProbeResult::Error(error) => set_view(
            ui,
            MODE_ERROR,
            "定位没有完成",
            error,
            shortcut_label,
            "macOS 快捷键事件",
            "重新检测",
            color("A33B32"),
        ),
        OwnerProbeResult::Cancelled => set_idle(ui),
    }
}

#[cfg(target_os = "macos")]
fn open_file_location(path: &Path) -> Result<(), String> {
    Command::new("/usr/bin/open")
        .arg("-R")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
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
        (EvidenceSource::SystemRule, _) => if cfg!(target_os = "macos") {
            "macOS 系统快捷键 · 未尝试注册"
        } else {
            "Windows 系统规则 · 未尝试注册"
        }
        .into(),
        (EvidenceSource::SystemError, _) => {
            let code = report.code.unwrap_or_default();
            format!(
                "{} 系统错误 · {code}",
                if cfg!(target_os = "macos") {
                    "macOS"
                } else {
                    "Windows"
                }
            )
        }
        #[cfg(windows)]
        _ => "检测证据不可用".into(),
    }
}

fn set_idle(ui: &AppWindow) {
    set_view(
        ui,
        MODE_IDLE,
        "选择修饰键",
        if cfg!(target_os = "macos") {
            "点选 ⌃、⌥、⇧、⌘；开始后只按一个目标键。"
        } else {
            "单击 Ctrl、Alt、Shift、Win；开始后只按一个目标键。"
        },
        "等待目标键",
        "安全输入 · 不发送完整组合",
        "开始检测",
        color("C9342A"),
    );
}

fn set_waiting_for_key(ui: &AppWindow, ctrl: bool, alt: bool, shift: bool, win: bool) {
    set_view(
        ui,
        MODE_WAITING_KEY,
        "请按一个目标键",
        "只按 A、F9、Space 等目标键；不要再按修饰键。",
        &modifier_preview(ctrl, alt, shift, win),
        "",
        "取消",
        color("C9342A"),
    );
}

fn modifier_preview(ctrl: bool, alt: bool, shift: bool, win: bool) -> String {
    let mut parts = Vec::new();
    if ctrl {
        parts.push(if cfg!(target_os = "macos") {
            "⌃"
        } else {
            "Ctrl"
        });
    }
    if alt {
        parts.push(if cfg!(target_os = "macos") {
            "⌥"
        } else {
            "Alt"
        });
    }
    if shift {
        parts.push(if cfg!(target_os = "macos") {
            "⇧"
        } else {
            "Shift"
        });
    }
    if win {
        parts.push(if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "Win"
        });
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
    ui.set_action_enabled(mode != MODE_LOCATING || cfg!(target_os = "macos"));
    ui.set_accent(accent);
    ui.set_owner_path(SharedString::new());
    if mode == MODE_IDLE || mode == MODE_WAITING_KEY {
        ui.set_filter_available(false);
        ui.set_filter_visible(false);
    }
}

fn relocalize_view(ui: &AppWindow, language: i18n::Language) {
    ui.set_filter_message(i18n::text(language, ui.get_filter_message().as_str()).into());
    ui.set_filter_report(
        ui.get_filter_report()
            .lines()
            .map(|line| i18n::text(language, line))
            .collect::<Vec<_>>()
            .join("\n")
            .into(),
    );
    let owner_error =
        i18n::text(i18n::Language::Zh, ui.get_state_title().as_str()) == "定位没有完成";
    ui.set_state_title(SharedString::from(i18n::text(
        language,
        ui.get_state_title().as_str(),
    )));
    let current_detail = ui.get_state_detail();
    let detail = if owner_error && cfg!(windows) {
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

#[cfg(windows)]
fn owner_display_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("未知进程")
        .into()
}

#[cfg(windows)]
fn owner_evidence(architecture: &str, pid: u32, thread_id: u32) -> String {
    format!("WM_HOTKEY · {architecture} · PID {pid} / TID {thread_id}")
}

#[cfg(windows)]
fn explorer_select_argument(path: &Path) -> OsString {
    let mut argument = OsString::from("/select,\"");
    argument.push(path.as_os_str());
    argument.push("\"");
    argument
}

#[cfg(windows)]
fn open_file_location(path: &Path) -> Result<(), String> {
    Command::new("explorer.exe")
        // Explorer's parser expects /select,"<path>". Command::arg would quote
        // the whole argument when the path contains spaces such as Program Files.
        .raw_arg(explorer_select_argument(path))
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg(windows)]
fn open_project_url() -> Result<(), String> {
    use windows::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    use windows::core::{PCWSTR, w};

    let url: Vec<u16> = PROJECT_URL.encode_utf16().chain(Some(0)).collect();
    // ShellExecute opens HTTPS URLs with the user's default browser.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(url.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    let code = result.0 as isize;
    if code > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecute failed ({code})"))
    }
}

#[cfg(target_os = "macos")]
fn open_project_url() -> Result<(), String> {
    Command::new("/usr/bin/open")
        .arg(PROJECT_URL)
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

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::detector::SystemRule;

    #[test]
    fn modifier_preview_uses_the_same_order_as_shortcut_labels() {
        assert_eq!(
            modifier_preview(true, true, true, true),
            "Ctrl + Alt + Shift + Win + …"
        );
        assert_eq!(modifier_preview(false, false, false, true), "Win + …");
        assert_eq!(modifier_preview(false, false, false, false), "按目标键…");
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
    fn explorer_argument_quotes_paths_with_spaces_for_explorer() {
        assert_eq!(
            explorer_select_argument(Path::new("C:\\Program Files\\Tencent\\QQNT\\QQ.exe")),
            OsString::from("/select,\"C:\\Program Files\\Tencent\\QQNT\\QQ.exe\"")
        );
    }
}
