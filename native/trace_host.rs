//! Shared host used by the GUI worker and the matching x86 helper.
use std::mem::size_of;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{CloseHandle, FreeLibrary, GetLastError, WAIT_OBJECT_0};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Memory::{
    FILE_MAP_ALL_ACCESS, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, HOOKPROC, MSG, PM_REMOVE, PeekMessageW, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, WH_GETMESSAGE,
};

use crate::trace_protocol as protocol;

pub fn run(directory: &Path) -> Result<(), String> {
    let mapping = unsafe { OpenFileMappingW(FILE_MAP_ALL_ACCESS, 0, protocol::NAME) };
    if mapping.is_null() {
        return Err("Professional trace mapping is unavailable".into());
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
    if view.Value.is_null() {
        unsafe { CloseHandle(mapping) };
        return Err("Cannot open professional trace".into());
    }
    let trace = view.Value.cast::<protocol::Trace>();
    let result = unsafe { host(directory, trace) };
    unsafe {
        UnmapViewOfFile(view);
        CloseHandle(mapping);
    }
    result
}

/// Supervises both bitnesses in the privilege context chosen by the GUI.
#[allow(dead_code)] // The GUI uses run; this entry belongs to the helper binary.
pub fn run_all(directory: &Path) -> Result<(), String> {
    let mut child = Command::new(directory.join("owner-x86/keycrash-owner-probe.exe"))
        .arg("--professional-trace")
        .creation_flags(0x0800_0000)
        .spawn()
        .map_err(|e| e.to_string())?;
    let stop = Arc::new(AtomicBool::new(false));
    let monitor_stop = stop.clone();
    let monitor = std::thread::spawn(move || {
        let mapping = unsafe { OpenFileMappingW(FILE_MAP_ALL_ACCESS, 0, protocol::NAME) };
        let view = if mapping.is_null() {
            None
        } else {
            let view = unsafe {
                MapViewOfFile(
                    mapping,
                    FILE_MAP_ALL_ACCESS,
                    0,
                    0,
                    size_of::<protocol::Trace>(),
                )
            };
            if view.Value.is_null() {
                None
            } else {
                Some(view)
            }
        };
        let mut failed = view.is_none();
        if let Some(view) = view {
            let trace = unsafe { &*view.Value.cast::<protocol::Trace>() };
            trace.helper_pid.store(child.id(), Ordering::Release);
            while !monitor_stop.load(Ordering::Acquire) && trace.active.load(Ordering::Acquire) != 0
            {
                let status = child.try_wait();
                if !matches!(status, Ok(None)) {
                    failed = true;
                    let _ = trace.error.compare_exchange(
                        0,
                        protocol::startup_failure(protocol::START_CHILD_EXIT, 0),
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    );
                    trace.active.store(0, Ordering::Release);
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            unsafe { UnmapViewOfFile(view) };
        }
        if !mapping.is_null() {
            unsafe { CloseHandle(mapping) };
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline && matches!(child.try_wait(), Ok(None)) {
            std::thread::sleep(Duration::from_millis(5));
        }
        if !matches!(child.try_wait(), Ok(Some(_))) {
            let _ = child.kill();
        }
        let _ = child.wait();
        if failed {
            Err("The x86 response observer exited unexpectedly".to_string())
        } else {
            Ok(())
        }
    });
    let result = run(directory);
    stop.store(true, Ordering::Release);
    let monitored = monitor
        .join()
        .map_err(|_| "Response supervisor stopped".to_string())?;
    result.and(monitored)
}

unsafe fn host(directory: &Path, trace: *mut protocol::Trace) -> Result<(), String> {
    if unsafe { (*trace).magic } != protocol::MAGIC {
        return Err("Unknown trace protocol".into());
    }
    if usize::BITS == 64 {
        unsafe {
            (*trace)
                .host64_pid
                .store(GetCurrentProcessId(), Ordering::Release)
        };
    } else {
        unsafe {
            (*trace)
                .helper_pid
                .store(GetCurrentProcessId(), Ordering::Release)
        };
    }
    let owner = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, (*trace).owner_pid) };
    if owner.is_null() {
        unsafe { startup_failed(trace, protocol::START_PARENT, GetLastError()) };
        return Err("Cannot monitor the recording process".into());
    }
    let dll = if usize::BITS == 64 {
        "keycrash_trace64_v3.dll"
    } else {
        "keycrash_trace32_v3.dll"
    };
    let path: Vec<u16> = directory
        .join(dll)
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let module = unsafe { LoadLibraryW(path.as_ptr()) };
    if module.is_null() {
        unsafe { startup_failed(trace, protocol::START_DLL, GetLastError()) };
        unsafe { CloseHandle(owner) };
        return Err(format!("Cannot load {dll}"));
    }
    let procedure = unsafe { GetProcAddress(module, c"keycrash_trace_hook".as_ptr().cast()) };
    if procedure.is_none() {
        unsafe {
            startup_failed(trace, protocol::START_EXPORT, GetLastError());
            FreeLibrary(module);
            CloseHandle(owner);
        }
        return Err("Cannot find passive message observer".into());
    }
    let procedure: HOOKPROC = unsafe { std::mem::transmute(procedure) };
    let hook = unsafe { SetWindowsHookExW(WH_GETMESSAGE, procedure, module, 0) };
    if hook.is_null() {
        unsafe {
            startup_failed(trace, protocol::START_HOOK, GetLastError());
            CloseHandle(owner);
            FreeLibrary(module);
        }
        return Err("Cannot install passive message observer".into());
    }
    let bit = if usize::BITS == 64 {
        protocol::HOST64
    } else {
        protocol::HOST32
    };
    unsafe { (*trace).ready.fetch_or(bit, Ordering::Release) };
    let deadline = Instant::now() + Duration::from_secs(70);
    let mut message = MSG::default();
    while unsafe { (*trace).active.load(Ordering::Acquire) } != 0
        && Instant::now() < deadline
        && unsafe { WaitForSingleObject(owner, 0) } != WAIT_OBJECT_0
    {
        while unsafe { PeekMessageW(&mut message, core::ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
            if unsafe { (*trace).active.load(Ordering::Acquire) } == 0 {
                break;
            }
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    if unsafe { (*trace).active.load(Ordering::Acquire) } != 0 {
        unsafe { (*trace).error.store(1, Ordering::Release) };
    }
    unsafe {
        UnhookWindowsHookEx(hook);
        FreeLibrary(module);
        CloseHandle(owner);
    }
    Ok(())
}

unsafe fn startup_failed(trace: *mut protocol::Trace, stage: u32, code: u32) {
    let _ = unsafe { &(*trace).error }.compare_exchange(
        0,
        protocol::startup_failure(stage, code),
        Ordering::AcqRel,
        Ordering::Acquire,
    );
}
