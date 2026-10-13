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
        "正在恢复全局监听",
        "Restoring global listeners",
        "グローバル監視を復元中"
    ),
    m!(
        "没有找到可参与检查的全局监听软件。",
        "No eligible global listener apps found.",
        "検査できるグローバル監視アプリがありません。"
    ),
    m!(
        "这里只列出候选软件，尚未确认它们使用了此组合。",
        "These are candidates; use of this shortcut has not been confirmed.",
        "候補の一覧です。この組み合わせの使用はまだ確認していません。"
    ),
    m!(
        "在系统设置中允许 KeyCrash，然后返回点击开始检查。",
        "Allow KeyCrash in System Settings, then return and start the check.",
        "システム設定で KeyCrash を許可してから検査を開始してください。"
    ),
    m!(
        "正在检查全局监听",
        "Checking global listeners",
        "グローバル監視を検査中"
    ),
    m!(
        "将多次触发此组合，结束或取消后恢复监听。",
        "The shortcut runs several times. Listeners are restored after completion or cancellation.",
        "組み合わせを複数回送信し、完了・キャンセル後に監視を復元します。"
    ),
    m!("仅检查所选软件", "Selected apps only", "選択したアプリのみ"),
    m!(
        "全局监听检查未完成",
        "Global listener check incomplete",
        "グローバル監視の検査が未完了です"
    ),
    m!(
        "未输出定位结论",
        "No attribution result",
        "特定結果はありません"
    ),
    m!(
        "改变按键传递的软件",
        "Apps that changed key delivery",
        "キーの伝達を変更したアプリ"
    ),
    m!("热键接收者", "Hotkey receivers", "ホットキーの受信アプリ"),
    m!(
        "所选软件未改变此次按键传递，不能排除未参与检查的软件。",
        "Selected apps did not change this key delivery. Other apps were not ruled out.",
        "選択したアプリは今回のキー伝達を変更しませんでした。未選択のアプリは除外できません。"
    ),
    m!(
        "全局监听检查完成",
        "Global listener check complete",
        "グローバル監視の検査が完了しました"
    ),
    m!(
        "未确认影响此组合的软件",
        "No affecting app confirmed",
        "影響するアプリを確認できませんでした"
    ),
    m!(
        "检查已结束，所选监听已恢复。",
        "Check finished. Selected listeners restored.",
        "検査が終了し、選択した監視を復元しました。"
    ),
    m!(
        "参与软件的监听状态已改变，请刷新列表后重试。",
        "A selected app's listeners changed. Refresh the list and retry.",
        "選択したアプリの監視状態が変わりました。一覧を更新して再試行してください。"
    ),
    m!(
        "无法确认监听器已全部恢复，请重新打开参与检查的软件。",
        "Listener restoration could not be confirmed. Reopen the selected apps.",
        "監視の復元を確認できません。選択したアプリを開き直してください。"
    ),
    m!(
        "无法启动全局监听检查，请重试。",
        "Could not start the global listener check. Please retry.",
        "グローバル監視の検査を開始できません。再試行してください。"
    ),
    m!(
        "无法读取全局监听软件，请重试。",
        "Could not list global listener apps. Please retry.",
        "グローバル監視アプリを取得できません。再試行してください。"
    ),
    m!(
        "此系统无法执行全局监听检查。",
        "Global listener checking is unavailable on this system.",
        "このシステムではグローバル監視を検査できません。"
    ),
    m!(
        "安全输入已启用，请关闭密码输入窗口后重试。",
        "Secure Input is enabled. Close password entry windows and retry.",
        "セキュア入力が有効です。パスワード入力画面を閉じて再試行してください。"
    ),
    m!(
        "请在系统设置中允许辅助功能后重试。",
        "Allow Accessibility in System Settings, then retry.",
        "システム設定でアクセシビリティを許可して再試行してください。"
    ),
    m!(
        "请选择一到六个软件参与检查。",
        "Select one to six apps to check.",
        "検査するアプリを1〜6個選択してください。"
    ),
    m!(
        "停用所选监听后按键仍未正常传递，无法确认原因。",
        "Key delivery did not recover with selected listeners paused. The cause is unconfirmed.",
        "選択した監視を停止してもキーの伝達が回復せず、原因を確認できません。"
    ),
    m!(
        "多次检查结果不一致，请稍后重试。",
        "Repeated checks disagreed. Please try again later.",
        "複数回の検査結果が一致しません。後で再試行してください。"
    ),
    m!(
        "系统设置中有此组合",
        "Shortcut enabled in system settings",
        "システム設定で有効な組み合わせ"
    ),
    m!(
        "没有观察到占用软件",
        "No receiving app observed",
        "受信するアプリが見つかりませんでした"
    ),
    m!(
        "可临时注册；定位会触发组合，用于检查共享热键。",
        "Registration succeeded. Locate sends the shortcut to check shared hotkeys.",
        "一時登録に成功。「特定」は共有ホットキーを確認するため組み合わせを送信します。"
    ),
    m!(
        "需要辅助功能权限",
        "Accessibility permission required",
        "アクセシビリティの許可が必要です"
    ),
    m!(
        "在系统设置中允许 KeyCrash，然后返回点击定位。",
        "Allow KeyCrash in System Settings, then return and choose Locate.",
        "システム設定で KeyCrash を許可し、戻って「特定」を押してください。"
    ),
    m!(
        "授权前不会发送快捷键",
        "No shortcut sent before permission is granted",
        "許可前にはショートカットを送信しません"
    ),
    m!(
        "正在观察此组合的接收进程，原动作可能执行。",
        "Observing the receiving process. The shortcut action may run.",
        "受信プロセスを確認しています。元の操作が実行される場合があります。"
    ),
    m!(
        "macOS 快捷键事件",
        "macOS shortcut events",
        "macOS ショートカットイベント"
    ),
    m!(
        "macOS 热键接收事件 · 已匹配本次组合",
        "macOS hotkey receiver · Matched this shortcut",
        "macOS ホットキー受信先 · 今回の組み合わせと一致"
    ),
    m!(
        "macOS 快捷键事件 · 未匹配",
        "macOS shortcut events · No match",
        "macOS ショートカットイベント · 一致なし"
    ),
    m!(
        "未收到匹配的热键事件；目标可能忽略模拟输入，或占用已释放。",
        "No matching hotkey event. The app may ignore synthetic input or have released the shortcut.",
        "一致するイベントがありません。疑似入力が無視されたか、登録が解放された可能性があります。"
    ),
    m!(
        "请松开键盘上的按键后再定位。",
        "Release held keys before locating the app.",
        "押しているキーを離してから特定してください。"
    ),
    m!(
        "无法观察快捷键事件，请检查辅助功能权限。",
        "Cannot observe shortcut events. Check Accessibility permission.",
        "イベントを確認できません。アクセシビリティの許可を確認してください。"
    ),
    m!(
        "无法创建快捷键观察器。",
        "Could not create the shortcut observer.",
        "イベント監視を開始できません。"
    ),
    m!(
        "无法创建快捷键输入。",
        "Could not create shortcut input.",
        "ショートカット入力を作成できません。"
    ),
    m!(
        "事件观察中断，请重试。",
        "Event observation was interrupted. Try again.",
        "イベント監視が中断されました。再試行してください。"
    ),
    m!(
        "此组合可临时注册",
        "Temporary registration succeeded",
        "一時登録に成功しました"
    ),
    m!(
        "临时注册已释放；应用内快捷键与监听不在检测范围。",
        "Released; app shortcuts and listeners are not checked.",
        "登録は解放済み。アプリ内の操作や監視は対象外です。"
    ),
    m!(
        "点选 ⌃、⌥、⇧、⌘；开始后只按一个目标键。",
        "Select ⌃, ⌥, ⇧, or ⌘, then press only the target key.",
        "⌃、⌥、⇧、⌘ を選び、対象キーだけを押します。"
    ),
    m!(
        "系统设置中启用了此组合；仍可定位实际接收的软件。",
        "Enabled in system settings. Locate can still identify the receiving app.",
        "システム設定で有効です。実際に受信するアプリを引き続き特定できます。"
    ),
    m!(
        "macOS 系统快捷键 · 未尝试注册",
        "macOS system shortcut · No registration attempted",
        "macOS のシステム設定 · 登録は未実行"
    ),
    m!(
        "无法读取 macOS 系统快捷键，请重试。",
        "Could not read macOS system shortcuts. Try again.",
        "macOS のシステム設定を読み取れません。再試行してください。"
    ),
    m!(
        "临时热键释放失败，请退出 KeyCrash 后重试。",
        "Could not release the test hotkey. Quit KeyCrash and retry.",
        "一時登録を解放できません。KeyCrash を終了して再試行してください。"
    ),
    m!(
        "macOS 无法注册此组合；请换用 ⌘ 或 ⌃ 后重试。",
        "macOS rejected registration. Try a combination with ⌘ or ⌃.",
        "macOS が登録を拒否しました。⌘ または ⌃ で再試行してください。"
    ),
    m!(
        "系统保留快捷键",
        "System-reserved shortcut",
        "システム予約のショートカット"
    ),
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
        "未发现注册冲突",
        "No registration conflict",
        "登録の競合は見つかりません"
    ),
    m!(
        "其他软件仍可能监听此键；本次临时注册已释放。",
        "Other apps may listen; test registration released.",
        "一時登録は解放済み。他のアプリのキー監視は対象外です。"
    ),
    m!("再测一次", "Check another", "もう一度検出"),
    m!(
        "已发现注册冲突",
        "Registration conflict found",
        "登録の競合が見つかりました"
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
        "正在自动检查占用软件；如需更高权限，Windows 会请求一次确认。",
        "Checking automatically. Windows will ask once if higher privileges are needed.",
        "自動で確認しています。より高い権限が必要な場合は、Windows が一度だけ確認します。"
    ),
    m!(
        "自动深度定位 · x64 / x86",
        "Automatic deep scan · x64 / x86",
        "自動詳細検出 · x64 / x86"
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
        "注册检测 · 不涵盖所有键盘监听",
        "Registration check · Not all key listeners",
        "登録を検査 · すべてのキー監視は対象外"
    ),
    m!(
        "Windows 系统规则 · 未尝试注册",
        "Windows rule · Registration not attempted",
        "Windows の予約 · 登録は未実行"
    ),
    m!(
        "检测证据不可用",
        "Probe evidence unavailable",
        "検出証拠を利用できません"
    ),
    m!("选择修饰键", "Choose modifiers", "修飾キーを選択"),
    m!(
        "单击 Ctrl、Alt、Shift、Win；开始后只按一个目标键。",
        "Select Ctrl, Alt, Shift, or Win, then press only one target key.",
        "Ctrl、Alt、Shift、Win を選び、開始後は対象キーを 1 つだけ押します。"
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
        "只按 A、F9、Space 等目标键；不要再按修饰键。",
        "Press A, F9, Space, or another target key without modifiers.",
        "A、F9、Space などの対象キーだけを押してください。"
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
        "F12 为调试器保留，请换一个键。",
        "F12 is reserved for debuggers. Choose another key.",
        "F12 はデバッガー用です。別のキーを選んでください。"
    ),
    m!(
        "请按一个目标键，例如 A、F9 或 Space。",
        "Press a target key such as A, F9, or Space.",
        "A、F9、Space などの対象キーを押してください。"
    ),
    m!(
        "Ctrl、Alt、Shift、Win 请使用上方标签选择，只按目标键。",
        "Choose Ctrl, Alt, Shift, and Win with the tags above; press only the target key.",
        "Ctrl、Alt、Shift、Win は上のタグで選び、対象キーだけを押してください。"
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
    if let Some(code) = extract_any(
        source,
        &[
            ("macOS 系统错误 · ", ""),
            ("macOS system error · ", ""),
            ("macOS システムエラー · ", ""),
        ],
    ) {
        return Some(match language {
            Language::Zh => format!("macOS 系统错误 · {code}"),
            Language::En => format!("macOS system error · {code}"),
            Language::Ja => format!("macOS システムエラー · {code}"),
        });
    }

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
    fn macos_error_codes_survive_language_switching() {
        assert_eq!(
            text(Language::En, "macOS 系统错误 · -9868"),
            "macOS system error · -9868"
        );
        assert_eq!(
            text(Language::Ja, "macOS system error · -9868"),
            "macOS システムエラー · -9868"
        );
        assert_eq!(
            text(Language::Zh, "macOS システムエラー · -9868"),
            "macOS 系统错误 · -9868"
        );
        assert_eq!(
            text(Language::En, "此组合可临时注册"),
            "Temporary registration succeeded"
        );
    }

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
