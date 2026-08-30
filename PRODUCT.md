# Product

<!-- impeccable:product-schema 1 -->

## Platform

windows

## Users

KeyCrash 面向遇到全局快捷键失效、被占用或无法注册问题的 Windows 10/11 用户。首要用户是不想浏览庞大热键表、只想检查一个具体组合的普通用户和效率工具用户。

## Product Purpose

用户单击选择 Ctrl、Alt、Shift 修饰键，点击“开始检测”，再在 KeyCrash 当前窗口内只按一个目标键；KeyCrash 先安全判断组合是否可注册，若已占用，再让用户一键定位接收该全局热键的软件。用户无需浏览全量热键表。

## Positioning

KeyCrash 以“虚拟选择修饰键 + 本地捕获一个目标键”为安全入口，再用按需、短时的双架构消息 Hook 定位标准 `RegisterHotKey` 占用者。相邻工具要求用户查找目标行后逐项检测；KeyCrash 将输入、冲突确认、软件归因和证据解释收敛为一次短流程。

## Operating Context

- Windows 10/11 桌面应用，首版以 x64 为目标。
- 本地运行、无需账户或网络服务；Release 默认请求管理员权限，以覆盖高完整性全局热键注册者。
- 主窗口是单任务工具，不做导航、仪表盘或默认全量热键列表。
- 目标键只在 KeyCrash 获得焦点并进入等待状态时由本地 UI 捕获；只有用户在已占用结果上选择“定位占用软件”时，才短时安装 x64/x86 `WH_GETMESSAGE` DLL Hook。

## Capabilities and Constraints

- 首版流程固定为：选择修饰键 → 等待一个目标键 → 安全检测 → 已占用时按需定位软件 → 结果。
- 首版可靠回答：当前可注册、已占用或系统保留、不受 `RegisterHotKey` 表达、其他系统错误。
- Windows 公开 API 无法从 `RegisterHotKey` 失败结果直接取得占用进程 PID；归因必须基于目标进程实际取走匹配 `WM_HOTKEY` 的运行时证据。
- 深度定位会通过 `SendInput` 最多触发两次完整组合，并在 `PM_REMOVE` 阶段尽量把匹配消息改为 `WM_NULL`；低级 Hook、Raw Input、驱动或忽略模拟输入的软件仍可能执行动作或无法归因。
- x64/x86 Helper 必须按位数匹配目标进程；Hook 只在单次定位期间存在，完成或超时后立即卸载。
- `Ctrl+Alt+Del` 等安全组合和部分系统保留组合不可捕获或覆盖，应给出明确说明。
- 技术栈为 Rust、Microsoft `windows` crate 和 Slint；不使用 Wails、Vue、Node 或 WebView。

## Brand Commitments

- 产品名：KeyCrash。
- 产品含义：快捷键发生“撞车”时，快速确认问题。
- 中文核心文案：一按，找到快捷键冲突。
- 品牌表达应直接、可信、有一点锋利感，但不能暗示软件会导致系统崩溃。

## Evidence on Hand

- 产品与技术调研：`../docs/research/windows-hotkey-conflict-detector.md`。
- 已确认的竞品痛点：Hotkey Screener 信息密度高、缺少搜索，用户需要手动滚动并找到组合后再逐项检测。
- 当前没有既有 logo、截图、用户数据、性能数据或发布声明；未来工作不得虚构。

## Product Principles

1. 一个组合键就是一次查询，不让用户浏览数据库。
2. 先给可靠结论，再解释技术边界，不猜测占用者。
3. 默认检测安全、本地；进程归因只在用户明确操作后短时安装消息 Hook，不记录普通输入。
4. 一屏完成任务，修饰键使用可关闭 tags，用户只需按一个目标键。
5. 每个状态都告诉用户发生了什么、证据来自哪里，以及下一步能做什么。

## Accessibility & Inclusion

- 完整键盘操作，`Esc` 随时取消捕获。
- 支持 Windows DPI 缩放、高对比度和清晰焦点状态。
- 状态不能只靠颜色表达，必须同时使用图形或文字。
- 中文界面是首版默认语言；快捷键名称使用用户熟悉的 Windows 标记。
