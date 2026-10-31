//! Input submission and observed-response evidence shared by both platforms.
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct KeyEvent {
    #[cfg_attr(target_os = "macos", allow(dead_code))] // Windows binding replay fields.
    pub code: u32,
    pub label: String,
    pub down: bool,
    pub repeat: bool,
    pub initial: bool,
    pub injected: bool,
    pub missing_press: bool,
    pub elapsed: Duration,
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub native_tick: Option<u32>,
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub observed_tick: Option<u32>,
}

pub struct Response {
    pub pid: u32,
    pub tid: u32,
    pub kind: u32,
    pub value: u32,
    pub elapsed: Duration,
    pub architecture: u32,
    pub path: std::path::PathBuf,
    #[cfg_attr(target_os = "macos", allow(dead_code))] // Windows recipient identity/replay fields.
    pub native_tick: u32,
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub created: u64,
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub module: u64,
    pub binding: Option<crate::sequence::Snapshot>,
    pub association: Option<crate::sequence::Association>,
}
