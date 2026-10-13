//! Bounded, explicitly requested active attribution. Observe only the marked
//! system-defined hotkey event, never ordinary key events or foreground PIDs.
use crate::detector::Shortcut;
use objc2::encode::{Encoding, RefEncode};
use objc2::{class, msg_send, rc::autoreleasepool, runtime::AnyObject};
use std::collections::BTreeSet;
use std::ffi::{CStr, c_char, c_void};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

type Ref = *mut c_void;
type TapCallback = unsafe extern "C" fn(Ref, u32, Ref, Ref) -> Ref;

// Preserve the Objective-C type encoding of the opaque Quartz event pointer.
#[repr(C)]
struct QuartzEvent {
    _opaque: [u8; 0],
}
unsafe impl RefEncode for QuartzEvent {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__CGEvent", &[]));
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGRequestPostEventAccess() -> bool;
    fn CGEventTapCreate(
        location: u32,
        place: u32,
        options: u32,
        mask: u64,
        callback: TapCallback,
        context: Ref,
    ) -> Ref;
    fn CGEventGetIntegerValueField(event: Ref, field: u32) -> i64;
    fn CGEventSetIntegerValueField(event: Ref, field: u32, value: i64);
    fn CGEventSourceCreate(state: i32) -> Ref;
    fn CGEventSourceKeyState(state: i32, key: u16) -> bool;
    fn CGEventCreateKeyboardEvent(source: Ref, key: u16, down: bool) -> Ref;
    fn CGEventSetFlags(event: Ref, flags: u64);
    fn CGEventPost(location: u32, event: Ref);
}
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFRunLoopDefaultMode: Ref;
    fn CFRelease(value: *const c_void);
    fn CFMachPortCreateRunLoopSource(allocator: Ref, port: Ref, order: isize) -> Ref;
    fn CFMachPortInvalidate(port: Ref);
    fn CFRunLoopGetCurrent() -> Ref;
    fn CFRunLoopAddSource(run_loop: Ref, source: Ref, mode: Ref);
    fn CFRunLoopRemoveSource(run_loop: Ref, source: Ref, mode: Ref);
    fn CFRunLoopRunInMode(mode: Ref, seconds: f64, return_after_source: bool) -> i32;
}
#[link(name = "proc")]
unsafe extern "C" {
    fn proc_pidpath(pid: i32, buffer: Ref, size: u32) -> i32;
}

#[derive(Debug)]
pub struct Owner {
    pub pid: u32,
    pub path: String,
}
#[derive(Debug)]
pub enum OwnerProbeResult {
    Found(Vec<Owner>),
    NotFound,
    PermissionRequired,
    Cancelled,
    Error(&'static str),
}

pub fn has_permission() -> bool {
    // CGPreflightPostEventAccess caches an initial denial on macOS 15.5 even
    // after the user grants Accessibility. AX reflects the live grant; the
    // short-lived event tap below still verifies access before any input.
    unsafe { AXIsProcessTrusted() }
}
// Call on the UI thread. Requesting permission never sends a shortcut.
pub fn request_permission() {
    unsafe {
        CGRequestPostEventAccess();
    }
}

struct Owned(Ref);
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) };
    }
}

struct Observation {
    marker: i64,
    pids: BTreeSet<u32>,
    interrupted: bool,
}

unsafe extern "C" fn observe(_: Ref, kind: u32, event: Ref, context: Ref) -> Ref {
    // Callback and observation are confined to this worker's run loop.
    let state = unsafe { &mut *context.cast::<Observation>() };
    if kind == u32::MAX || kind == u32::MAX - 1 {
        state.interrupted = true;
        return event;
    }
    // 14 = NX_SYSDEFINED; 42 = kCGEventSourceUserData; 40 = target Unix PID.
    if kind != 14
        || event.is_null()
        || unsafe { CGEventGetIntegerValueField(event, 42) } != state.marker
    {
        return event;
    }
    autoreleasepool(|_| unsafe {
        let native: *mut AnyObject =
            msg_send![class!(NSEvent), eventWithCGEvent: event.cast::<QuartzEvent>()];
        if !native.is_null() {
            let subtype: i16 = msg_send![native, subtype];
            // Hotkey-down (6) is distinct from key-down and hotkey-up (9).
            if subtype == 6 {
                let pid = CGEventGetIntegerValueField(event, 40);
                if pid > 0 && pid <= i64::from(i32::MAX) && pid != i64::from(std::process::id()) {
                    state.pids.insert(pid as u32);
                }
            }
        }
    });
    event
}

