//! Executes only the explicit, validated plan supplied by the parent.
use crate::sequence_protocol as wire;
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    System::{SystemInformation::GetTickCount, Threading::*},
    UI::Input::KeyboardAndMouse::*,
};
struct Parent(HANDLE);
impl Drop for Parent {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn stopped(stop: &AtomicBool, parent: HANDLE) -> bool {
    stop.load(Ordering::Acquire) || unsafe { WaitForSingleObject(parent, 0) } != WAIT_TIMEOUT
}
fn pause(ms: u32, stop: &AtomicBool, parent: HANDLE) -> bool {
    let until = Instant::now() + Duration::from_millis(ms.into());
    while Instant::now() < until {
        if stopped(stop, parent) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    !stopped(stop, parent)
}
fn key_event(
    key: u32,
    down: bool,
    cleanup: bool,
    held: &mut BTreeSet<u32>,
    output: &mut impl Write,
) -> Result<(), String> {
    let tick = unsafe { GetTickCount() };
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: wire::virtual_key(key) as u16,
                wScan: 0,
                dwFlags: if down { 0 } else { KEYEVENTF_KEYUP }
                    | if wire::extended(key) {
                        KEYEVENTF_EXTENDEDKEY
                    } else {
                        0
                    },
                time: tick,
                dwExtraInfo: 0,
            },
        },
    };
    if unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) } != 1 {
        return Err("SendInput failed".into());
    }
    if down {
        held.insert(key);
    } else {
        held.remove(&key);
    }
    wire::Record {
        kind: wire::SENT,
        key,
        flags: if down { wire::DOWN } else { 0 } | if cleanup { wire::CLEANUP } else { 0 },
        tick,
        observed: unsafe { GetTickCount() },
    }
    .write(output)
    .map_err(|e| e.to_string())
}
pub fn run(parent_pid: u32) -> Result<(), String> {
    let parent = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, parent_pid) };
    if parent.is_null() {
        return Err("Cannot monitor sequence parent".into());
    }
    let parent = Parent(parent);
    let mut control = std::io::stdin();
    let plan = wire::read_plan(&mut control)?;
    let mut output = std::io::stdout();
    output
        .write_all(&wire::RESULT_MAGIC)
        .map_err(|e| e.to_string())?;
    wire::Record {
        kind: wire::READY,
        ..Default::default()
    }
    .write(&mut output)
    .map_err(|e| e.to_string())?;
    let mut go = [0];
    if control.read_exact(&mut go).is_err()
        || go[0] != 1
        || unsafe { WaitForSingleObject(parent.0, 0) } != WAIT_TIMEOUT
    {
        wire::Record {
            kind: wire::END,
            key: wire::CANCELLED,
            ..Default::default()
        }
        .write(&mut output)
        .map_err(|e| e.to_string())?;
        return Ok(());
    }
    let stop = Arc::new(AtomicBool::new(false));
    let control_stop = stop.clone();
    std::thread::spawn(move || {
        let _ = control.read(&mut [0; 1]);
        control_stop.store(true, Ordering::Release);
    });
    let mut status = wire::COMPLETE;
    let mut held = BTreeSet::new();
    let started = Instant::now();
    if plan
        .iter()
        .filter(|step| step.action != wire::Action::Wait)
        .any(|step| unsafe { GetAsyncKeyState(wire::virtual_key(step.key) as i32) } < 0)
        || [0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0x5b, 0x5c]
            .iter()
            .any(|key| unsafe { GetAsyncKeyState(*key) } < 0)
    {
        status = wire::KEYS_HELD;
    }
    for (index, step) in plan.into_iter().enumerate() {
        if status != wire::COMPLETE {
            break;
        }
        if stopped(&stop, parent.0)
            || started.elapsed() > Duration::from_millis(wire::MAX_MS as u64 + 1000)
        {
            status = wire::CANCELLED;
            break;
        }
        let result = match step.action {
            wire::Action::Wait => Ok(pause(step.delay_ms, &stop, parent.0)),
            wire::Action::Down => key_event(step.key, true, false, &mut held, &mut output)
                .map(|()| pause(wire::GAP_MS, &stop, parent.0)),
            wire::Action::Up => key_event(step.key, false, false, &mut held, &mut output)
                .map(|()| pause(wire::GAP_MS, &stop, parent.0)),
            wire::Action::Tap => {
                key_event(step.key, true, false, &mut held, &mut output).and_then(|()| {
                    if !pause(wire::GAP_MS, &stop, parent.0) {
                        return Ok(false);
                    }
                    key_event(step.key, false, false, &mut held, &mut output)?;
                    Ok(pause(wire::GAP_MS, &stop, parent.0))
                })
            }
        };
        match result {
            Ok(true) => {
                if (wire::Record {
                    kind: wire::STEP_DONE,
                    key: index as u32,
                    ..Default::default()
                })
                .write(&mut output)
                .is_err()
                {
                    status = wire::SEND_FAILED;
                }
            }
            Ok(false) => status = wire::CANCELLED,
            Err(_) => status = wire::SEND_FAILED,
        }
    }
    // Releasing this job's keys is mandatory even when cancellation breaks the pipe.
    for key in held.clone().into_iter().rev() {
        if key_event(key, false, true, &mut held, &mut output).is_err() {
            status = wire::SEND_FAILED;
        }
    }
    wire::Record {
        kind: wire::END,
        key: status,
        ..Default::default()
    }
    .write(&mut output)
    .map_err(|e| e.to_string())?;
    Ok(())
}
