//! One explicitly armed key picker, using this window's native key events only.
use crate::{AppWindow, sequence_protocol as plan};
use slint::{
    ComponentHandle,
    winit_030::{
        EventResult, WinitWindowAccessor,
        winit::{
            event::{ElementState, WindowEvent},
            keyboard::PhysicalKey,
            platform::scancode::PhysicalKeyExtScancode,
        },
    },
};
use std::collections::HashSet;

#[derive(Default)]
struct Capture {
    down: HashSet<PhysicalKey>,
    consumed: HashSet<PhysicalKey>,
}

fn key_code(key: PhysicalKey) -> Option<u32> {
    #[cfg(target_os = "macos")]
    let code = key.to_scancode()?;
    #[cfg(windows)]
    let code = {
        use slint::winit_030::winit::keyboard::{KeyCode as K, PhysicalKey::Code};
        // Preserve physical keypad identity regardless of Num Lock. MapVirtualKey
        // is used for the remaining native scan codes, including the current layout.
        match key {
            Code(K::NumpadEnter) => plan::NUM_ENTER,
            Code(K::Numpad0) => 0x60,
            Code(K::Numpad1) => 0x61,
            Code(K::Numpad2) => 0x62,
            Code(K::Numpad3) => 0x63,
            Code(K::Numpad4) => 0x64,
            Code(K::Numpad5) => 0x65,
            Code(K::Numpad6) => 0x66,
            Code(K::Numpad7) => 0x67,
            Code(K::Numpad8) => 0x68,
            Code(K::Numpad9) => 0x69,
            Code(K::NumpadMultiply) => 0x6a,
            Code(K::NumpadAdd) => 0x6b,
            Code(K::NumpadComma) => 0x6c,
            Code(K::NumpadSubtract) => 0x6d,
            Code(K::NumpadDecimal) => 0x6e,
            Code(K::NumpadDivide) => 0x6f,
            Code(K::NumLock) => 0x90,
            Code(K::Pause) => 0x13,
            Code(K::PrintScreen) => 0x2c,
            _ => {
                use windows::Win32::UI::Input::KeyboardAndMouse::{
                    MAPVK_VSC_TO_VK_EX, MapVirtualKeyW,
                };
                unsafe { MapVirtualKeyW(key.to_scancode()?, MAPVK_VSC_TO_VK_EX) }
            }
        }
    };
    plan::keys()
        .iter()
        .any(|(allowed, _)| *allowed == code)
        .then_some(code)
}

impl Capture {
    fn key(
        &mut self,
        ui: &AppWindow,
        key: PhysicalKey,
        state: ElementState,
        repeat: bool,
        synthetic: bool,
    ) -> EventResult {
        if state == ElementState::Released {
            self.down.remove(&key);
            return if self.consumed.remove(&key) {
                EventResult::PreventDefault
            } else {
                EventResult::Propagate
            };
        }
        let was_down = !self.down.insert(key);
        if self.consumed.contains(&key) {
            return EventResult::PreventDefault;
        }
        if !ui.get_sequence_key_capturing() {
            return EventResult::Propagate;
        }
        if !was_down {
            self.consumed.insert(key);
        }
        if !repeat && !synthetic && !was_down {
            if let Some(code) = key_code(key) {
                ui.invoke_sequence_key_captured(code as i32);
            } else {
                ui.set_sequence_key_capture_unknown(true);
            }
        }
        EventResult::PreventDefault
    }

    fn focus_lost(&mut self, ui: &AppWindow) {
        self.down.clear();
        self.consumed.clear();
        ui.set_sequence_key_capturing(false);
    }
}

pub fn install(ui: &AppWindow) {
    let weak = ui.as_weak();
    let mut capture = Capture::default();
    ui.window().on_winit_window_event(move |_, event| {
        let Some(ui) = weak.upgrade() else {
            return EventResult::Propagate;
        };
        if !ui.get_professional()
            || ui.get_help_visible()
            || ui.get_sequence_tab() != 0
            || ui.get_sequence_action_kind() == 3
            || ui.get_mode() == "sequence-testing"
        {
            ui.set_sequence_key_capturing(false);
        }
        match event {
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => capture.key(
                &ui,
                event.physical_key,
                event.state,
                event.repeat,
                *is_synthetic,
            ),
            WindowEvent::Focused(false) => {
                capture.focus_lost(&ui);
                EventResult::Propagate
            }
            _ => EventResult::Propagate,
        }
    });
}
