use crate::detector::Shortcut;
use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::{PCWSTR, w};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const ELEVATED_PROBE_TIMEOUT_MS: u32 = 120_000;
static OUTPUT_SEQUENCE: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnerProbeResult {
    Found {
        pid: u32,
        thread_id: u32,
        path: String,
        suppressed: bool,
        architecture: String,
    },
    NotFound,
    Error(String),
}

pub fn locate_owner(shortcut: &Shortcut) -> OwnerProbeResult {
    let directory = match executable_directory() {
        Ok(path) => path,
        Err(error) => return OwnerProbeResult::Error(error),
    };

    let x64 = run_helper(&directory.join("keycrash-owner-probe.exe"), shortcut, "x64");
    if matches!(x64, OwnerProbeResult::Found { .. }) {
        return x64;
    }

    let x86_helper = directory.join("owner-x86").join("keycrash-owner-probe.exe");
    let x86 = if x86_helper.is_file() {
        run_helper(&x86_helper, shortcut, "x86")
    } else {
        OwnerProbeResult::NotFound
    };
    if matches!(x86, OwnerProbeResult::Found { .. }) {
        return x86;
    }

    let standard_result = combine_misses(x64, x86);
    let elevated = run_elevated_helper(&directory.join("keycrash-owner-probe.exe"), shortcut);
    match elevated {
        OwnerProbeResult::Found { .. } => elevated,
        OwnerProbeResult::NotFound => standard_result,
        OwnerProbeResult::Error(elevated_error) => match standard_result {
            OwnerProbeResult::Error(standard_error) => OwnerProbeResult::Error(format!(
                "普通权限：{standard_error}；管理员权限：{elevated_error}"
            )),
            _ => OwnerProbeResult::Error(elevated_error),
        },
    }
}

fn combine_misses(x64: OwnerProbeResult, x86: OwnerProbeResult) -> OwnerProbeResult {
    match (x64, x86) {
        (OwnerProbeResult::Error(first), OwnerProbeResult::Error(second)) => {
            OwnerProbeResult::Error(format!("x64：{first}；x86：{second}"))
        }
        (OwnerProbeResult::Error(error), _) | (_, OwnerProbeResult::Error(error)) => {
            OwnerProbeResult::Error(error)
        }
        _ => OwnerProbeResult::NotFound,
    }
}

fn run_helper(helper: &Path, shortcut: &Shortcut, architecture: &str) -> OwnerProbeResult {
    let output = match Command::new(helper)
        .arg(shortcut.modifiers.to_string())
        .arg(shortcut.virtual_key.to_string())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            return OwnerProbeResult::Error(format!("无法启动深度定位组件：{error}"));
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_helper_output(stdout.trim(), architecture)
}

fn run_elevated_helper(helper: &Path, shortcut: &Shortcut) -> OwnerProbeResult {
    let output_file = match ProbeOutputFile::new() {
        Ok(file) => file,
        Err(error) => return OwnerProbeResult::Error(error),
    };
    let parameters = format!(
        "{} {} --include-x86 --output \"{}\"",
        shortcut.modifiers,
        shortcut.virtual_key,
        output_file.path.display()
    );
    let helper_wide = wide(helper.as_os_str());
    let parameters_wide = wide(OsStr::new(&parameters));
    let directory_wide = helper
        .parent()
        .map(|directory| wide(directory.as_os_str()))
        .unwrap_or_else(|| vec![0]);
    let mut execute = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(helper_wide.as_ptr()),
        lpParameters: PCWSTR(parameters_wide.as_ptr()),
        lpDirectory: PCWSTR(directory_wide.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };

    if let Err(error) = unsafe { ShellExecuteExW(&mut execute) } {
        return OwnerProbeResult::Error(format!("管理员定位未启动：{error}"));
    }
    if execute.hProcess.is_invalid() {
        return OwnerProbeResult::Error("管理员定位没有返回进程句柄".into());
    }

    let wait = unsafe { WaitForSingleObject(execute.hProcess, ELEVATED_PROBE_TIMEOUT_MS) };
    let mut exit_code = 0;
    let exit_result = unsafe { GetExitCodeProcess(execute.hProcess, &mut exit_code) };
    let _ = unsafe { CloseHandle(execute.hProcess) };
    if wait != WAIT_OBJECT_0 {
        return OwnerProbeResult::Error("管理员定位等待超时".into());
    }
    if let Err(error) = exit_result {
        return OwnerProbeResult::Error(format!("无法读取管理员定位退出码：{error}"));
    }

    let output = match std::fs::read_to_string(&output_file.path) {
        Ok(output) => output,
        Err(error) => {
            return OwnerProbeResult::Error(format!("无法读取管理员定位结果：{error}"));
        }
    };
    if output.trim().is_empty() && exit_code != 0 {
        return OwnerProbeResult::Error(format!("管理员定位失败，退出码 {exit_code}"));
    }
    parse_helper_output(output.trim(), "x64")
}

