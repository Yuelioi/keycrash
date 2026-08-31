#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Language {
    #[default]
    Zh,
    En,
    Ja,
}

impl Language {
    pub fn from_code(code: &str) -> Self {
        match code {
            "en" => Self::En,
            "ja" => Self::Ja,
            _ => Self::Zh,
        }
    }
}

struct Message {
    zh: &'static str,
    en: &'static str,
    ja: &'static str,
}

macro_rules! m {
    ($zh:literal, $en:literal, $ja:literal) => {
        Message {
            zh: $zh,
            en: $en,
            ja: $ja,
        }
    };
}

const MESSAGES: &[Message] = &[
    m!(
        "没有识别到目标键",
        "Target key not recognized",
        "対象キーを認識できません"
    ),
    m!("请再试一次", "Please try again", "もう一度お試しください"),
    m!(
        "本地输入 · 未发送完整组合",
        "Local input · Full shortcut not sent",
        "ローカル入力 · ショートカット未送信"
    ),
    m!("重新选择", "Choose again", "選び直す"),
    m!(
        "没有可用于定位的软件冲突，请重新检测。",
        "No software conflict is available to locate. Run the check again.",
        "特定できるソフトウェア競合がありません。再検出してください。"
    ),
    m!(
        "无法打开文件位置 · 请按上方路径手动打开",
        "Could not open the file location · Open the path above manually",
        "ファイルの場所を開けません · 上のパスを手動で開いてください"
    ),
    m!(
        "这个组合当前可用",
        "This shortcut is available",
        "この組み合わせは使用できます"
    ),
    m!(
        "Windows 接受了临时注册，KeyCrash 已立即释放它。",
        "Windows accepted a temporary registration, which KeyCrash immediately released.",
        "Windows は一時登録を受け入れ、KeyCrash はすぐに解放しました。"
    ),
    m!("再测一次", "Check another", "もう一度検出"),
    m!(
        "已被占用或系统保留",
        "In use or reserved by the system",
        "使用中またはシステム予約済み"
    ),
    m!("定位占用软件", "Locate the app", "使用中のアプリを特定"),
    m!(
        "检测没有完成",
        "Check did not complete",
        "検出を完了できません"
    ),
    m!("重新检测", "Run again", "再検出"),
    m!(
        "冲突已确认；定位会真实触发组合，原动作可能执行。",
        "Conflict confirmed. Locating sends the shortcut, so its original action may run.",
        "競合を確認しました。特定時はショートカットを送信するため、元の操作が実行される場合があります。"
    ),
    m!(
        "正在定位占用软件",
        "Locating the app",
        "使用中のアプリを特定中"
    ),
    m!(
        "先检查普通软件；未命中时再请求管理员权限。",
        "Checking regular apps first; administrator access is requested only if needed.",
        "まず通常のアプリを確認し、必要な場合のみ管理者権限を要求します。"
    ),
    m!(
        "普通 / 管理员 × x64 / x86",
        "Standard / admin × x64 / x86",
        "標準 / 管理者 × x64 / x86"
    ),
    m!("定位中…", "Locating…", "特定中…"),
    m!(
        "已占用，但没有观察到软件",
        "In use, but no app was observed",
        "使用中ですが、アプリを観測できません"
    ),
    m!(
        "可能来自低级 Hook、Raw Input、驱动，或目标忽略了模拟输入。",
        "It may use a low-level hook, Raw Input, a driver, or ignore simulated input.",
        "低レベル Hook、Raw Input、ドライバー、またはシミュレート入力の無視が原因の可能性があります。"
    ),
    m!(
        "x64 + x86 深度定位 · 未观察到 WM_HOTKEY",
        "x64 + x86 deep scan · WM_HOTKEY not observed",
        "x64 + x86 詳細検出 · WM_HOTKEY 未観測"
    ),
    m!(
        "定位没有完成",
        "Location did not complete",
        "特定を完了できません"
    ),
    m!("占用软件未知", "Unknown app", "使用中のアプリは不明"),
    m!(
        "深度定位组件错误",
        "Deep scan component error",
        "詳細検出コンポーネントエラー"
    ),
    m!(
        "运行时探测 · 可直接注册",
        "Runtime probe · Registration available",
        "実行時プローブ · 登録可能"
    ),
    m!(
        "系统规则 + 运行时探测",
        "System rule + runtime probe",
        "システムルール + 実行時プローブ"
    ),
    m!(
        "检测证据不可用",
        "Probe evidence unavailable",
        "検出証拠を利用できません"
    ),
    m!("选择修饰键", "Choose modifiers", "修飾キーを選択"),
    m!(
        "单击 Ctrl、Alt、Shift；开始后只按一个目标键。",
        "Select Ctrl, Alt, or Shift, then press only one target key.",
        "Ctrl、Alt、Shift を選び、開始後は対象キーを 1 つだけ押します。"
    ),
    m!("等待目标键", "Waiting for a key", "対象キー待ち"),
    m!(
        "安全输入 · 不发送完整组合",
        "Safe input · Full shortcut not sent",
        "安全入力 · ショートカット未送信"
    ),
    m!("开始检测", "Start check", "検出を開始"),
    m!(
        "请按一个目标键",
        "Press one target key",
        "対象キーを 1 つ押してください"
    ),
    m!(
        "只按 A、F12、Space 等目标键；不要再按修饰键。",
        "Press A, F12, Space, or another target key without modifiers.",
        "A、F12、Space などの対象キーだけを押してください。"
    ),
    m!("取消", "Cancel", "キャンセル"),
    m!("按目标键…", "Press a key…", "対象キーを押す…"),
    m!("未知进程", "Unknown process", "不明なプロセス"),
    m!(
        "深度定位组件返回错误，请再试一次。",
        "The deep scan component returned an error. Please try again.",
        "詳細検出コンポーネントがエラーを返しました。もう一度お試しください。"
    ),
    m!(
        "Ctrl + Alt + Delete 由 Windows 安全桌面保留",
        "Ctrl + Alt + Delete is reserved by Windows Secure Desktop",
        "Ctrl + Alt + Delete は Windows セキュアデスクトップによって予約されています"
    ),
    m!(
        "Win + L 由 Windows 锁屏功能保留",
        "Win + L is reserved for Windows lock",
        "Win + L は Windows のロック機能によって予約されています"
    ),
    m!(
        "F12 始终为调试器保留",
        "F12 is always reserved for the debugger",
        "F12 は常にデバッガー用に予約されています"
    ),
    m!(
        "请按一个目标键，例如 A、F12 或 Space。",
        "Press a target key such as A, F12, or Space.",
        "A、F12、Space などの対象キーを押してください。"
    ),
    m!(
        "Ctrl、Alt、Shift 请使用上方标签选择，只按目标键。",
        "Choose Ctrl, Alt, and Shift with the tags above; press only the target key.",
        "Ctrl、Alt、Shift は上のタグで選び、対象キーだけを押してください。"
    ),
];

