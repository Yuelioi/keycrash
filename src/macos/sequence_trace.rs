use crate::recording::Response;
use std::{
    ffi::{CStr, c_char, c_void},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
type Ref = *mut c_void;
#[repr(C)]
struct Receipt {
    pid: u32,
    tick: u32,
    path: [c_char; 4096],
}
unsafe extern "C" {
    fn kc_sequence_observer(marker: i64) -> Ref;
    fn kc_sequence_activate(context: Ref);
    fn kc_sequence_pump(context: Ref);
    fn kc_sequence_take(context: Ref, receipt: *mut Receipt) -> bool;
    fn kc_sequence_incomplete(context: Ref) -> bool;
    fn kc_sequence_observer_stop(context: Ref);
    fn kc_sequence_observer_free(context: Ref);
    fn kc_sequence_clock() -> u32;
}
pub struct MessageTrace {
    context: Ref,
    marker: i64,
    started: u32,
    stopped: bool,
}
impl MessageTrace {
    pub fn start(include_admin: bool) -> Result<Self, String> {
        if include_admin {
            return Err("此平台不支持管理员监听。".into());
        }
        if !crate::owner_probe::has_permission() {
            return Err("请在系统设置中允许辅助功能后重试。".into());
        }
        let marker = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos() as i64)
            ^ ((std::process::id() as i64) << 32);
        let context = unsafe { kc_sequence_observer(marker) };
        if context.is_null() {
            return Err("无法观察快捷键事件，请检查辅助功能权限。".into());
        }
        Ok(Self {
            context,
            marker,
            started: 0,
            stopped: false,
        })
    }
    pub fn marker(&self) -> i64 {
        self.marker
    }
    pub fn activate(&mut self) {
        self.started = unsafe { kc_sequence_clock() };
        unsafe {
            kc_sequence_activate(self.context);
        }
    }
    pub fn drain(&mut self) -> Vec<Response> {
        if !self.stopped {
            unsafe {
                kc_sequence_pump(self.context);
            }
        }
        let mut result = Vec::new();
        let mut receipt = Receipt {
            pid: 0,
            tick: 0,
            path: [0; 4096],
        };
        while unsafe { kc_sequence_take(self.context, &mut receipt) } {
            result.push(Response {
                pid: receipt.pid,
                tid: 0,
                kind: 3,
                value: 0,
                elapsed: Duration::from_millis(receipt.tick.wrapping_sub(self.started).into()),
                architecture: 64,
                path: unsafe { CStr::from_ptr(receipt.path.as_ptr()) }
                    .to_string_lossy()
                    .into_owned()
                    .into(),
                native_tick: receipt.tick,
                created: 0,
                module: 0,
                binding: None,
                association: None,
            });
        }
        result
    }
    pub fn stop(&mut self) {
        if !self.stopped {
            unsafe {
                kc_sequence_observer_stop(self.context);
            }
            self.stopped = true;
        }
    }
    pub fn incomplete(&self) -> bool {
        unsafe { kc_sequence_incomplete(self.context) }
    }
}
impl Drop for MessageTrace {
    fn drop(&mut self) {
        self.stop();
        unsafe {
            kc_sequence_observer_free(self.context);
        }
    }
}
