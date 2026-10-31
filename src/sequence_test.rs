//! One explicit active test, with input execution isolated from the UI process.
use crate::{
    recording::{KeyEvent, Response},
    sequence_protocol as wire,
};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    io::{Read, Write},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
#[cfg(windows)]
#[path = "windows/trace.rs"]
mod trace;
#[cfg(target_os = "macos")]
#[path = "macos/sequence_trace.rs"]
mod trace;
fn clock() -> u32 {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::System::SystemInformation::GetTickCount()
    }
    #[cfg(target_os = "macos")]
    unsafe {
        unsafe extern "C" {
            fn kc_sequence_clock() -> u32;
        }
        kc_sequence_clock()
    }
}

pub enum Notice {
    Preparing,
    Countdown(u32),
    Sending,
    StepDone(usize),
    Done(Result<Outcome, String>),
}
pub struct Outcome {
    pub events: Vec<KeyEvent>,
    pub responses: Vec<Response>,
    pub completed_steps: usize,
    pub error: Option<String>,
    pub cancelled: bool,
    pub incomplete: bool,
    pub release_unconfirmed: bool,
}
pub struct Job {
    pub receiver: mpsc::Receiver<Notice>,
    cancel: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Job {
    pub fn start(plan: Vec<wire::Step>, include_admin: bool) -> Result<Self, String> {
        wire::validate(&plan).map_err(str::to_owned)?;
        let (tx, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let worker = thread::spawn(move || {
            let _ = tx.send(Notice::Preparing);
            let result = run(&plan, include_admin, &stop, &tx);
            let _ = tx.send(Notice::Done(result));
        });
        Ok(Self {
            receiver,
            cancel,
            worker: Some(worker),
        })
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Executor {
    child: Child,
    receiver: mpsc::Receiver<Result<wire::Record, &'static str>>,
    reader: Option<JoinHandle<()>>,
}
impl Executor {
    fn start(plan: &[wire::Step], marker: i64) -> Result<Self, String> {
        #[cfg(windows)]
        let directory = std::env::current_exe()
            .map_err(|_| "序列测试组件缺失，请重新完整解压。")?
            .parent()
            .ok_or("序列测试组件缺失，请重新完整解压。")?
            .to_path_buf();
        #[cfg(windows)]
        let mut command = {
            let _ = marker;
            Command::new(directory.join("keycrash-owner-probe.exe"))
        };
        #[cfg(target_os = "macos")]
        let mut command =
            Command::new(std::env::current_exe().map_err(|_| "无法启动序列测试组件。")?);
        command.args(["--professional-sequence", &std::process::id().to_string()]);
        #[cfg(target_os = "macos")]
        command.arg(marker.to_string());
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "无法启动序列测试组件。")?;
        let mut output = child.stdout.take().expect("piped output");
        let (tx, receiver) = mpsc::sync_channel(wire::MAX_STEPS * 3 + 4);
        let reader = thread::spawn(move || {
            let result = (|| {
                let mut magic = [0; 4];
                output.read_exact(&mut magic).map_err(|_| ())?;
                if magic != wire::RESULT_MAGIC {
                    return Err(());
                }
                for index in 0..wire::MAX_STEPS * 3 + 3 {
                    let record = wire::Record::read(&mut output).map_err(|_| ())?;
                    if (index == 0 && record.kind != wire::READY)
                        || (index != 0 && record.kind == wire::READY)
                        || !matches!(
                            record.kind,
                            wire::READY | wire::SENT | wire::END | wire::STEP_DONE
                        )
                    {
                        return Err(());
                    }
                    if tx.send(Ok(record)).is_err() {
                        return Ok(());
                    }
                    if record.kind == wire::END {
                        return Ok(());
                    }
                }
                Err(())
            })();
            if result.is_err() {
                let _ = tx.send(Err("序列测试组件提前退出，结果可能不完整。"));
            }
        });
        let mut executor = Self {
            child,
            receiver,
            reader: Some(reader),
        };
        wire::write_plan(plan, executor.child.stdin.as_mut().expect("control pipe"))?;
        match executor.receiver.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(record)) if record.kind == wire::READY => Ok(executor),
            _ => Err("序列测试组件未能就绪。".into()),
        }
    }
    fn go(&mut self) -> Result<(), String> {
        self.child
            .stdin
            .as_mut()
            .ok_or("序列测试已取消。")?
            .write_all(&[1])
            .map_err(|_| "无法发送测试序列。".into())
    }
    fn stop(&mut self) {
        self.child.stdin.take();
    }
}
impl Drop for Executor {
    fn drop(&mut self) {
        self.stop();
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until && matches!(self.child.try_wait(), Ok(None)) {
            thread::sleep(Duration::from_millis(5));
        }
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn pause(ms: u64, cancel: &AtomicBool) -> bool {
    let until = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < until {
        if cancel.load(Ordering::Acquire) {
            return false;
        }
        thread::sleep(Duration::from_millis(10));
    }
    !cancel.load(Ordering::Acquire)
}
fn run(
    plan: &[wire::Step],
    include_admin: bool,
    cancel: &AtomicBool,
    notify: &mpsc::Sender<Notice>,
) -> Result<Outcome, String> {
    let mut trace = trace::MessageTrace::start(include_admin)?;
    #[cfg(windows)]
    let marker = 0;
    #[cfg(target_os = "macos")]
    let marker = trace.marker();
    let mut executor = Executor::start(plan, marker)?;
    let mut result = Outcome {
        events: Vec::new(),
        responses: Vec::new(),
        completed_steps: 0,
        error: None,
        cancelled: false,
        incomplete: false,
        release_unconfirmed: false,
    };
    for count in (1..=3).rev() {
        let _ = notify.send(Notice::Countdown(count));
        if !pause(1000, cancel) {
            result.cancelled = true;
            executor.stop();
            drop(executor);
            trace.stop();
            return Ok(result);
        }
    }
    trace.activate();
    let started_tick = clock();
    executor.go()?;
    let _ = notify.send(Notice::Sending);
    let started = Instant::now();
    let mut cancel_at = None;
    let mut complete = false;
    let mut ended = false;
    let mut held = std::collections::BTreeSet::new();
    let expected: Vec<_> = plan
        .iter()
        .flat_map(|step| match step.action {
            wire::Action::Tap => vec![(step.key, true), (step.key, false)],
            wire::Action::Down => vec![(step.key, true)],
            wire::Action::Up => vec![(step.key, false)],
            wire::Action::Wait => Vec::new(),
        })
        .collect();
    loop {
        if cancel.load(Ordering::Acquire) && cancel_at.is_none() {
            executor.stop();
            cancel_at = Some(Instant::now());
            result.cancelled = true;
        }
        if started.elapsed() > Duration::from_millis(wire::MAX_MS as u64 + 3000)
            || cancel_at.is_some_and(|at| at.elapsed() > Duration::from_secs(2))
        {
            result.error = Some("序列测试超时，结果可能不完整。".into());
            break;
        }
        result.responses.extend(trace.drain());
        match executor.receiver.recv_timeout(Duration::from_millis(20)) {
            Ok(Ok(record)) if record.kind == wire::SENT => {
                if record.flags & !(wire::DOWN | wire::CLEANUP) != 0 {
                    result.error = Some("序列测试组件返回了不一致的步骤。".into());
                    break;
                }
                let down = record.flags & wire::DOWN != 0;
                if record.flags & wire::CLEANUP != 0 {
                    if down || !held.remove(&record.key) {
                        result.error = Some("序列测试组件返回了不一致的步骤。".into());
                        break;
                    }
                    result.incomplete = true;
                    continue;
                }
                if record.flags & !wire::DOWN != 0
                    || expected.get(result.events.len()) != Some(&(record.key, down))
                {
                    result.error = Some("序列测试组件返回了不一致的步骤。".into());
                    break;
                }
                if down {
                    held.insert(record.key);
                } else {
                    held.remove(&record.key);
                }
                result.events.push(KeyEvent {
                    #[cfg(windows)]
                    code: wire::virtual_key(record.key),
                    #[cfg(target_os = "macos")]
                    code: record.key,
                    label: wire::key_label(record.key),
                    down,
                    repeat: false,
                    initial: false,
                    injected: true,
                    missing_press: false,
                    elapsed: Duration::from_millis(record.tick.wrapping_sub(started_tick).into()),
                    native_tick: Some(record.tick),
                    observed_tick: Some(record.observed),
                });
            }
            Ok(Ok(record)) if record.kind == wire::STEP_DONE => {
                if record.key as usize != result.completed_steps
                    || result.completed_steps >= plan.len()
                {
                    result.error = Some("序列测试组件返回了不一致的步骤。".into());
                    break;
                }
                result.completed_steps += 1;
                let _ = notify.send(Notice::StepDone(result.completed_steps));
            }
            Ok(Ok(record)) if record.kind == wire::END => {
                ended = true;
                match record.key {
                    wire::COMPLETE => complete = true,
                    wire::CANCELLED => result.cancelled = true,
                    wire::KEYS_HELD => result.error = Some("请先松开按住的按键后再测试。".into()),
                    wire::PERMISSION_REQUIRED => {
                        result.error = Some("请在系统设置中允许辅助功能后重试。".into())
                    }
                    wire::RELEASE_UNCONFIRMED => {
                        result.release_unconfirmed = true;
                        result.error = Some(
                            "辅助功能权限已失效，按键释放状态无法确认；请手动按下并松开。".into(),
                        );
                    }
                    _ => result.error = Some("发送按键失败，请检查目标窗口权限。".into()),
                }
                break;
            }
            Ok(Err(error)) => {
                result.error = Some(error.into());
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            _ => {
                result.error = Some("序列测试组件提前退出，结果可能不完整。".into());
                break;
            }
        }
    }
    drop(executor);
    if !ended {
        result.release_unconfirmed = true;
        result.error =
            Some("测试组件异常结束，释放状态未确认；如有按键保持按下，请手动按下并松开。".into());
    } else if !held.is_empty() {
        result.release_unconfirmed = true;
        result.error = Some("部分按键可能未松开，请手动按下并松开它们。".into());
    }
    if complete {
        let settle = Instant::now() + Duration::from_millis(400);
        while Instant::now() < settle && !cancel.load(Ordering::Acquire) {
            result.responses.extend(trace.drain());
            thread::sleep(Duration::from_millis(10));
        }
    }
    trace.stop();
    result.responses.extend(trace.drain());
    result.cancelled |= cancel.load(Ordering::Acquire);
    result.incomplete |= trace.incomplete()
        || !complete
        || result.events.len() != expected.len()
        || result.completed_steps != plan.len()
        || result.error.is_some()
        || result.cancelled;
    #[cfg(windows)]
    crate::sequence::associate(&result.events, &mut result.responses, result.incomplete);
    Ok(result)
}
