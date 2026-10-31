//! Professional mode: edit a platform-native plan, then explicitly test it.
use crate::{
    AppWindow, color, i18n,
    sequence_history::History,
    sequence_protocol as plan,
    sequence_test::{Job, Notice, Outcome},
};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, rc::Rc, time::Duration};
struct State {
    steps: Vec<plan::Step>,
    history: History,
    job: Option<Job>,
    outcome: Option<Outcome>,
    requested_admin: bool,
    live_done: usize,
    cancelling: bool,
}
pub struct Professional {
    state: Rc<RefCell<State>>,
    _timer: slint::Timer,
}
impl Drop for Professional {
    fn drop(&mut self) {
        self.state.borrow_mut().job.take();
    }
}
fn language(ui: &AppWindow) -> i18n::Language {
    i18n::Language::from_code(ui.get_language().as_str())
}
fn text(ui: &AppWindow, value: &str) -> SharedString {
    i18n::text(language(ui), value).into()
}
fn summary(steps: &[plan::Step]) -> String {
    steps
        .iter()
        .map(|step| {
            if step.action == plan::Action::Wait {
                format!("{} ms", step.delay_ms)
            } else {
                let action = match step.action {
                    plan::Action::Tap => "↕",
                    plan::Action::Down => "↓",
                    plan::Action::Up => "↑",
                    plan::Action::Wait => unreachable!(),
                };
                format!("{} {}", plan::key_label(step.key), action)
            }
        })
        .collect::<Vec<_>>()
        .join(" → ")
}
fn history_rows(ui: &AppWindow, state: &State) {
    ui.set_sequence_history(ModelRc::new(VecModel::from(
        state
            .history
            .entries
            .iter()
            .map(|steps| summary(steps).into())
            .collect::<Vec<SharedString>>(),
    )));
}
fn rows(ui: &AppWindow, state: &State) {
    let done = state
        .outcome
        .as_ref()
        .map_or(state.live_done, |outcome| outcome.completed_steps);
    let rows = state
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| crate::SequenceStep {
            action: step.action as i32,
            key: if step.action == plan::Action::Wait {
                SharedString::new()
            } else {
                plan::key_label(step.key).into()
            },
            delay: step.delay_ms as i32,
            done: index < done,
        })
        .collect::<Vec<_>>();
    ui.set_sequence_steps(ModelRc::new(VecModel::from(rows)));
    ui.set_recorded_summary(summary(&state.steps).into());
    let validation = plan::validate(&state.steps);
    ui.set_sequence_valid(validation.is_ok());
    ui.set_sequence_validation(
        validation
            .err()
            .map_or_else(SharedString::new, |error| text(ui, error)),
    );
}
fn edited(ui: &AppWindow, state: &mut State) {
    state.outcome.take();
    state.live_done = 0;
    ui.set_recorded_events(ModelRc::default());
    ui.set_recorded_responses(ModelRc::default());
    ui.set_sequence_tab(0);
    ui.set_recording_status(SharedString::new());
    ui.set_sequence_scope_ready(false);
    rows(ui, state);
    if ui.get_professional() {
        set_idle(ui);
    }
}
pub fn install(ui: &AppWindow) -> Professional {
    let history = match History::load() {
        Ok(history) => history,
        Err(_) => {
            ui.set_sequence_history_error("load".into());
            History::default()
        }
    };
    let state = Rc::new(RefCell::new(State {
        steps: history.entries.first().cloned().unwrap_or_default(),
        history,
        job: None,
        outcome: None,
        requested_admin: false,
        live_done: 0,
        cancelling: false,
    }));
    ui.set_sequence_keys(ModelRc::new(VecModel::from(
        plan::keys()
            .into_iter()
            .map(|(_, label)| label.into())
            .collect::<Vec<SharedString>>(),
    )));
    ui.set_sequence_key(
        plan::keys()
            .iter()
            .position(|(key, _)| *key == plan::letter_key(b'C'))
            .unwrap() as i32,
    );
    rows(ui, &state.borrow());
    history_rows(ui, &state.borrow());
    {
        let weak = ui.as_weak();
        let state = state.clone();
        ui.on_detection_mode_changed(move |professional| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let mut state = state.borrow_mut();
            if state.job.is_some()
                || ui.get_mode() == "locating"
                || ui.get_professional() == professional
            {
                return;
            }
            ui.set_professional(professional);
            ui.set_sequence_key_capturing(false);
            ui.set_help_visible(false);
            ui.set_filter_available(false);
            ui.set_filter_visible(false);
            edited(&ui, &mut state);
            crate::set_idle(&ui);
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        ui.on_sequence_clear(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let mut state = state.borrow_mut();
            if state.job.is_some() || ui.get_sequence_key_capturing() {
                return;
            }
            state.steps.clear();
            edited(&ui, &mut state);
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        ui.on_sequence_history_action(move |index, delete| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let mut state = state.borrow_mut();
            if state.job.is_some()
                || ui.get_sequence_key_capturing()
                || index < 0
                || index as usize >= state.history.entries.len()
            {
                return;
            }
            let index = index as usize;
            if delete {
                let selected = state.history.entries[index].clone();
                match History::delete(&selected) {
                    Ok(history) => state.history = history,
                    Err(_) => {
                        ui.set_sequence_history_error("save".into());
                        return;
                    }
                }
                ui.set_sequence_history_error(SharedString::new());
                history_rows(&ui, &state);
            } else {
                state.steps = state.history.entries[index].clone();
                edited(&ui, &mut state);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        ui.on_sequence_add(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let mut state = state.borrow_mut();
            if state.job.is_some() || ui.get_sequence_key_capturing() {
                return;
            }
            if state.steps.len() >= plan::MAX_STEPS {
                ui.set_sequence_validation(text(&ui, "最多编排 32 个步骤。"));
                return;
            }
            let action = match ui.get_sequence_action_kind() {
                0 => plan::Action::Tap,
                1 => plan::Action::Down,
                2 => plan::Action::Up,
                3 => plan::Action::Wait,
                _ => return,
            };
            let key = if action == plan::Action::Wait {
                0
            } else {
                let Some((key, _)) = plan::keys().get(ui.get_sequence_key() as usize).cloned()
                else {
                    return;
                };
                key
            };
            state.steps.push(plan::Step {
                action,
                key,
                delay_ms: if action == plan::Action::Wait {
                    ui.get_sequence_delay().clamp(20, 2000) as u32
                } else {
                    0
                },
            });
            edited(&ui, &mut state);
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        ui.on_sequence_edit(move |index, operation| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let mut state = state.borrow_mut();
            if state.job.is_some()
                || ui.get_sequence_key_capturing()
                || index < 0
                || index as usize >= state.steps.len()
            {
                return;
            }
            let index = index as usize;
            match operation {
                -1 if index > 0 => state.steps.swap(index, index - 1),
                1 if index + 1 < state.steps.len() => state.steps.swap(index, index + 1),
                0 => {
                    state.steps.remove(index);
                }
                _ => return,
            }
            edited(&ui, &mut state);
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        ui.on_sequence_capture_requested(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if state.borrow().job.is_some()
                || !ui.get_professional()
                || ui.get_help_visible()
                || ui.get_sequence_tab() != 0
                || ui.get_sequence_action_kind() == 3
            {
                return;
            }
            ui.set_sequence_key_capture_unknown(false);
            ui.set_sequence_key_capturing(!ui.get_sequence_key_capturing());
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        ui.on_sequence_key_captured(move |code| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if !ui.get_sequence_key_capturing()
                || state.borrow().job.is_some()
                || !ui.get_professional()
                || ui.get_help_visible()
                || ui.get_sequence_tab() != 0
                || ui.get_sequence_action_kind() == 3
            {
                return;
            }
            let Some(index) = plan::keys().iter().position(|(key, _)| *key == code as u32) else {
                ui.set_sequence_key_capture_unknown(true);
                return;
            };
            ui.set_sequence_key(index as i32);
            ui.set_sequence_key_capturing(false);
            ui.set_sequence_key_capture_unknown(false);
        });
    }
    crate::sequence_key_capture::install(ui);
    {
        let weak = ui.as_weak();
        let state = state.clone();
        ui.on_sequence_action(move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if !ui.get_professional() || ui.get_sequence_key_capturing() {
                return;
            }
            let mut state = state.borrow_mut();
            if let Some(job) = &state.job {
                job.cancel();
                state.cancelling = true;
                ui.set_recording_status(text(&ui, "正在取消并松开本次按键…"));
                ui.set_action_enabled(false);
                return;
            }
            if let Err(error) = plan::validate(&state.steps) {
                ui.set_sequence_validation(text(&ui, error));
                return;
            }
            state.outcome.take();
            state.live_done = 0;
            state.cancelling = false;
            ui.set_sequence_tab(0);
            ui.set_sequence_scope_ready(false);
            ui.set_recorded_events(ModelRc::default());
            ui.set_recorded_responses(ModelRc::default());
            ui.set_recording_status(SharedString::new());
            state.requested_admin = ui.get_include_admin();
            rows(&ui, &state);
            #[cfg(target_os = "macos")]
            if !crate::owner_probe::has_permission() {
                crate::owner_probe::request_permission();
                view(
                    &ui,
                    "sequence-result",
                    "需要辅助功能权限",
                    "请在系统设置中允许辅助功能后重试。",
                    "测试序列",
                );
                return;
            }
            match Job::start(state.steps.clone(), state.requested_admin) {
                Ok(job) => {
                    state.job = Some(job);
                    view(
                        &ui,
                        "sequence-testing",
                        "正在准备测试",
                        "准备完成后将倒计时，请稍候。",
                        "取消测试",
                    );
                }
                Err(error) => {
                    ui.set_recording_status(text(&ui, &error));
                }
            }
            rows(&ui, &state);
        });
    }
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    let timer_state = state.clone();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(40),
        move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let mut state = timer_state.borrow_mut();
            let notices = state
                .job
                .as_ref()
                .map(|job| job.receiver.try_iter().collect::<Vec<_>>())
                .unwrap_or_default();
            for notice in notices {
                match notice {
                    Notice::Preparing => {}
                    Notice::Countdown(count) => {
                        ui.set_sequence_scope_ready(true);
                        ui.set_recording_admin(state.requested_admin);
                        view(
                            &ui,
                            "sequence-testing",
                            "即将发送测试序列",
                            "全局快捷键可停留；应用内快捷键请切到目标。",
                            "取消测试",
                        );
                        ui.set_recording_status(
                            format!("{} {}", count, i18n::text(language(&ui), "秒后发送")).into(),
                        );
                    }
                    Notice::Sending => {
                        match History::record(&state.steps) {
                            Ok(history) => {
                                state.history = history;
                                ui.set_sequence_history_error(SharedString::new());
                            }
                            Err(_) => ui.set_sequence_history_error("save".into()),
                        }
                        history_rows(&ui, &state);
                        view(
                            &ui,
                            "sequence-testing",
                            "正在测试序列",
                            "正在发送步骤并观察软件响应。",
                            "取消测试",
                        );
                        ui.set_recording_status(SharedString::new());
                    }
                    Notice::StepDone(count) => {
                        state.live_done = count;
                        rows(&ui, &state);
                        if !state.cancelling {
                            ui.set_recording_status(
                                format!(
                                    "{} {}/{}",
                                    i18n::text(language(&ui), "已完成步骤："),
                                    count,
                                    state.steps.len()
                                )
                                .into(),
                            );
                        }
                    }
                    Notice::Done(result) => {
                        state.job.take();
                        match result {
                            Ok(outcome) => {
                                render(&ui, &outcome);
                                finish(&ui, &outcome);
                                state.outcome = Some(outcome);
                                rows(&ui, &state);
                            }
                            Err(error) => {
                                view(
                                    &ui,
                                    "sequence-result",
                                    "无法开始测试",
                                    "检查组件或系统权限后重试。",
                                    "测试序列",
                                );
                                ui.set_recording_status(text(&ui, &error));
                            }
                        }
                    }
                }
            }
        },
    );
    Professional {
        state,
        _timer: timer,
    }
}
pub fn set_idle(ui: &AppWindow) {
    view(
        ui,
        "sequence-idle",
        "编排按键序列",
        "按顺序发送按键，观察软件响应。",
        "测试序列",
    );
    ui.set_action_enabled(ui.get_sequence_valid());
}
fn view(ui: &AppWindow, mode: &str, title: &str, detail: &str, action: &str) {
    crate::set_view(ui, mode, title, detail, "", "", action, color("007A7E"));
}
fn finish(ui: &AppWindow, outcome: &Outcome) {
    let (title, detail) = if outcome.release_unconfirmed {
        ("测试已停止", "组件异常结束，部分发送状态未知。")
    } else if outcome.error.is_some() {
        ("测试未完成", "部分步骤未能发送，请查看结果。")
    } else if outcome.cancelled {
        ("测试已取消", "已停止发送测试序列。")
    } else if outcome.incomplete {
        (
            "测试结果不完整",
            "不能确认完整序列，请查看发送记录和软件响应。",
        )
    } else if outcome
        .responses
        .iter()
        .any(|response| response.association.is_some())
    {
        (
            "已关联到软件响应",
            "软件活动绑定与测试输入相符，不代表独占。",
        )
    } else if !outcome.responses.is_empty() {
        ("已观察到软件响应", "响应已记录，整段序列的关联仍未确认。")
    } else {
        ("未观察到软件响应", "不能据此判断序列可用或没有其他监听。")
    };
    view(ui, "sequence-result", title, detail, "再次测试");
    ui.set_recording_status(
        outcome
            .error
            .as_ref()
            .map_or_else(SharedString::new, |error| text(ui, error)),
    );
    ui.set_sequence_tab(if !outcome.responses.is_empty() {
        2
    } else if !outcome.events.is_empty() {
        1
    } else {
        0
    });
}
fn render(ui: &AppWindow, outcome: &Outcome) {
    ui.set_recorded_events(ModelRc::new(VecModel::from(
        outcome
            .events
            .iter()
            .map(|event| crate::RecordedEvent {
                time: format!("{:.3} s", event.elapsed.as_secs_f64()).into(),
                key: event.label.clone().into(),
                action: if event.initial {
                    "held"
                } else if event.repeat {
                    "repeat"
                } else if event.down {
                    "down"
                } else if event.missing_press {
                    "up-missing"
                } else {
                    "up"
                }
                .into(),
                injected: event.injected,
            })
            .collect::<Vec<_>>(),
    )));
    ui.set_recorded_responses(ModelRc::new(VecModel::from(
        outcome
            .responses
            .iter()
            .map(|response| crate::ObservedResponse {
                time: format!("{:.3} s", response.elapsed.as_secs_f64()).into(),
                app: response
                    .path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| format!("PID {}", response.pid))
                    .into(),
                path: response.path.to_string_lossy().into_owned().into(),
                kind: if response.kind == 1 {
                    "hotkey"
                } else if response.kind == 3 {
                    "carbon"
                } else {
                    "gd-notice"
                }
                .into(),
                value: if response.kind == 1 {
                    format_shortcut(response.value & 0xffff, response.value >> 16)
                } else if response.kind == 3 {
                    String::new()
                } else {
                    response.value.to_string()
                }
                .into(),
                pid: response.pid as i32,
                tid: response.tid as i32,
                architecture: format!("{}-bit", response.architecture).into(),
                matched: response.association.is_some(),
                input_range: response
                    .association
                    .as_ref()
                    .map(|matched| {
                        format!(
                            "{:.3}–{:.3} s",
                            outcome.events[matched.first_event].elapsed.as_secs_f64(),
                            outcome.events[matched.last_event].elapsed.as_secs_f64()
                        )
                    })
                    .unwrap_or_default()
                    .into(),
                binding: response
                    .binding
                    .as_ref()
                    .and_then(|snapshot| snapshot.bindings.get(response.value as usize))
                    .filter(|binding| binding.key1 != 0)
                    .map(|binding| {
                        let first = format_shortcut(binding.modifiers, binding.key1);
                        if binding.key2 == 0 {
                            first
                        } else {
                            format!(
                                "{first} → {}",
                                format_shortcut(binding.modifiers, binding.key2)
                            )
                        }
                    })
                    .unwrap_or_default()
                    .into(),
            })
            .collect::<Vec<_>>(),
    )));
}
fn format_shortcut(modifiers: u32, key: u32) -> String {
    #[cfg(windows)]
    {
        crate::detector::format_shortcut(modifiers, key)
    }
    #[cfg(target_os = "macos")]
    {
        crate::mac_keys::shortcut_label(modifiers, &plan::key_label(key))
    }
}
pub fn relocalize(ui: &AppWindow, lang: i18n::Language) {
    ui.set_recording_status(i18n::text(lang, ui.get_recording_status().as_str()).into());
    ui.set_sequence_validation(i18n::text(lang, ui.get_sequence_validation().as_str()).into());
}
