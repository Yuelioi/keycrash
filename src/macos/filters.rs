//! Explicitly selected global-filter isolation, with independent restore leases.
use crate::detector::Shortcut;
use crate::owner_probe::{Owner, event_flags};
use serde_json::Value;
use std::ffi::{CStr, CString, c_char, c_void};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

unsafe extern "C" {
    fn kc_filter_candidates() -> *mut c_char;
    fn kc_filter_free(value: *mut c_char);
    fn kc_filter_validate(value: *const c_char, disabled: bool) -> i32;
    fn kc_filter_guardian() -> i32;
    fn kc_filter_observe(
        key: u16,
        flags: u64,
        cancel: extern "C" fn(*mut c_void) -> bool,
        context: *mut c_void,
    ) -> *mut c_char;
}

pub fn guardian_entry() -> Option<i32> {
    (std::env::args().nth(1).as_deref() == Some("--keycrash-filter-guardian"))
        .then(|| unsafe { kc_filter_guardian() })
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub name: String,
    pub path: String,
    pub pid: u32,
    snapshot: Value,
}
pub struct Findings {
    pub receivers: Vec<Owner>,
    pub filters: Vec<Owner>,
}
const CHANGED: &str = "参与软件的监听状态已改变，请刷新列表后重试。";
pub const RESTORE: &str = "无法确认监听器已全部恢复，请重新打开参与检查的软件。";
const START: &str = "无法启动全局监听检查，请重试。";

fn native_json(pointer: *mut c_char) -> Result<Value, &'static str> {
    if pointer.is_null() {
        return Err(START);
    }
    let value = unsafe { serde_json::from_slice(CStr::from_ptr(pointer).to_bytes()) };
    unsafe { kc_filter_free(pointer) };
    value.map_err(|_| START)
}

pub fn candidates() -> Result<Vec<Candidate>, &'static str> {
    let value = native_json(unsafe { kc_filter_candidates() })?;
    let rows = value["candidates"]
        .as_array()
        .ok_or("无法读取全局监听软件，请重试。")?;
    rows.iter()
        .map(|row| {
            Ok(Candidate {
                name: row["name"].as_str().ok_or(START)?.into(),
                path: row["path"].as_str().ok_or(START)?.into(),
                pid: row["pid"]
                    .as_u64()
                    .ok_or(START)?
                    .try_into()
                    .map_err(|_| START)?,
                snapshot: row.clone(),
            })
        })
        .collect()
}

fn unchanged(candidate: &Candidate, disabled: bool) -> bool {
    let Ok(snapshot) = CString::new(candidate.snapshot.to_string()) else {
        return false;
    };
    unsafe { kc_filter_validate(snapshot.as_ptr(), disabled) == 0 }
}