struct ProbeOutputFile {
    path: PathBuf,
}

impl ProbeOutputFile {
    fn new() -> Result<Self, String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        for _ in 0..16 {
            let sequence = OUTPUT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "keycrash-owner-{}-{timestamp}-{sequence}.txt",
                std::process::id()
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!("无法创建管理员定位结果文件：{error}"));
                }
            }
        }
        Err("无法分配管理员定位结果文件".into())
    }
}

impl Drop for ProbeOutputFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

fn executable_directory() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    Ok(executable
        .parent()
        .ok_or("KeyCrash 无法确定自身目录")?
        .into())
}

fn parse_helper_output(output: &str, architecture: &str) -> OwnerProbeResult {
    if output == "NOT_FOUND" {
        return OwnerProbeResult::NotFound;
    }

    if let Some(error) = output.strip_prefix("ERROR\t") {
        return OwnerProbeResult::Error(error.into());
    }

    let mut fields = output.splitn(5, '\t');
    let architecture = match fields.next() {
        Some("FOUND") => architecture,
        Some("FOUND_X86") => "x86",
        _ => return OwnerProbeResult::Error("深度定位组件返回了无法识别的结果".into()),
    };

    let parsed = (|| {
        Some(OwnerProbeResult::Found {
            pid: fields.next()?.parse().ok()?,
            thread_id: fields.next()?.parse().ok()?,
            suppressed: fields.next()? == "1",
            path: fields.next().unwrap_or_default().into(),
            architecture: architecture.into(),
        })
    })();
    parsed.unwrap_or_else(|| OwnerProbeResult::Error("深度定位结果不完整".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_found_not_found_and_error_results() {
        assert_eq!(
            parse_helper_output("FOUND\t42\t7\t1\tC:\\Apps\\QQ.exe", "x64"),
            OwnerProbeResult::Found {
                pid: 42,
                thread_id: 7,
                path: "C:\\Apps\\QQ.exe".into(),
                suppressed: true,
                architecture: "x64".into(),
            }
        );
        assert_eq!(
            parse_helper_output("NOT_FOUND", "x86"),
            OwnerProbeResult::NotFound
        );
        assert_eq!(
            parse_helper_output("FOUND_X86\t84\t9\t1\tC:\\Apps\\Legacy.exe", "x64"),
            OwnerProbeResult::Found {
                pid: 84,
                thread_id: 9,
                path: "C:\\Apps\\Legacy.exe".into(),
                suppressed: true,
                architecture: "x86".into(),
            }
        );
        assert_eq!(
            parse_helper_output("ERROR\tmissing hook", "x64"),
            OwnerProbeResult::Error("missing hook".into())
        );
    }

    #[test]
    fn combines_normal_probe_misses_without_losing_errors() {
        assert_eq!(
            combine_misses(OwnerProbeResult::NotFound, OwnerProbeResult::NotFound),
            OwnerProbeResult::NotFound
        );
        assert_eq!(
            combine_misses(
                OwnerProbeResult::Error("x64 failed".into()),
                OwnerProbeResult::NotFound,
            ),
            OwnerProbeResult::Error("x64 failed".into())
        );
    }
}
