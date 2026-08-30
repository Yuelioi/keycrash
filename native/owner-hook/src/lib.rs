use core::ptr::{read_volatile, write_volatile};
use windows_sys::Win32::Foundation::{CloseHandle, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::Memory::{
    FILE_MAP_ALL_ACCESS, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile,
};
use windows_sys::Win32::System::Threading::{
    EVENT_MODIFY_STATE, GetCurrentProcessId, GetCurrentThreadId, OpenEventW, SetEvent,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, MSG, PM_REMOVE, WM_HOTKEY, WM_NULL,
};

const MAGIC: u32 = 0x4B43_4F50;
const MAPPING_NAME: *const u16 = windows_sys::w!("Local\\KeyCrashOwnerProbeV1");
const EVENT_NAME: *const u16 = windows_sys::w!("Local\\KeyCrashOwnerProbeEventV1");

#[repr(C)]
struct SharedData {
    magic: u32,
    host_pid: u32,
    target_modifiers: u32,
    target_virtual_key: u32,
    sequence: u32,
    owner_pid: u32,
    owner_thread_id: u32,
    suppressed: u32,
}

/// Observes a message removed from an injected process queue.
///
/// # Safety
///
/// Windows calls this function through `SetWindowsHookExW`. A non-negative
/// `code` must provide the documented `WH_GETMESSAGE` `MSG` pointer in
/// `lparam`; callers must not invoke it with an arbitrary pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn keycrash_get_message_hook(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 && wparam as u32 == PM_REMOVE && lparam != 0 {
        let message = lparam as *mut MSG;
        if unsafe { read_volatile(&(*message).message) } == WM_HOTKEY {
            unsafe { record_and_suppress(message) };
        }
    }

    unsafe { CallNextHookEx(core::ptr::null_mut(), code, wparam, lparam) }
}

unsafe fn record_and_suppress(message: *mut MSG) {
    let mapping = unsafe { OpenFileMappingW(FILE_MAP_ALL_ACCESS, 0, MAPPING_NAME) };
    if mapping.is_null() {
        return;
    }

    let view = unsafe {
        MapViewOfFile(
            mapping,
            FILE_MAP_ALL_ACCESS,
            0,
            0,
            core::mem::size_of::<SharedData>(),
        )
    };
    if view.Value.is_null() {
        unsafe { CloseHandle(mapping) };
        return;
    }

    let shared = view.Value.cast::<SharedData>();
    let hotkey = unsafe { read_volatile(&(*message).lParam) } as u32;
    let modifiers = hotkey & 0xFFFF;
    let virtual_key = hotkey >> 16;
    let current_pid = unsafe { GetCurrentProcessId() };
    let matches = unsafe {
        read_volatile(&(*shared).magic) == MAGIC
            && read_volatile(&(*shared).sequence) == 0
            && read_volatile(&(*shared).host_pid) != current_pid
            && read_volatile(&(*shared).target_modifiers) == modifiers
            && read_volatile(&(*shared).target_virtual_key) == virtual_key
    };

    if matches {
        unsafe {
            write_volatile(&mut (*shared).owner_pid, current_pid);
            write_volatile(&mut (*shared).owner_thread_id, GetCurrentThreadId());
            write_volatile(&mut (*shared).suppressed, 1);
            write_volatile(&mut (*shared).sequence, 1);
            write_volatile(&mut (*message).message, WM_NULL);
        }

        let event = unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, EVENT_NAME) };
        if !event.is_null() {
            unsafe {
                SetEvent(event);
                CloseHandle(event);
            }
        }
    }

    unsafe {
        UnmapViewOfFile(view);
        CloseHandle(mapping);
    }
}
