# KeyCrash v1 Context

## Product truth

KeyCrash 解决的是“我已经知道哪个快捷键有问题，只想立刻知道是否冲突、被哪个软件占用”的任务。入口必须是点选修饰键、一次按钮操作和一个目标键输入，不能退化成要求用户浏览全量热键表的工具。

安全探测可靠回答组合当前能否通过 `RegisterHotKey` 注册。失败错误 1409 只能证明“已占用或系统保留”；只有进程内观察到精确匹配的 `WM_HOTKEY` 才能命名 owner，任何未观察到的结果都不得猜测进程。

## Technical decisions

- Windows 10/11 x64 first；Rust stable、Slint 1.17.1、`windows` 0.62.2。
- QQ 是已验证反例：普通 `WH_KEYBOARD_LL` 捕获无法保证阻止所有第三方热键动作，即使双方完整性级别相同且标准注册探针返回 1409。
- 默认产品路径采用虚拟 Ctrl / Alt / Shift tags，加当前窗口内单个目标键捕获；不安装全局键盘 hook，也不真实触发完整组合。
- 等待态只接受一个目标键；物理 Ctrl / Alt / Shift / Win 输入被忽略，Esc 取消。
- F1、`PrtSc` 等普通 Winit 事件可能被第三方全局热键吞掉；仅在 KeyCrash 是前台进程且处于等待态时，每 8ms 轮询全部受支持目标键的 `GetAsyncKeyState` 最高位按下边沿作为兜底，不依赖不可靠的最低位。首次 tick 只建立基线，按住不重复，物理修饰键存在时不捕获。
- 修饰键标签、预览和最终读数统一使用 Ctrl → Alt → Shift 顺序。
- `RegisterHotKey` 与 `UnregisterHotKey` 在同一次同步探测调用中、同一线程配对完成。
- `ProbeReport` 分离 status、错误码与已知 system rule；UI 文案从报告派生。
- `Ctrl+Alt+Delete` 与 `Win+L` 规则只匹配精确 modifiers；F12 规则独立匹配调试器保留键。
- owner 定位是已占用结果上的显式第二步；普通 x64/x86 Helper 先尝试，均未命中时再由一次 UAC 启动管理员 x64 coordinator 覆盖 x64/x86，最多真实触发四次完整组合。
- Hook DLL 在 `PM_REMOVE` 阶段直接记录 `GetCurrentProcessId()` / TID，不依赖可能为 NULL 的 `MSG.hwnd`；精确匹配后尽量把消息改成 `WM_NULL`。
- 低级 Hook、Raw Input、驱动或拒绝注入输入的软件可能先响应或不产生 `WM_HOTKEY`；suppression 只能标为 best effort。
- x64/x86 Helper 与 DLL 通过固定大小的命名共享内存 + Event 协作，定位完成或超时即卸载 Hook。
- Hotkey Detective 为 GPL-3.0；KeyCrash 仅参考公开机制与 Win32 文档，正式实现独立重写。
- 默认模式不做 Toolhelp32 进程快照或时间相关性猜测；失败时明确显示 owner 未知。
- GUI 固定使用 `asInvoker`：管理员 GUI 位于前台时会阻断普通软件的注入热键链，Snipaste F1 已实证从 FOUND 退化为 NOT_FOUND。UAC 只用于普通定位未命中后的短生命周期管理员 helper。

## UX decisions

- 中文为默认语言，同时支持 English 与日本語；460×380 单窗口、单主操作，切换语言不清空当前检测状态。
- 视觉方向是“维修台信号仪”：冷灰面板、单一状态色、捕获时三路轨迹汇入读数胶囊，结果态恢复安静。
- 状态不能只靠颜色表达；按钮必须支持 Tab、Space、Enter，Esc 取消捕获。
- owner 标题使用不带 `.exe` 的文件 stem；完整路径保留，成功态提供“打开文件位置”。界面只陈述观察到的 WM_HOTKEY/PID/TID，不声称原动作已拦截。
- 右上角“？”使用同窗帮助面板渐进披露三步流程与真实触发风险，不新增窗口；“×”或 Esc 返回原状态。
- 描边次操作的黑色焦点环只表达键盘导航焦点；鼠标点击不主动聚焦，点击后保持默认钢灰描边。

## Validation baseline

- `cargo fmt --check`
- `cargo test`
- `cargo clippy -- -D warnings`
- `scripts/build-release.ps1`
- `scripts/test-owner-fixture.ps1`
- x64/x86 受控 owner fixture 返回精确 PID 且 `WM_HOTKEY` 被改写。
- Slint Viewer 对 idle、waiting-key、occupied、locating、owner-found、owner-missed 状态截图验证。

## Open decisions

- 尚未选择 Slint GPLv3、Royalty-free attribution 或商业许可路径。
- 尚未决定安装器、代码签名和更新渠道。
- 诊断报告的文件格式和 UI 入口仍待实现。
- 首次 Git 提交需用户明确授权。
