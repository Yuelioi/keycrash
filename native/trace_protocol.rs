//! Shared POD layout for passive, bounded professional message observations.
#![allow(dead_code)] // Each of the three consumers uses a different subset of the contract.
use std::sync::atomic::AtomicU32;

pub const NAME: *const u16 = windows_sys::w!("Local\\KeyCrashProfessionalTraceV3");
pub const MAGIC: u32 = 0x4B43_5433;
pub const PREPARING: u32 = 2;
pub const CAPACITY: usize = 256;
pub const HOST64: u32 = 1;
pub const HOST32: u32 = 2;
pub const HOTKEY: u32 = 1;
pub const GD_NOTICE: u32 = 2;

pub const START_PARENT: u32 = 1;
pub const START_DLL: u32 = 2;
pub const START_EXPORT: u32 = 3;
pub const START_HOOK: u32 = 4;
pub const START_CHILD_EXIT: u32 = 5;

// Startup diagnostics fit the existing error word; the V3 layout is unchanged.
// Win32 system error codes occupy the low 16 bits, as in HRESULT_FROM_WIN32.
pub fn startup_failure(stage: u32, code: u32) -> u32 {
    (stage << 24) | (u32::from(usize::BITS == 32) << 23) | (code & 0xffff)
}

pub fn failure_detail(error: u32) -> Option<String> {
    if error >> 24 == START_CHILD_EXIT {
        return Some("x86 / observer exited".into());
    }
    let stage = match error >> 24 {
        START_PARENT => "OpenProcess",
        START_DLL => "LoadLibraryW",
        START_EXPORT => "GetProcAddress",
        START_HOOK => "SetWindowsHookExW",
        _ => return None,
    };
    let architecture = if error & (1 << 23) != 0 { "x86" } else { "x64" };
    Some(format!(
        "{architecture} / {stage} / Windows {}",
        error & 0xffff
    ))
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Binding {
    pub key1: u32,
    pub key2: u32,
    pub modifiers: u32,
}

#[repr(C)]
pub struct Slot {
    pub ready: AtomicU32,
    pub pid: u32,
    pub tid: u32,
    pub kind: u32,
    pub value: u32,
    pub tick: u32,
    pub architecture: u32,
    pub created_low: u32,
    pub created_high: u32,
    pub module_low: u32,
    pub module_high: u32,
    pub binding_profile: u32,
    pub bindings: [Binding; 5],
    pub path_len: u32,
    pub path: [u16; 512],
}

#[repr(C)]
pub struct Trace {
    pub magic: u32,
    pub owner_pid: u32,
    pub helper_pid: AtomicU32,
    pub host64_pid: AtomicU32,
    pub active: AtomicU32,
    pub ready: AtomicU32,
    pub error: AtomicU32,
    pub count: AtomicU32,
    pub overflow: AtomicU32,
    pub started: u32,
    pub slots: [Slot; CAPACITY],
}
