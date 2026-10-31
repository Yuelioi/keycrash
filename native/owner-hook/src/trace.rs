use core::mem::size_of;
use std::sync::atomic::Ordering;
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, FreeLibrary, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleFileNameW, GetModuleHandleExW, GetProcAddress,
};
use windows_sys::Win32::System::Memory::{
    FILE_MAP_ALL_ACCESS, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId, GetProcessTimes,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetWindowThreadProcessId, MSG, PM_REMOVE, WM_APP, WM_HOTKEY,
};

use crate::trace_protocol as protocol;

struct Module(HMODULE);
impl Drop for Module {
    fn drop(&mut self) {
        unsafe { FreeLibrary(self.0) };
    }
}

/// Observes supported notifications without rewriting or suppressing them.
///
/// # Safety
/// Called only by Windows as a WH_GETMESSAGE hook with the documented MSG pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn keycrash_trace_hook(
    code: i32,
    removed: usize,
    lparam: isize,
) -> isize {
    if code >= 0 && removed as u32 == PM_REMOVE && lparam != 0 {
        let message = unsafe { &*(lparam as *const MSG) };
        if message.message == WM_HOTKEY
            || (message.message == WM_APP + 1 && message.lParam == 0 && message.wParam < 5)
        {
            unsafe { record(message) };
        }
    }
    unsafe { CallNextHookEx(core::ptr::null_mut(), code, removed, lparam) }
}

unsafe fn record(message: &MSG) {
    let pid = unsafe { GetCurrentProcessId() };
    let tid = unsafe { GetCurrentThreadId() };
    // A mismatched-bitness hook can run in a host. Never assign that host as owner.
    if !message.hwnd.is_null() {
        let mut window_pid = 0;
        let window_tid = unsafe { GetWindowThreadProcessId(message.hwnd, &mut window_pid) };
        if window_pid != pid || window_tid != tid {
            return;
        }
    }
    let vendor = if message.message == WM_HOTKEY {
        None
    } else {
        let mut module = core::ptr::null_mut();
        if unsafe { GetModuleHandleExW(0, windows_sys::w!("GdHotkeys.dll"), &mut module) } == 0 {
            return;
        }
        let module = Module(module);
        // Identify the documented vendor protocol, not arbitrary WM_APP+1 traffic.
        for name in [c"setHook", c"removeHook", c"setHotkeys", c"clearHotkeys"] {
            if unsafe { GetProcAddress(module.0, name.as_ptr().cast()) }.is_none() {
                return;
            }
        }
        Some(module)
    };
    let kind = if vendor.is_some() {
        protocol::GD_NOTICE
    } else {
        protocol::HOTKEY
    };
    let mapping = unsafe { OpenFileMappingW(FILE_MAP_ALL_ACCESS, 0, protocol::NAME) };
    if mapping.is_null() {
        return;
    }
    let view = unsafe {
        MapViewOfFile(
            mapping,
            FILE_MAP_ALL_ACCESS,
            0,
            0,
            size_of::<protocol::Trace>(),
        )
    };
    if !view.Value.is_null() {
        let trace = view.Value.cast::<protocol::Trace>();
        let valid = unsafe {
            (*trace).magic == protocol::MAGIC
                && (*trace).active.load(Ordering::Acquire) == 1
                && (*trace).owner_pid != pid
                && (*trace).helper_pid.load(Ordering::Acquire) != pid
                && (*trace).host64_pid.load(Ordering::Acquire) != pid
                && message.time.wrapping_sub((*trace).started) <= 60_000
        };
        if valid {
            let index = unsafe { (*trace).count.fetch_add(1, Ordering::Relaxed) } as usize;
            if index < protocol::CAPACITY {
                let slot = unsafe { core::ptr::addr_of_mut!((*trace).slots[index]) };
                unsafe {
                    (*slot).pid = pid;
                    (*slot).tid = tid;
                    (*slot).kind = kind;
                    (*slot).value = if kind == protocol::HOTKEY {
                        message.lParam as u32
                    } else {
                        message.wParam as u32
                    };
                    (*slot).tick = message.time;
                    (*slot).architecture = usize::BITS;
                    let mut created = FILETIME::default();
                    let mut exit = FILETIME::default();
                    let mut kernel = FILETIME::default();
                    let mut user = FILETIME::default();
                    if GetProcessTimes(
                        GetCurrentProcess(),
                        &mut created,
                        &mut exit,
                        &mut kernel,
                        &mut user,
                    ) != 0
                    {
                        (*slot).created_low = created.dwLowDateTime;
                        (*slot).created_high = created.dwHighDateTime;
                    }
                    if let Some(module) = &vendor {
                        let base = module.0 as usize as u64;
                        (*slot).module_low = base as u32;
                        (*slot).module_high = (base >> 32) as u32;
                        if let Some((profile, bindings)) = crate::goldendict::snapshot(module.0) {
                            (*slot).binding_profile = profile;
                            (*slot).bindings = bindings;
                        }
                    }
                    let length = GetModuleFileNameW(
                        core::ptr::null_mut(),
                        core::ptr::addr_of_mut!((*slot).path).cast(),
                        512,
                    );
                    (*slot).path_len = if length < 512 { length } else { 0 };
                    (*slot).ready.store(1, Ordering::Release);
                }
            } else {
                unsafe { (*trace).overflow.store(1, Ordering::Release) };
            }
        }
        unsafe { UnmapViewOfFile(view) };
    }
    unsafe { CloseHandle(mapping) };
}
