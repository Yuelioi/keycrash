use std::mem::size_of;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, FILE_MAP_ALL_ACCESS, MapViewOfFile, PAGE_READWRITE, UnmapViewOfFile,
};
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcessToken, TerminateProcess, WaitForSingleObject,
};

#[path = "../../native/trace_host.rs"]
mod host;
use crate::trace_protocol as protocol;

pub struct MessageTrace {
    mapping: HANDLE,
    view: *mut protocol::Trace,
    worker: Option<JoinHandle<Result<(), String>>>,
    helper: Option<Child>,
    manager: Option<AdminManager>,
    consumed: [bool; protocol::CAPACITY],
}

struct AdminManager(HANDLE);
impl Drop for AdminManager {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

fn launch_admin(helper: &std::path::Path) -> Result<AdminManager, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        Win32::UI::{
            Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
            WindowsAndMessaging::SW_HIDE,
        },
        core::{PCWSTR, w},
    };
    let file: Vec<u16> = helper.as_os_str().encode_wide().chain(Some(0)).collect();
    let directory: Vec<u16> = helper
        .parent()
        .ok_or("专业模式组件不完整，请重新完整解压。")?
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let mut execute = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(file.as_ptr()),
        lpDirectory: PCWSTR(directory.as_ptr()),
        lpParameters: w!("--professional-trace-all"),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut execute) }.map_err(|error| {
        if error.code().0 == 0x800704c7u32 as i32 {
            "管理员监听未获授权。".to_string()
        } else {
            "管理员监听组件启动失败。".to_string()
        }
    })?;
    let process = execute.hProcess;
    if process.is_invalid() {
        return Err("管理员监听组件启动失败。".into());
    }
    let manager = AdminManager(process.0);
    let mut token = core::ptr::null_mut();
    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0;
    let elevated = unsafe {
        if OpenProcessToken(manager.0, TOKEN_QUERY, &mut token) == 0 {
            false
        } else {
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                (&mut elevation as *mut TOKEN_ELEVATION).cast(),
                size_of::<TOKEN_ELEVATION>() as u32,
                &mut returned,
            ) != 0;
            CloseHandle(token);
            ok && returned as usize == size_of::<TOKEN_ELEVATION>()
                && elevation.TokenIsElevated != 0
        }
    };
    if !elevated {
        return Err("管理员监听组件未取得管理员权限。".into());
    }
    Ok(manager)
}

