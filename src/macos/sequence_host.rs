//! Self-hosted input executor; control EOF and parent death release this job's keys.
use crate::sequence_protocol as wire;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::c_void,
    io::{Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
type Ref = *mut c_void;
unsafe extern "C" {
    fn kc_sequence_parent(pid: u32) -> bool;
    fn kc_sequence_source() -> Ref;
    fn kc_sequence_event(source: Ref, key: u32, down: bool) -> Ref;
    fn kc_sequence_post(event: Ref, flags: u64, marker: i64, key: u32, down: bool);
    fn kc_sequence_release(value: Ref);
    fn kc_sequence_clock() -> u32;
    fn kc_sequence_flags() -> u64;
    fn kc_sequence_held(key: u32) -> bool;
}
struct Owned(Ref);
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe {
            kc_sequence_release(self.0);
        }
    }
}
struct Input {
    _source: Owned,
    events: BTreeMap<u32, (Owned, Owned)>,
    held: BTreeSet<u32>,
    caps: u64,
    marker: i64,
    posted: bool,
}
impl Input {
    fn new(plan: &[wire::Step], marker: i64) -> Result<Self, String> {
        let source = unsafe { kc_sequence_source() };
        if source.is_null() {
            return Err("Cannot create input source".into());
        }
        let mut input = Self {
            _source: Owned(source),
            events: BTreeMap::new(),
            held: BTreeSet::new(),
            caps: unsafe { kc_sequence_flags() } & (1 << 16),
            marker,
            posted: false,
        };
        for key in plan
            .iter()
            .filter(|s| s.action != wire::Action::Wait)
            .map(|s| s.key)
            .collect::<BTreeSet<_>>()
        {
            let down = unsafe { kc_sequence_event(source, key, true) };
            let up = unsafe { kc_sequence_event(source, key, false) };
            let pair = (Owned(down), Owned(up));
            if down.is_null() || up.is_null() {
                return Err("Cannot create key events".into());
            }
            input.events.insert(key, pair);
        }
        Ok(input)
    }
    fn send(
        &mut self,
        key: u32,
        down: bool,
        cleanup: bool,
        output: &mut impl Write,
    ) -> Result<(), String> {
        if down {
            self.held.insert(key);
            if key == 57 {
                self.caps ^= 1 << 16;
            }
        } else {
            self.held.remove(&key);
        }
        let mut flags = self
            .held
            .iter()
            .fold(self.caps, |bits, key| bits | wire::event_modifier(*key));
        if key < 128
            && (crate::mac_keys::is_function_key(key as u16)
                || matches!(key,0x73..=0x77|0x79|0x7b..=0x7e))
        {
            flags |= 1 << 23;
        }
        if matches!(key,65|67|69|71|75|76|78|81..=89|91|92|95|123..=126) {
            flags |= 1 << 21;
        }
        if key == 114 {
            flags |= 1 << 22;
        }
        let events = self.events.get(&key).ok_or("Unprepared key")?;
        let event = if down { events.0.0 } else { events.1.0 };
        let tick = unsafe { kc_sequence_clock() };
        unsafe {
            kc_sequence_post(event, flags, self.marker, key, down);
        }
        self.posted = true;
        wire::Record {
            kind: wire::SENT,
            key,
            flags: if down { wire::DOWN } else { 0 } | if cleanup { wire::CLEANUP } else { 0 },
            tick,
            observed: unsafe { kc_sequence_clock() },
        }
        .write(output)
        .map_err(|e| e.to_string())
    }
}
fn stopped(cancel: &AtomicBool, parent: u32) -> bool {
    cancel.load(Ordering::Acquire) || !unsafe { kc_sequence_parent(parent) }
}
fn pause(ms: u32, cancel: &AtomicBool, parent: u32) -> bool {
    let end = Instant::now() + Duration::from_millis(ms.into());
    while Instant::now() < end {
        if stopped(cancel, parent) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    !stopped(cancel, parent)
}
pub fn entry() -> Option<i32> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("--professional-sequence") {
        return None;
    }
    let result = (|| {
        let parent = args
            .get(2)
            .and_then(|v| v.parse().ok())
            .ok_or("Missing sequence parent")?;
        let marker = args
            .get(3)
            .and_then(|v| v.parse().ok())
            .ok_or("Missing sequence marker")?;
        run(parent, marker)
    })();
    Some(if result.is_ok() { 0 } else { 1 })
}
fn run(parent: u32, marker: i64) -> Result<(), String> {
    if !unsafe { kc_sequence_parent(parent) } {
        return Err("Sequence parent mismatch".into());
    }
    let mut control = std::io::stdin();
    let plan = wire::read_plan(&mut control)?;
    let mut input = Input::new(&plan, marker)?;
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
    let cancel = Arc::new(AtomicBool::new(false));
    let mut status = if control.read_exact(&mut go).is_err()
        || go[0] != 1
        || !unsafe { kc_sequence_parent(parent) }
    {
        wire::CANCELLED
    } else {
        wire::COMPLETE
    };
    let stop = cancel.clone();
    std::thread::spawn(move || {
        let _ = control.read(&mut [0]);
        stop.store(true, Ordering::Release);
    });
    if status == wire::COMPLETE {
        input.caps = unsafe { kc_sequence_flags() } & (1 << 16);
        if !crate::owner_probe::has_permission() {
            status = wire::PERMISSION_REQUIRED;
        } else if plan
            .iter()
            .filter(|s| s.action != wire::Action::Wait && s.key != 57)
            .any(|s| unsafe { kc_sequence_held(s.key) })
            || [54, 55, 56, 58, 59, 60, 61, 62, 63]
                .iter()
                .any(|key| unsafe { kc_sequence_held(*key) })
        {
            status = wire::KEYS_HELD;
        }
    }
    let started = Instant::now();
    for (index, step) in plan.iter().enumerate() {
        if status != wire::COMPLETE {
            break;
        }
        if !crate::owner_probe::has_permission() {
            status = wire::PERMISSION_REQUIRED;
            break;
        }
        if stopped(&cancel, parent)
            || started.elapsed() > Duration::from_millis((wire::MAX_MS + 1000).into())
        {
            status = wire::CANCELLED;
            break;
        }
        let result = match step.action {
            wire::Action::Wait => Ok(pause(step.delay_ms, &cancel, parent)),
            wire::Action::Down | wire::Action::Up => input
                .send(
                    step.key,
                    step.action == wire::Action::Down,
                    false,
                    &mut output,
                )
                .map(|()| pause(wire::GAP_MS, &cancel, parent)),
            wire::Action::Tap => input
                .send(step.key, true, false, &mut output)
                .and_then(|()| {
                    if !pause(wire::GAP_MS, &cancel, parent) {
                        return Ok(false);
                    }
                    input.send(step.key, false, false, &mut output)?;
                    Ok(pause(wire::GAP_MS, &cancel, parent))
                }),
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
    for key in input.held.clone().into_iter().rev() {
        if input.send(key, false, true, &mut output).is_err() {
            status = wire::SEND_FAILED;
        }
    }
    if input.posted && !crate::owner_probe::has_permission() {
        status = wire::RELEASE_UNCONFIRMED;
    }
    wire::Record {
        kind: wire::END,
        key: status,
        ..Default::default()
    }
    .write(&mut output)
    .map_err(|e| e.to_string())
}
