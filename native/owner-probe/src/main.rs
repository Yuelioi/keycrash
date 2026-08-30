use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::thread;
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    CloseHandle, FreeLibrary, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, FILE_MAP_ALL_ACCESS, MapViewOfFile, PAGE_READWRITE, UnmapViewOfFile,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, GetCurrentProcessId, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW, WaitForSingleObject,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, MOD_ALT,
    MOD_CONTROL, MOD_SHIFT, SendInput, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    HOOKPROC, SetWindowsHookExW, UnhookWindowsHookEx, WH_GETMESSAGE,
};

const MAGIC: u32 = 0x4B43_4F50;
const MAPPING_NAME: *const u16 = windows_sys::w!("Local\\KeyCrashOwnerProbeV1");
const EVENT_NAME: *const u16 = windows_sys::w!("Local\\KeyCrashOwnerProbeEventV1");

#[repr(C)]
#[derive(Clone, Copy)]
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

fn main() {
    match parse_arguments()
        .and_then(|(modifiers, virtual_key)| unsafe { locate_owner(modifiers, virtual_key) })
    {
        Ok(Some(owner)) => println!(
            "FOUND\t{}\t{}\t{}\t{}",
            owner.owner_pid,
            owner.owner_thread_id,
            owner.suppressed,
            process_path(owner.owner_pid).unwrap_or_default()
        ),
        Ok(None) => println!("NOT_FOUND"),
        Err(error) => {
            println!("ERROR\t{}", error.replace(['\t', '\r', '\n'], " "));
            std::process::exit(1);
        }
    }
}

fn parse_arguments() -> Result<(u32, u32), String> {
    let mut arguments = std::env::args().skip(1);
    let modifiers = arguments
        .next()
        .ok_or("missing modifiers")?
        .parse()
        .map_err(|_| "invalid modifiers")?;
    let virtual_key = arguments
        .next()
        .ok_or("missing virtual key")?
        .parse()
        .map_err(|_| "invalid virtual key")?;
    Ok((modifiers, virtual_key))
}

unsafe fn locate_owner(modifiers: u32, virtual_key: u32) -> Result<Option<SharedData>, String> {
    if modifiers_physically_held() {
        return Err("release Ctrl, Alt, and Shift before locating the owner".into());
    }

    let mapping = unsafe {
        CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            core::ptr::null(),
            PAGE_READWRITE,
            0,
            size_of::<SharedData>() as u32,
            MAPPING_NAME,
        )
    };
    if mapping.is_null() {
        return Err("CreateFileMappingW failed".into());
    }

    let view =
        unsafe { MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, size_of::<SharedData>()) };
    if view.Value.is_null() {
        unsafe { CloseHandle(mapping) };
        return Err("MapViewOfFile failed".into());
    }

    let shared = view.Value.cast::<SharedData>();
    unsafe {
        shared.write(SharedData {
            magic: MAGIC,
            host_pid: GetCurrentProcessId(),
            target_modifiers: modifiers,
            target_virtual_key: virtual_key,
            sequence: 0,
            owner_pid: 0,
            owner_thread_id: 0,
            suppressed: 0,
        });
    }

    let event = unsafe { CreateEventW(core::ptr::null(), 1, 0, EVENT_NAME) };
    if event.is_null() {
        cleanup_mapping(view, mapping);
        return Err("CreateEventW failed".into());
    }

    let dll_path = sibling("keycrash_owner_hook.dll")?;
    let dll_wide: Vec<u16> = dll_path.as_os_str().encode_wide().chain(Some(0)).collect();
    let module = unsafe { LoadLibraryW(dll_wide.as_ptr()) };
    if module.is_null() {
        unsafe { CloseHandle(event) };
        cleanup_mapping(view, mapping);
        return Err(format!("cannot load {}", dll_path.display()));
    }

    let procedure = unsafe { GetProcAddress(module, c"keycrash_get_message_hook".as_ptr().cast()) };
    let procedure: HOOKPROC = unsafe { core::mem::transmute(procedure) };
    let hook = unsafe { SetWindowsHookExW(WH_GETMESSAGE, procedure, module, 0) };
    if hook.is_null() {
        unsafe {
            FreeLibrary(module);
            CloseHandle(event);
        }
        cleanup_mapping(view, mapping);
        return Err("SetWindowsHookExW failed".into());
    }

    thread::sleep(Duration::from_millis(120));
    let send_result = send_shortcut(modifiers, virtual_key);
    let wait_result = if send_result.is_ok() {
        unsafe { WaitForSingleObject(event, 1_500) }
    } else {
        u32::MAX
    };
    let observed = unsafe { shared.read() };

    unsafe {
        UnhookWindowsHookEx(hook);
        FreeLibrary(module);
        CloseHandle(event);
    }
    cleanup_mapping(view, mapping);
    send_result?;

    if wait_result == WAIT_OBJECT_0 && observed.sequence != 0 {
        Ok(Some(observed))
    } else {
        Ok(None)
    }
}

fn modifiers_physically_held() -> bool {
    [VK_CONTROL, VK_MENU, VK_SHIFT]
        .iter()
        .any(|key| unsafe { GetAsyncKeyState(*key as i32) } < 0)
}

fn send_shortcut(modifiers: u32, virtual_key: u32) -> Result<(), String> {
    let mut keys = Vec::with_capacity(8);
    if modifiers & MOD_CONTROL != 0 {
        keys.push(key_input(VK_CONTROL, false));
    }
    if modifiers & MOD_ALT != 0 {
        keys.push(key_input(VK_MENU, false));
    }
    if modifiers & MOD_SHIFT != 0 {
        keys.push(key_input(VK_SHIFT, false));
    }
    keys.push(key_input(virtual_key as u16, false));
    keys.push(key_input(virtual_key as u16, true));
    if modifiers & MOD_SHIFT != 0 {
        keys.push(key_input(VK_SHIFT, true));
    }
    if modifiers & MOD_ALT != 0 {
        keys.push(key_input(VK_MENU, true));
    }
    if modifiers & MOD_CONTROL != 0 {
        keys.push(key_input(VK_CONTROL, true));
    }

    let sent = unsafe { SendInput(keys.len() as u32, keys.as_ptr(), size_of::<INPUT>() as i32) };
    if sent as usize == keys.len() {
        Ok(())
    } else {
        Err(format!("SendInput sent {sent} of {} events", keys.len()))
    }
}

fn key_input(virtual_key: u16, key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: virtual_key,
                wScan: 0,
                dwFlags: if key_up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn process_path(pid: u32) -> Option<String> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return None;
    }

    let mut buffer = vec![0u16; 32_768];
    let mut length = buffer.len() as u32;
    let success =
        unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) };
    unsafe { CloseHandle(process) };
    (success != 0).then(|| String::from_utf16_lossy(&buffer[..length as usize]))
}

fn sibling(name: &str) -> Result<std::path::PathBuf, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    Ok(executable
        .parent()
        .ok_or("helper has no parent directory")?
        .join(name))
}

fn cleanup_mapping(
    view: windows_sys::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS,
    mapping: windows_sys::Win32::Foundation::HANDLE,
) {
    unsafe {
        UnmapViewOfFile(view);
        CloseHandle(mapping);
    }
}