pub(crate) fn event_flags(shortcut: &Shortcut) -> u64 {
    let mut flags = 0;
    for (carbon, quartz) in [(12, 18), (11, 19), (9, 17), (8, 20)] {
        if shortcut.modifiers & (1 << carbon) != 0 {
            flags |= 1 << quartz;
        }
    }
    if crate::mac_keys::is_function_key(shortcut.key_code)
        || matches!(shortcut.key_code, 0x73..=0x77 | 0x79 | 0x7b..=0x7e)
    {
        flags |= 1 << 23; // SecondaryFn, also set by Carbon for navigation keys.
    }
    flags
}

pub fn locate_owner(shortcut: &Shortcut, cancelled: &AtomicBool) -> OwnerProbeResult {
    if cancelled.load(Ordering::Relaxed) {
        return OwnerProbeResult::Cancelled;
    }
    if !has_permission() {
        return OwnerProbeResult::PermissionRequired;
    }
    unsafe {
        // Do not combine synthetic input with keys the user is still holding.
        if [54, 55, 56, 58, 59, 60, 61, 62, shortcut.key_code]
            .iter()
            .any(|&key| CGEventSourceKeyState(1, key))
        {
            return OwnerProbeResult::Error("请松开键盘上的按键后再定位。");
        }
        static SEQUENCE: AtomicU64 = AtomicU64::new(1);
        let marker = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
            ^ (u64::from(std::process::id()) << 32)
            ^ SEQUENCE.fetch_add(1, Ordering::Relaxed)) as i64;
        let mut observation = Observation {
            marker,
            pids: BTreeSet::new(),
            interrupted: false,
        };
        // Listen-only, after WindowServer routing. Never alter/suppress events.
        let tap = CGEventTapCreate(
            2,
            1,
            1,
            1 << 14,
            observe,
            (&mut observation as *mut Observation).cast(),
        );
        if tap.is_null() {
            return OwnerProbeResult::Error("无法观察快捷键事件，请检查辅助功能权限。");
        }
        let tap = Owned(tap);
        let source = CFMachPortCreateRunLoopSource(ptr::null_mut(), tap.0, 0);
        if source.is_null() {
            CFMachPortInvalidate(tap.0);
            return OwnerProbeResult::Error("无法创建快捷键观察器。");
        }
        let source = Owned(source);
        let run_loop = CFRunLoopGetCurrent();
        CFRunLoopAddSource(run_loop, source.0, kCFRunLoopDefaultMode);
        let result = (|| {
            let input = CGEventSourceCreate(-1);
            if input.is_null() {
                return OwnerProbeResult::Error("无法创建快捷键输入。");
            }
            let input = Owned(input);
            let down = CGEventCreateKeyboardEvent(input.0, shortcut.key_code, true);
            let up = CGEventCreateKeyboardEvent(input.0, shortcut.key_code, false);
            if down.is_null() || up.is_null() {
                if !down.is_null() {
                    CFRelease(down);
                }
                if !up.is_null() {
                    CFRelease(up);
                }
                return OwnerProbeResult::Error("无法创建快捷键输入。");
            }
            let down = Owned(down);
            let up = Owned(up);
            for event in [&down, &up] {
                CGEventSetFlags(event.0, event_flags(shortcut));
                CGEventSetIntegerValueField(event.0, 42, marker);
            }
            // Restore neutral modifiers; no real modifier was held above.
            CGEventSetFlags(up.0, 0);
            if cancelled.load(Ordering::Relaxed) {
                return OwnerProbeResult::Cancelled;
            }
            CGEventPost(0, down.0);
            std::thread::sleep(Duration::from_millis(20));
            // Always pair a posted down with up, including cancellation.
            CGEventPost(0, up.0);
            let deadline = Instant::now() + Duration::from_millis(900);
            while Instant::now() < deadline && !cancelled.load(Ordering::Relaxed) {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.025, false);
            }
            if cancelled.load(Ordering::Relaxed) {
                return OwnerProbeResult::Cancelled;
            }
            if observation.interrupted {
                return OwnerProbeResult::Error("事件观察中断，请重试。");
            }
            if observation.pids.is_empty() {
                return OwnerProbeResult::NotFound;
            }
            OwnerProbeResult::Found(
                observation
                    .pids
                    .iter()
                    .map(|&pid| {
                        let mut path = [0 as c_char; 4096];
                        let length =
                            proc_pidpath(pid as i32, path.as_mut_ptr().cast(), path.len() as u32);
                        let path = if length > 0 {
                            CStr::from_ptr(path.as_ptr()).to_string_lossy().into_owned()
                        } else {
                            String::new()
                        };
                        Owner { pid, path }
                    })
                    .collect(),
            )
        })();
        CFRunLoopRemoveSource(run_loop, source.0, kCFRunLoopDefaultMode);
        CFMachPortInvalidate(tap.0);
        result
    }
}