impl MessageTrace {
    pub fn start(include_admin: bool) -> Result<Self, String> {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let directory = executable
            .parent()
            .ok_or("Missing application directory")?
            .to_path_buf();
        let helper = directory.join("owner-x86/keycrash-owner-probe.exe");
        for path in [
            directory.join("keycrash_trace64_v3.dll"),
            directory.join("owner-x86/keycrash_trace32_v3.dll"),
            helper.clone(),
            directory.join("keycrash-owner-probe.exe"),
        ] {
            if !path.is_file() {
                return Err("专业模式组件不完整，请重新完整解压。".into());
            }
        }
        let mapping = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                core::ptr::null(),
                PAGE_READWRITE,
                0,
                size_of::<protocol::Trace>() as u32,
                protocol::NAME,
            )
        };
        if mapping.is_null() {
            return Err("Cannot create professional message trace".into());
        }
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe { CloseHandle(mapping) };
            return Err("另一个专业录制正在运行，请先结束。".into());
        }
        let mapped = unsafe {
            MapViewOfFile(
                mapping,
                FILE_MAP_ALL_ACCESS,
                0,
                0,
                size_of::<protocol::Trace>(),
            )
        };
        if mapped.Value.is_null() {
            unsafe { CloseHandle(mapping) };
            return Err("Cannot map professional message trace".into());
        }
        let view = mapped.Value.cast::<protocol::Trace>();
        unsafe {
            core::ptr::write_bytes(view, 0, 1);
            (*view).magic = protocol::MAGIC;
            (*view).owner_pid = GetCurrentProcessId();
            (*view).started = GetTickCount();
            (*view).active.store(protocol::PREPARING, Ordering::Release);
        }
        let mut session = Self {
            mapping,
            view,
            worker: None,
            helper: None,
            manager: None,
            consumed: [false; protocol::CAPACITY],
        };
        if include_admin {
            session.manager = Some(launch_admin(&directory.join("keycrash-owner-probe.exe"))?);
        } else {
            let child = Command::new(helper)
                .arg("--professional-trace")
                .creation_flags(0x0800_0000)
                .spawn()
                .map_err(|e| e.to_string())?;
            unsafe { (*view).helper_pid.store(child.id(), Ordering::Release) };
            session.helper = Some(child);
            session.worker = Some(std::thread::spawn(move || host::run(&directory)));
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            let error = unsafe { (*view).error.load(Ordering::Acquire) };
            if error != 0 {
                return Err(match protocol::failure_detail(error) {
                    Some(detail) => format!("专业消息监听启动失败：{detail}"),
                    None => "专业消息监听启动失败。".into(),
                });
            }
            if unsafe { (*view).ready.load(Ordering::Acquire) }
                == (protocol::HOST64 | protocol::HOST32)
            {
                return Ok(session);
            }
            if session
                .helper
                .as_mut()
                .is_some_and(|helper| !matches!(helper.try_wait(), Ok(None)))
                || session.manager.as_ref().is_some_and(
                    |manager| unsafe { WaitForSingleObject(manager.0, 0) } == WAIT_OBJECT_0,
                )
            {
                return Err("专业消息监听组件提前退出。".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Err("专业消息监听启动超时。".into())
    }

    pub fn activate(&mut self) {
        unsafe {
            (*self.view).started = GetTickCount();
            (*self.view).active.store(1, Ordering::Release);
        }
    }

    pub fn drain(&mut self) -> Vec<crate::recording::Response> {
        let mut result = Vec::new();
        let trace = unsafe { &*self.view };
        if trace.active.load(Ordering::Acquire) == 1
            && (self
                .worker
                .as_ref()
                .is_some_and(|worker| worker.is_finished())
                || self
                    .helper
                    .as_mut()
                    .is_some_and(|helper| !matches!(helper.try_wait(), Ok(None)))
                || self.manager.as_ref().is_some_and(
                    |manager| unsafe { WaitForSingleObject(manager.0, 0) } == WAIT_OBJECT_0,
                ))
        {
            trace.error.store(1, Ordering::Release);
        }
        let count = (trace.count.load(Ordering::Acquire) as usize).min(protocol::CAPACITY);
        for (index, consumed) in self.consumed.iter_mut().enumerate().take(count) {
            let slot = &trace.slots[index];
            if *consumed || slot.ready.load(Ordering::Acquire) == 0 {
                continue;
            }
            result.push(crate::recording::Response {
                pid: slot.pid,
                tid: slot.tid,
                kind: slot.kind,
                value: slot.value,
                elapsed: Duration::from_millis(u64::from(slot.tick.wrapping_sub(trace.started))),
                architecture: slot.architecture,
                native_tick: slot.tick,
                created: u64::from(slot.created_low) | (u64::from(slot.created_high) << 32),
                module: u64::from(slot.module_low) | (u64::from(slot.module_high) << 32),
                binding: (slot.binding_profile != 0).then(|| crate::sequence::Snapshot {
                    profile: slot.binding_profile,
                    bindings: slot.bindings.map(|b| crate::sequence::Binding {
                        key1: b.key1,
                        key2: b.key2,
                        modifiers: b.modifiers,
                    }),
                }),
                association: None,
                path: PathBuf::from(String::from_utf16_lossy(
                    &slot.path[..slot.path_len.min(512) as usize],
                )),
            });
            *consumed = true;
        }
        result
    }

    pub fn incomplete(&self) -> bool {
        let trace = unsafe { &*self.view };
        trace.overflow.load(Ordering::Acquire) != 0 || trace.error.load(Ordering::Acquire) != 0
    }

    pub fn stop(&mut self) {
        unsafe { (*self.view).active.store(0, Ordering::Release) };
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        if let Some(mut helper) = self.helper.take() {
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline && matches!(helper.try_wait(), Ok(None)) {
                std::thread::sleep(Duration::from_millis(5));
            }
            if matches!(helper.try_wait(), Ok(None)) {
                let _ = helper.kill();
            }
            let _ = helper.wait();
        }
        if let Some(manager) = self.manager.take()
            && unsafe { WaitForSingleObject(manager.0, 2_000) } != WAIT_OBJECT_0
        {
            unsafe {
                (*self.view).error.store(1, Ordering::Release);
                TerminateProcess(manager.0, 1);
                WaitForSingleObject(manager.0, 1_000);
            }
        }
        // A recipient can exit after reserving a slot but before publishing it.
        // Preserve other responses, while preventing association on a lost one.
        let trace = unsafe { &*self.view };
        let count = (trace.count.load(Ordering::Acquire) as usize).min(protocol::CAPACITY);
        if trace.slots[..count]
            .iter()
            .any(|slot| slot.ready.load(Ordering::Acquire) == 0)
        {
            trace.error.store(1, Ordering::Release);
        }
    }
}

impl Drop for MessageTrace {
    fn drop(&mut self) {
        self.stop();
        unsafe {
            UnmapViewOfFile(
                windows_sys::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.view.cast(),
                },
            );
            CloseHandle(self.mapping);
        }
    }
}
