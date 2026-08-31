use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

pub const VK_ESCAPE_CODE: u32 = 0x1B;

pub struct ActionKeyCapture {
    was_armed: bool,
    was_down: [bool; 256],
}

impl Default for ActionKeyCapture {
    fn default() -> Self {
        Self {
            was_armed: false,
            was_down: [false; 256],
        }
    }
}

impl ActionKeyCapture {
    pub fn observe<I>(
        &mut self,
        armed: bool,
        forbidden_modifier_down: bool,
        key_states: I,
    ) -> Option<u32>
    where
        I: IntoIterator<Item = (u32, bool)>,
    {
        let first_armed_tick = armed && !self.was_armed;
        self.was_armed = armed;
        let mut pressed = None;

        for (virtual_key, is_down) in key_states {
            let Some(previous) = self.was_down.get_mut(virtual_key as usize) else {
                continue;
            };
            if pressed.is_none()
                && armed
                && !first_armed_tick
                && !forbidden_modifier_down
                && is_down
                && !*previous
            {
                pressed = Some(virtual_key);
            }
            *previous = is_down;
        }

        pressed
    }
}

pub fn supported_virtual_keys() -> Vec<u32> {
    let mut keys = vec![
        0x08,
        0x09,
        0x0D,
        0x13,
        0x14,
        VK_ESCAPE_CODE,
        0x20,
        0x21,
        0x22,
        0x23,
        0x24,
        0x25,
        0x26,
        0x27,
        0x28,
        0x2C,
        0x2D,
        0x2E,
        0x6A,
        0x6B,
        0x6D,
        0x6E,
        0x6F,
        0x90,
        0x91,
        0xBA,
        0xBB,
        0xBC,
        0xBD,
        0xBE,
        0xBF,
        0xC0,
        0xDB,
        0xDC,
        0xDD,
        0xDE,
    ];
    keys.extend(0x30..=0x39);
    keys.extend(0x41..=0x5A);
    keys.extend(0x60..=0x69);
    keys.extend(0x70..=0x87);
    keys
}

pub fn physical_modifier_is_down() -> bool {
    [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN]
        .into_iter()
        .any(modifier_is_down)
}

pub fn application_is_foreground() -> bool {
    let foreground_window = unsafe { GetForegroundWindow() };
    if foreground_window.0.is_null() {
        return false;
    }

    let mut foreground_pid = 0;
    unsafe {
        GetWindowThreadProcessId(foreground_window, Some(&mut foreground_pid));
    }
    foreground_pid == unsafe { GetCurrentProcessId() }
}

pub fn key_is_down(virtual_key: u32) -> bool {
    (unsafe { GetAsyncKeyState(virtual_key as i32) }) < 0
}

fn modifier_is_down(key: VIRTUAL_KEY) -> bool {
    key_is_down(key.0 as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_one_press_edge_only_while_armed() {
        let mut capture = ActionKeyCapture::default();

        assert_eq!(capture.observe(false, false, [(0x2C, false)]), None);
        assert_eq!(capture.observe(true, false, [(0x2C, false)]), None);
        assert_eq!(capture.observe(true, false, [(0x2C, true)]), Some(0x2C));
        assert_eq!(capture.observe(true, false, [(0x2C, true)]), None);
        assert_eq!(capture.observe(true, false, [(0x2C, false)]), None);
        assert_eq!(capture.observe(true, false, [(0x2C, true)]), Some(0x2C));
    }

    #[test]
    fn ignores_keys_pressed_before_capture_or_with_physical_modifiers() {
        let mut capture = ActionKeyCapture::default();

        assert_eq!(capture.observe(false, false, [(0x70, true)]), None);
        assert_eq!(capture.observe(true, false, [(0x70, true)]), None);
        assert_eq!(capture.observe(true, false, [(0x70, false)]), None);
        assert_eq!(capture.observe(true, true, [(0x70, true)]), None);
        assert_eq!(capture.observe(true, false, [(0x70, true)]), None);
        assert_eq!(capture.observe(true, false, [(0x70, false)]), None);
        assert_eq!(capture.observe(true, false, [(0x70, true)]), Some(0x70));
    }

    #[test]
    fn captures_f1_when_the_window_event_was_consumed() {
        let mut capture = ActionKeyCapture::default();

        assert_eq!(capture.observe(true, false, [(0x70, false)]), None);
        assert_eq!(capture.observe(true, false, [(0x70, true)]), Some(0x70));
        assert_eq!(capture.observe(true, false, [(0x70, true)]), None);
    }

    #[test]
    fn polls_every_action_key_family() {
        let keys = supported_virtual_keys();

        for expected in [0x31, 0x41, 0x60, 0x70, 0x87, 0x2C, 0xDE] {
            assert!(keys.contains(&expected), "missing VK_{expected:02X}");
        }
    }
}