struct Lease {
    child: Child,
    input: Option<ChildStdin>,
    records: mpsc::Receiver<Value>,
    finished: bool,
}
impl Lease {
    fn start(candidate: &Candidate, cancelled: &AtomicBool) -> Result<Self, &'static str> {
        let mut child = Command::new(std::env::current_exe().map_err(|_| START)?)
            .arg("--keycrash-filter-guardian")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| START)?;
        let output = child.stdout.take().ok_or(START)?;
        let (send, records) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let Ok(line) = line else {
                    break;
                };
                let Ok(value) = serde_json::from_str(&line) else {
                    break;
                };
                if send.send(value).is_err() {
                    break;
                }
            }
        });
        let input = child.stdin.take();
        let mut lease = Self {
            child,
            input,
            records,
            finished: false,
        };
        writeln!(lease.input.as_mut().ok_or(START)?, "{}", candidate.snapshot)
            .map_err(|_| START)?;
        let deadline = Instant::now() + Duration::from_secs(6);
        while Instant::now() < deadline && !cancelled.load(Ordering::Relaxed) {
            match lease.records.recv_timeout(Duration::from_millis(25)) {
                Ok(record) if record["kind"] == "ready" => return Ok(lease),
                Ok(record) if record["error"] == "unsupported" => {
                    return Err("此系统无法执行全局监听检查。");
                }
                Ok(record) => {
                    let restored = lease.finish();
                    if record["mutated"] == true && restored == Err(RESTORE) {
                        return Err(RESTORE);
                    }
                    return Err(CHANGED);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return match lease.finish() {
                        Err(RESTORE) => Err(RESTORE),
                        _ => Err(CHANGED),
                    };
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
        match lease.finish() {
            Err(RESTORE) => Err(RESTORE),
            _ => Err(START),
        }
    }
    fn finish(&mut self) -> Result<(), &'static str> {
        if self.finished {
            return Ok(());
        }
        self.input.take(); // EOF is the guardian's restore request, including cancellation.
        let deadline = Instant::now() + Duration::from_secs(4);
        let mut restored = false;
        let mut valid = false;
        while Instant::now() < deadline {
            while let Ok(record) = self.records.try_recv() {
                if record["kind"] == "restored" {
                    restored = record["ok"] == true;
                    valid = record["reason"] == "released";
                }
            }
            if let Ok(Some(status)) = self.child.try_wait() {
                // Drain after wait too: the final line may have reached the reader thread later.
                while !restored {
                    let Ok(record) = self.records.recv_timeout(Duration::from_millis(100)) else {
                        break;
                    };
                    if record["kind"] == "restored" {
                        restored = record["ok"] == true;
                        valid = record["reason"] == "released";
                        break;
                    }
                }
                self.finished = true;
                return if !status.success() || !restored {
                    Err(RESTORE)
                } else if !valid {
                    Err(CHANGED)
                } else {
                    Ok(())
                };
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        // Never kill the process that owns restoration. Its own 30s lease remains active.
        self.finished = true;
        Err(RESTORE)
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

extern "C" fn is_cancelled(context: *mut c_void) -> bool {
    unsafe { &*context.cast::<AtomicBool>() }.load(Ordering::Relaxed)
}
fn observe(shortcut: &Shortcut, cancelled: &AtomicBool) -> Result<Value, &'static str> {
    let phase = native_json(unsafe {
        kc_filter_observe(
            shortcut.key_code,
            event_flags(shortcut),
            is_cancelled,
            (cancelled as *const AtomicBool).cast_mut().cast(),
        )
    })?;
    match phase["error"].as_str() {
        Some("keys") => Err("请松开键盘上的按键后再定位。"),
        Some("secure") => Err("安全输入已启用，请关闭密码输入窗口后重试。"),
        Some("permission") => Err("请在系统设置中允许辅助功能后重试。"),
        Some(_) => Err("事件观察中断，请重试。"),
        None => Ok(phase),
    }
}
fn owners(phase: &Value) -> Vec<Owner> {
    phase["receivers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| {
            Some(Owner {
                pid: row["pid"].as_u64()?.try_into().ok()?,
                path: row["path"].as_str()?.into(),
            })
        })
        .collect()
}

pub fn locate(
    shortcut: &Shortcut,
    selected: &[Candidate],
    cancelled: &AtomicBool,
) -> Result<Findings, &'static str> {
    if selected.is_empty() || selected.len() > 6 {
        return Err("请选择一到六个软件参与检查。");
    }
    let mut leases: Vec<Option<Lease>> = (0..selected.len()).map(|_| None).collect();
    let mut touched = vec![false; selected.len()];
    let result = (|| {
        if cancelled.load(Ordering::Relaxed) {
            return Err(START);
        }
        if selected.iter().any(|c| !unchanged(c, false)) {
            return Err(CHANGED);
        }
        // Preconditions (including held keys and Secure Input) before changing any listener.
        observe(shortcut, cancelled)?;
        for (i, candidate) in selected.iter().enumerate() {
            leases[i] = Some(Lease::start(candidate, cancelled)?);
            touched[i] = true;
        }
        let phase = |leases: &[Option<Lease>]| -> Result<Value, &'static str> {
            if selected
                .iter()
                .enumerate()
                .any(|(i, c)| !unchanged(c, leases[i].is_some()))
            {
                return Err(CHANGED);
            }
            let observation = observe(shortcut, cancelled)?;
            if selected
                .iter()
                .enumerate()
                .any(|(i, c)| !unchanged(c, leases[i].is_some()))
            {
                return Err(CHANGED);
            }
            Ok(observation)
        };
        let control = phase(&leases)?;
        if control["mask"] != 63 && owners(&control).is_empty() {
            return Err("停用所选监听后按键仍未正常传递，无法确认原因。");
        }
        let mut filters = Vec::new();
        for (i, candidate) in selected.iter().enumerate() {
            leases[i].take().ok_or(START)?.finish()?;
            let first = phase(&leases)?;
            let repeat = phase(&leases)?;
            if first != repeat {
                return Err("多次检查结果不一致，请稍后重试。");
            }
            if first != control {
                filters.push(Owner {
                    pid: candidate.pid,
                    path: candidate.path.clone(),
                });
            }
            leases[i] = Some(Lease::start(candidate, cancelled)?);
            if phase(&leases)? != control {
                return Err("多次检查结果不一致，请稍后重试。");
            }
        }
        Ok(Findings {
            receivers: owners(&control),
            filters,
        })
    })();
    // Restore every participant even if one restoration fails; never publish partial success.
    let mut restore_error = None;
    for mut lease in leases.into_iter().flatten() {
        if let Err(error) = lease.finish()
            && restore_error != Some(RESTORE)
        {
            restore_error = Some(error);
        }
    }
    if let Some(error) = restore_error {
        return Err(error);
    }
    if selected
        .iter()
        .enumerate()
        .any(|(i, c)| touched[i] && !unchanged(c, false))
    {
        return Err(RESTORE);
    }
    result
}
