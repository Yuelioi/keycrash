#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeStatus {
    Available,
    Blocked,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceSource {
    RuntimeProbe,
    SystemRule,
    SystemError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerAttribution {
    NotApplicable,
    #[cfg(windows)]
    KnownSystem,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemRule {
    #[cfg(target_os = "macos")]
    MacSymbolicHotKey,
    #[cfg(windows)]
    SecureAttention,
    #[cfg(windows)]
    LockWorkstation,
    #[cfg(windows)]
    DebuggerF12,
}

impl SystemRule {
    pub fn explanation(self) -> &'static str {
        match self {
            #[cfg(target_os = "macos")]
            Self::MacSymbolicHotKey => "系统设置中启用了此组合；仍可定位实际接收的软件。",
            #[cfg(windows)]
            Self::SecureAttention => "Ctrl + Alt + Delete 由 Windows 安全桌面保留",
            #[cfg(windows)]
            Self::LockWorkstation => "Win + L 由 Windows 锁屏功能保留",
            #[cfg(windows)]
            Self::DebuggerF12 => "F12 为调试器保留，请换一个键。",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeReport {
    pub status: ProbeStatus,
    pub source: EvidenceSource,
    pub owner: OwnerAttribution,
    pub code: Option<i64>,
    pub system_rule: Option<SystemRule>,
    pub detail: Option<&'static str>,
}