pub fn text(language: Language, source: &str) -> String {
    let canonical = MESSAGES
        .iter()
        .find(|message| message.zh == source || message.en == source || message.ja == source);
    if let Some(message) = canonical {
        return match language {
            Language::Zh => message.zh,
            Language::En => message.en,
            Language::Ja => message.ja,
        }
        .into();
    }

    translate_dynamic(language, source).unwrap_or_else(|| source.into())
}

fn translate_dynamic(language: Language, source: &str) -> Option<String> {
    if let Some(value) = source
        .strip_prefix("无法识别目标键“")
        .and_then(|value| value.strip_suffix("”，请换一个键。"))
    {
        return Some(match language {
            Language::Zh => source.into(),
            Language::En => format!("The target key “{value}” is not supported. Try another key."),
            Language::Ja => {
                format!("対象キー「{value}」を認識できません。別のキーをお試しください。")
            }
        });
    }

    if let Some(code) = extract_any(
        source,
        &[
            ("Windows 返回了系统错误 ", "，请再试一次。"),
            ("Windows returned system error ", ". Please try again."),
            (
                "Windows がシステムエラー ",
                " を返しました。もう一度お試しください。",
            ),
        ],
    ) {
        return Some(match language {
            Language::Zh => format!("Windows 返回了系统错误 {code}，请再试一次。"),
            Language::En => format!("Windows returned system error {code}. Please try again."),
            Language::Ja => {
                format!("Windows がシステムエラー {code} を返しました。もう一度お試しください。")
            }
        });
    }

    if let Some(name) = extract_any(
        source,
        &[
            ("", "占用了它"),
            ("", " is using it"),
            ("", " が使用しています"),
        ],
    ) && !name.is_empty()
    {
        return Some(match language {
            Language::Zh => format!("{name}占用了它"),
            Language::En => format!("{name} is using it"),
            Language::Ja => format!("{name} が使用しています"),
        });
    }

    if let Some(code) = extract_any(
        source,
        &[
            ("运行时探测 · 错误 ", " · owner 未知"),
            ("Runtime probe · Error ", " · Unknown owner"),
            ("実行時プローブ · エラー ", " · 所有者不明"),
        ],
    ) {
        return Some(match language {
            Language::Zh => format!("运行时探测 · 错误 {code} · owner 未知"),
            Language::En => format!("Runtime probe · Error {code} · Unknown owner"),
            Language::Ja => format!("実行時プローブ · エラー {code} · 所有者不明"),
        });
    }
    if let Some(code) = extract_any(
        source,
        &[
            ("Windows 系统错误 · ", ""),
            ("Windows system error · ", ""),
            ("Windows システムエラー · ", ""),
        ],
    ) {
        return Some(match language {
            Language::Zh => format!("Windows 系统错误 · {code}"),
            Language::En => format!("Windows system error · {code}"),
            Language::Ja => format!("Windows システムエラー · {code}"),
        });
    }

    None
}

fn extract_any<'a>(source: &'a str, patterns: &[(&str, &str)]) -> Option<&'a str> {
    patterns
        .iter()
        .find_map(|(prefix, suffix)| source.strip_prefix(prefix)?.strip_suffix(suffix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_core_copy_in_all_languages() {
        assert_eq!(text(Language::En, "开始检测"), "Start check");
        assert_eq!(text(Language::Ja, "开始检测"), "検出を開始");
        assert_eq!(text(Language::Zh, "Start check"), "开始检测");
    }

    #[test]
    fn leaves_paths_and_process_names_unchanged() {
        assert_eq!(
            text(Language::Ja, r"C:\\Apps\\tool.exe"),
            r"C:\\Apps\\tool.exe"
        );
    }
}
