# Windows 全局快捷键占用者归因研究

更新日期：2026-08-30

## 结论先行

用户的质疑成立：KeyCrash 当前的 `RegisterHotKey` 临时注册探测只能回答“可用/已占用”，不能回答“哪个软件占用”，因此还没有达到最初产品目标。错误 1409 只表示热键已注册，并不携带 PID；但这不等于 Windows 上无法做进程归因。[Microsoft 系统错误 1409](https://learn.microsoft.com/en-us/windows/win32/debug/system-error-codes--1300-1699-)

Hotkey Screener 官方明确宣称能对每个热键显示“哪个应用响应”，公开的实现机制是：安装全局消息 Hook DLL、模拟一次热键按压，然后监听热键事件。它也明确承认 Windows 没有受文档支持的直接所有者查询 API。[Hotkey Screener 官方页](https://www.ntwind.com/freeware/hotkey-screener.html)

可复现的受支持链路是：

1. 用 `RegisterHotKey` 先判断组合是否已占用。
2. 对已占用组合，临时安装跨进程 `WH_GETMESSAGE` Hook DLL。
3. 让该组合真实进入系统输入流一次（用户物理按下，或 `SendInput` 合成）。
4. 注册者取走 `WM_HOTKEY` 时，注入其进程的 Hook DLL 看到消息并直接记录 `GetCurrentProcessId()`。
5. 最好在 `PM_REMOVE` 时把目标消息改为 `WM_NULL`，尽量避免注册者执行动作；但低级键盘 Hook、Raw Input 或驱动可能在此之前已经响应，所以无法保证零副作用。

Hotkey Detective 的官方源码证明了这条路线：它安装 `WH_GETMESSAGE` 等全局 DLL Hook，在 Hook 内捕获 `WM_HOTKEY`，再将窗口映射到 PID 和进程路径。[Hook 安装代码](https://github.com/ITachiLab/hotkey-detective/blob/8c10832453c28eefaf60f359f64f2e81a0d0372b/detective/src/Core.cpp#L46-L67)、[捕获 `WM_HOTKEY` 的 Hook](https://github.com/ITachiLab/hotkey-detective/blob/8c10832453c28eefaf60f359f64f2e81a0d0372b/hook/src/HkdHook.cpp#L148-L200)、[PID/路径映射](https://github.com/ITachiLab/hotkey-detective/blob/8c10832453c28eefaf60f359f64f2e81a0d0372b/detective/src/MainWindow.cpp#L74-L84)

因此，之前 KeyCrash 使用 `WH_KEYBOARD_LL` 捕获并阻止组合的方向并不能完成所有者归因。正确观察点不是“谁先看见了键盘事件”，而是“哪个进程的消息队列取到了匹配的 `WM_HOTKEY`”。

## 1. Hotkey Screener 到底能不能返回进程

### 官方承诺与准确语义

能，但准确说法是返回“观察到响应这个热键的应用”，而不是 Windows 提供的权威注册所有者字段。官方页写明：

- 它枚举通过 `RegisterHotKey` 安装的系统级热键。
- 每个热键都有 “Detect application”，用于查看哪个应用响应。
- 检测时安装全局消息 Hook DLL、模拟一次热键按压并监听热键事件。
- 它覆盖 64 位、32 位、普通权限、管理员或 UIAccess 进程。
- 若另一个程序通过低级键盘 Hook 或 Raw Input 抢先抑制热键，界面可能呈现第二个程序的动作，而不是原注册者，并且检测可能真正执行危险操作。[Hotkey Screener 官方说明](https://www.ntwind.com/freeware/hotkey-screener.html)

这一区分很重要：Hotkey Screener 确实比当前 KeyCrash 多做了进程归因，但它自己也没有声称结果在所有输入链下都是“注册表式的绝对真相”。

### 官方发行包提供的实现旁证

对 [Hotkey Screener 官方 ZIP](https://www.ntwind.com/download/HotkeyScreener.zip) v1.2（ReadMe 标注 2026-08-04）的只读检查结果：

- 包内同时包含 `hkscr.exe`、`hkscr.dll`、`hkscr64.exe`、`hkscr64.dll`，即 x86/x64 两套 EXE 与 DLL。
- 四个 PE 文件均有 NTWIND LLC 的有效 Authenticode 签名。
- x64 EXE 清单为 `asInvoker + uiAccess=true`；x86 EXE 为 `asInvoker + uiAccess=false`。
- 官方 ReadMe 要求把全部 EXE/DLL 放到 `%ProgramFiles%` 下再启动，以取得 UIAccess。

这与官方网页描述的“双位数 + UIAccess”覆盖一致。由于 Hotkey Screener 未开放源码，不能仅凭文件布局断言其所有内部 IPC 细节；最可靠的机制边界仍以其官方说明为准。

## 2. Hotkey Detective 的开源实现说明了什么

Hotkey Detective 的 README 明确要求：选择 x86 或 x64 版本、以管理员运行、然后由用户按下被占用的真实组合，工具显示对应进程路径。它还要求 x64 Windows 上在未命中时尝试另一位数版本。[Hotkey Detective README](https://github.com/ITachiLab/hotkey-detective/blob/8c10832453c28eefaf60f359f64f2e81a0d0372b/README.adoc#L10-L25)、[位数与管理员 FAQ](https://github.com/ITachiLab/hotkey-detective/blob/8c10832453c28eefaf60f359f64f2e81a0d0372b/README.adoc#L50-L68)

它的具体链路是：

1. 主程序调用 DLL 导出的 `setupHook`，安装 `WH_GETMESSAGE`、`WH_CALLWNDPROC`、`WH_SYSMSGFILTER` 三类全局 Hook。[Core.cpp](https://github.com/ITachiLab/hotkey-detective/blob/8c10832453c28eefaf60f359f64f2e81a0d0372b/detective/src/Core.cpp#L46-L67)
2. DLL 的 `WH_GETMESSAGE` 回调查看目标进程通过 `GetMessage`/`PeekMessage` 取出的消息；遇到 `WM_HOTKEY` 时把 `HWND` 与原始 `lParam` 发回主窗口。[HkdHook.cpp](https://github.com/ITachiLab/hotkey-detective/blob/8c10832453c28eefaf60f359f64f2e81a0d0372b/hook/src/HkdHook.cpp#L148-L200)
3. 主窗口用 `GetWindowThreadProcessId` 从 `HWND` 得到 PID，再用 `OpenProcess` 和 `QueryFullProcessImageName` 得到路径。[MainWindow.cpp](https://github.com/ITachiLab/hotkey-detective/blob/8c10832453c28eefaf60f359f64f2e81a0d0372b/detective/src/MainWindow.cpp#L74-L84)、[Core.cpp](https://github.com/ITachiLab/hotkey-detective/blob/8c10832453c28eefaf60f359f64f2e81a0d0372b/detective/src/Core.cpp#L69-L80)
4. `WM_HOTKEY` 的 `lParam` 自带修饰键和虚拟键，源码据此还原组合。[KeySequence.cpp](https://github.com/ITachiLab/hotkey-detective/blob/8c10832453c28eefaf60f359f64f2e81a0d0372b/detective/src/KeySequence.cpp#L117-L125)

其 README 对原理的描述也很直接：DLL Hook 进入每个可访问进程，等待某个进程收到热键命令；它不暴力触发所有组合，而是要求用户按下目标组合。[Hotkey Detective 原理说明](https://github.com/ITachiLab/hotkey-detective/blob/8c10832453c28eefaf60f359f64f2e81a0d0372b/README.adoc#L27-L48)

### 不应原样照抄的实现缺口

Hotkey Detective 把 `MSG.hwnd` 发回主程序再反查 PID，但 `RegisterHotKey(hWnd=NULL, ...)` 会把 `WM_HOTKEY` 发到注册线程队列，此时消息的 `hwnd` 可以是 `NULL`。[RegisterHotKey](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey)、[GetMessage 对线程消息的说明](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmessage)

KeyCrash 的 Hook DLL 应直接在被注入进程内记录 `GetCurrentProcessId()`，不要依赖 `MSG.hwnd`。这样窗口绑定和线程绑定的 `RegisterHotKey` 都能覆盖。`HWND` 只作为辅助证据。

## 3. 为什么必须真实触发一次组合

`RegisterHotKey` 的失败结果只表示组合通常已经被其他热键注册，API 没有 owner/PID 输出参数。[RegisterHotKey 返回值与冲突说明](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey)

Windows 只有在组合被按下并匹配注册项后，才把 `WM_HOTKEY` 放到注册窗口或注册线程的消息队列；`WM_HOTKEY` 文档也明确说该消息在用户按下已注册热键时产生。[RegisterHotKey 消息投递](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey)、[`WM_HOTKEY`](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-hotkey)

`WH_GETMESSAGE` 正好能在目标应用的 `GetMessage`/`PeekMessage` 返回消息之前观察消息队列；Hook 回调拿到完整 `MSG`，而且文档允许检查或修改它。[Hooks Overview](https://learn.microsoft.com/en-us/windows/win32/winmsg/about-hooks)、[GetMsgProc](https://learn.microsoft.com/en-us/windows/win32/winmsg/getmsgproc)

所以当前 KeyCrash 的“点 Ctrl/Alt 标签，开始后只在本窗口按 A”只构造出了一个 `Ctrl+Alt+A` 数据对象，没有把完整组合送入系统输入流，也就不会让注册者收到 `WM_HOTKEY`。要归因必须二选一：

- 像 Hotkey Detective：Hook 就绪后让用户物理按完整组合。
- 像 Hotkey Screener：Hook 就绪后用 `SendInput` 合成完整的按下/释放序列。

`SendInput` 会把事件串行插入系统键鼠输入流，但受 UIPI 限制，只能向相同或更低完整性级别注入；低级键盘 Hook 还能通过 `LLKHF_INJECTED` 判断事件是否为合成输入，因此目标软件可能主动忽略它。[SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)、[KBDLLHOOKSTRUCT 注入标志](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-kbdllhookstruct)

### 可以降低但不能消灭副作用

KeyCrash 的 `WH_GETMESSAGE` Hook 可只在 `code == HC_ACTION`、`wParam == PM_REMOVE`、`message == WM_HOTKEY` 且 `lParam` 精确匹配目标组合时：

1. 先记录 PID/TID/组合；
2. 把 `MSG.message` 改为 `WM_NULL`；
3. 继续调用 `CallNextHookEx`。

`GetMsgProc` 官方文档允许 Hook 检查或修改消息。这能阻止多数标准 `RegisterHotKey` 注册者继续分派 `WM_HOTKEY`，但不是安全承诺：低级 Hook、Raw Input、驱动或另一个进程可能在消息队列之前已经响应。Hotkey Screener 官方也因此要求检测前保存数据。[GetMsgProc](https://learn.microsoft.com/en-us/windows/win32/winmsg/getmsgproc)、[Hotkey Screener 风险说明](https://www.ntwind.com/freeware/hotkey-screener.html)

## 4. UAC、32/64 位与 UIAccess 的准确边界

### UAC / 完整性级别

UIPI 会阻止低权限进程向更高完整性进程发送消息或安装 Hook；同完整性级别不受这条限制。[Microsoft UIAccess/UIPI 说明](https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-server-2012-r2-and-2012/jj852244(v=ws.11))

- KeyCrash 普通启动时，可覆盖同桌面的普通用户进程，但不能保证 Hook 进入“以管理员运行”的高完整性进程。
- 管理员启动后，Hook 进入普通与高完整性同桌面进程的覆盖会更完整；Hotkey Detective 因此官方要求管理员运行。[Hotkey Detective README](https://github.com/ITachiLab/hotkey-detective/blob/8c10832453c28eefaf60f359f64f2e81a0d0372b/README.adoc#L20-L25)
- 但“默认 UAC”本身不会让 `RegisterHotKey` 返回 PID；必须同时实现跨进程 `WH_GETMESSAGE` DLL Hook。
- 即使管理员运行，也不代表可以进入 SYSTEM、受保护进程、其他会话或 UAC 安全桌面。

对 KeyCrash，生产版更合理的是普通 GUI + 按需提升的短生命周期归因 Helper；但如果目标是先验证机制，给归因原型使用 `requireAdministrator` 清单是合理的简化，并且符合用户明确要求。提升应只覆盖归因 Helper，而不是让 Slint GUI 永久高权限运行。

### 32/64 位

微软明确规定 32 位 DLL 不能注入 64 位进程，64 位 DLL 不能注入 32 位进程；要完整覆盖 64 位 Windows，必须由 32 位进程安装 32 位全局 Hook、64 位进程安装 64 位全局 Hook，两套 DLL 名称还必须不同，并持续泵消息。[SetWindowsHookEx 位数说明](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowshookexw)

位数不匹配时，系统可能把回调放到安装 Hook 的进程线程执行；这时在回调里直接调用 `GetCurrentProcessId()` 会误报 Helper 自己。因此 KeyCrash 必须：

- 同时运行 x86/x64 Hook Host 与匹配 DLL；
- 只接受“DLL 已确认在外部目标进程内执行”的记录；
- 过滤两个 Host 和 KeyCrash 自身 PID；
- 两个 Host 都要保持消息循环，探测结束立即 `UnhookWindowsHookEx`。

### UIAccess

UIAccess 不是“免 UAC 的万能管理员模式”。微软当前文档给出的边界是：

- 使用 UIAccess 必须有 Authenticode 签名、受信任的安全安装目录（如 Program Files）和 `uiAccess=true` 清单。
- 非管理员用户启动 UIAccess 程序只能得到 medium+，仍不能跨到右键“以管理员运行”的 high IL。
- 管理员用户启动 UIAccess 程序可到 high IL；仍不能访问 SYSTEM IL。
- 微软明确说 UIAccess 应用于辅助技术，不应被普通应用滥用。[Security Considerations for Assistive Technologies](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-securityoverview)

`SetWindowsHookEx` 还说明：全局窗口 Hook DLL 默认不会加载进 Windows Store/UWP 进程，除非由 UIAccess 进程安装。[SetWindowsHookEx 的 Store app 限制](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowshookexw)

Hotkey Screener 选择了签名 + Program Files + UIAccess 的发布路线；KeyCrash 作为非辅助技术工具不宜直接照搬。第一版应选择普通/管理员 Helper，并把 Store/UWP 特例诚实显示为“可能无法归因”。

### 桌面边界

全局 Hook 只覆盖调用线程所在的同一个 Desktop，且必须把跨进程 Hook 过程放在 DLL 中。微软也提醒全局 Hook 会影响所有同桌面应用，应只用于专用/调试场景并尽快卸载。[Hooks Overview](https://learn.microsoft.com/en-us/windows/win32/winmsg/about-hooks)、[SetWindowsHookEx](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowshookexw)

## 5. 当前 Rust + Slint 项目的最小可验证原型

### 推荐进程布局

```text
keycrash.exe (x64, Slint, asInvoker)
  ├─ keycrash-attrib64.exe (x64, 原型期 requireAdministrator)
  │    └─ keycrash-hook64.dll (x64, WH_GETMESSAGE)
  └─ keycrash-attrib32.exe (x86, 原型期 requireAdministrator)
       └─ keycrash-hook32.dll (x86, WH_GETMESSAGE)

共享协议：命名共享内存 + 命名 Event（目标 PID/TID/HWND/lParam/架构/时间戳）
```

主 GUI 不应该自己加载 x86 DLL。x64/x86 Host 各自加载匹配位数 DLL、调用 `SetWindowsHookExW(WH_GETMESSAGE, ..., thread_id=0)` 并保持原生 Win32 消息循环。Hook DLL 中不得依赖 Slint 或 Rust 异步运行时，只做固定大小的共享内存写入、事件通知、精确消息过滤和 `CallNextHookEx`。

IPC 推荐共享内存 + Event，而不是让所有被注入进程向高完整性主窗口发送自定义消息；这样更容易跨普通/管理员完整性边界，也能避免 Hook 回调里分配内存或做复杂 IPC。

### 单次检测状态机

1. 用户继续按现有方式选择 Ctrl/Alt/Shift 标签，开始后只在 KeyCrash 窗口输入目标键。
2. 先保留当前 `RegisterHotKey` 临时探测：若成功，立即释放并显示“可用”，无需 Hook。
3. 若返回 1409，显示第二阶段：“定位占用软件会在系统中触发一次完整组合，可能执行对应动作”。用户确认后启动两个归因 Host。
4. 两个 Host 都报告 Hook ready 后，主进程或高完整性 Host 用 `SendInput` 依次发送修饰键按下、目标键按下/释放、修饰键逆序释放。发送前先确认这些键当前没有被物理按住；无论成功/超时都执行完整 key-up 清理。[SendInput 键盘状态提醒](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)
5. 匹配位数的目标进程 Hook 在 `PM_REMOVE + WM_HOTKEY + 精确 lParam` 时写入当前 PID/TID，并将消息改成 `WM_NULL`（best effort）。
6. 主进程等待短超时（例如 1 秒），收到记录后用 `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` + `QueryFullProcessImageNameW` 读取路径；路径读取失败时仍显示 PID，不猜测前台进程。[OpenProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-openprocess)、[QueryFullProcessImageNameW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-queryfullprocessimagenamew)
7. 立即卸载两个 Hook、结束 Host、清空共享对象。显示“注册者已确认”“观察到其他响应者”或“已占用但未观察到 `WM_HOTKEY`”三种不同结果，不把后两者伪装成确定 owner。

### Rust 工程变化的最小集合

- 保留现有 `src/detector.rs` 的快捷键解析、格式化和 `RegisterHotKey` 探测。
- 新增一个无 UI 的共享协议 crate，结构只包含 POD 字段。
- 新增 x86/x64 Hook Host bin target；CI/本地构建同时安装 `i686-pc-windows-msvc` 与 `x86_64-pc-windows-msvc`。
- 新增两个 `cdylib` Hook target，产物显式命名 `keycrash-hook32.dll` / `keycrash-hook64.dll`。
- `windows` crate 至少需要 `Win32_UI_WindowsAndMessaging`、`Win32_System_Threading`、`Win32_System_Memory`、`Win32_System_LibraryLoader` 等功能；现有 `Win32_UI_Input_KeyboardAndMouse` 可继续提供 `SendInput` / `RegisterHotKey`。
- 构建脚本为两个 Host 嵌入管理员清单；GUI 仍用 `asInvoker`。

### 原型验收矩阵

不要先拿 QQ 当唯一测试样本。先写四个可控 owner fixture：

| Fixture | 位数 | 注册方式 | 权限 | 预期 |
|---|---:|---|---|---|
| A | x64 | `RegisterHotKey(HWND, ...)` | 普通 | 返回正确 PID，动作被 best-effort 吞掉 |
| B | x64 | `RegisterHotKey(NULL, ...)` | 普通 | 仍返回正确 PID，证明不依赖 `MSG.hwnd` |
| C | x86 | 两种任选 | 普通 | 由 x86 DLL 返回正确 PID |
| D | x64 | 两种任选 | 管理员 | 普通模式失败/管理员 Helper 成功 |

随后再测 QQ `Ctrl+Alt+A`：

- 若收到 QQ 的匹配 `WM_HOTKEY`，显示 QQ 路径并记录截图是否仍触发。
- 若 1409 但无任何匹配 `WM_HOTKEY`，说明 QQ 或中间层可能用低级 Hook/Raw Input 抑制或忽略合成输入；结果必须显示“占用已确认，但注册者未观察到”，不能猜 QQ。

## 6. 仍然检测不到或可能误归因的场景

| 场景 | 结果边界 | 依据/处理 |
|---|---|---|
| 标准 Win32 `RegisterHotKey`，同桌面、位数和完整性均覆盖 | 可高置信度定位 | 匹配 `WM_HOTKEY` 的进程内 Hook PID |
| `RegisterHotKey(NULL, ...)` 线程热键 | 可定位，但必须用进程内 PID | `MSG.hwnd` 可能为 NULL，不能照抄 Hotkey Detective 的 HWND 反查 |
| 应用内部快捷键（例如前台浏览器 Ctrl+T） | 不在能力范围 | 没有系统级 `RegisterHotKey` / `WM_HOTKEY`；Hotkey Detective README 也明确不支持[官方 FAQ](https://github.com/ITachiLab/hotkey-detective/blob/8c10832453c28eefaf60f359f64f2e81a0d0372b/README.adoc#L60-L68) |
| 低级键盘 Hook、Raw Input、DirectInput、驱动层热键 | 可能探测为可用，或没有 owner 事件 | 不产生 `WM_HOTKEY`；需要完全不同的观测方案 |
| A 用 `RegisterHotKey` 注册，B 用低级 Hook/Raw Input 抢先抑制 | 注册者可能收不到；用户看到 B 动作 | Hotkey Screener 官方列出的最常见失败模式[官方说明](https://www.ntwind.com/freeware/hotkey-screener.html) |
| 软件拒绝 `LLKHF_INJECTED` 输入 | 合成触发无结果，物理触发可能成功 | 低级 Hook 可识别注入标志[KBDLLHOOKSTRUCT](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-kbdllhookstruct) |
| 普通 KeyCrash 对管理员 owner | 无结果 | UIPI 阻止低权限进程安装高权限 Hook；提供管理员 Helper 重试 |
| 缺少 x86 或 x64 Hook | 漏报或把 Helper 误当 owner | 必须双位数匹配[SetWindowsHookEx](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowshookexw) |
| UWP/Store app | 可能漏报 | 非 UIAccess 全局 Hook DLL 不加载进 Store 进程[SetWindowsHookEx](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowshookexw) |
| SYSTEM、受保护进程、其他桌面/会话、安全桌面 | 不保证 | Hook 只覆盖同桌面，管理员也不是 SYSTEM |
| Win/F12/Ctrl+Alt+Delete 等系统保留组合 | 可能无普通注册者 | 继续使用 KeyCrash 的系统规则单独解释，不冒充进程 owner |
| 目标进程未及时泵消息、退出或卡死 | 超时 | 显示“已占用但本次未观察到”，允许物理触发/管理员重试 |
| 多个观察记录 | 不应简单取第一个 | 精确比对 `lParam`、去重 PID，并区分注册者消息与上游响应者 |

## 7. 能否直接复用 Hotkey Detective 代码

可以在满足 GPL-3.0 的前提下复用，但不建议直接复制到当前 KeyCrash：

- Hotkey Detective 仓库明确标注 GPL-3.0，仓库附带完整 GPL-3.0 许可证。[仓库许可证](https://github.com/ITachiLab/hotkey-detective/blob/8c10832453c28eefaf60f359f64f2e81a0d0372b/LICENSE)
- 若复制、修改或链接其 Hook/Core 代码并分发组合程序，GPL-3.0 第 5、6 节通常要求整个组合工作在兼容 GPL 的条款下发布，并向接收二进制的用户提供 Corresponding Source、构建/安装脚本与许可证通知。[GPL-3.0 正文](https://www.gnu.org/licenses/gpl-3.0.html.en)、[GNU GPL FAQ：链接与组合工作](https://www.gnu.org/licenses/gpl-faq.en.html#LinkingWithGPL)
- 单纯阅读其机制、依据公开 Win32 API 独立重写，不会自动把 KeyCrash 变成 GPL 项目；应避免逐段翻译其表达性代码，保留本研究作为设计来源记录。
- 如果 KeyCrash 本来就愿意整体采用 GPL-3.0，则可复用，但仍应修正线程热键 `hwnd=NULL`、双位数 Host、可靠卸载与 IPC 等问题。

Hotkey Screener 只有免费使用条款和“as is”免责说明，官方页底部标注 All rights reserved，未提供源码或允许再分发/链接其 DLL 的许可证。因此不能把它的 EXE/DLL 直接捆进 KeyCrash；只能依据官方公开机制独立实现。[Hotkey Screener 官方页](https://www.ntwind.com/freeware/hotkey-screener.html)、[官方发行包](https://www.ntwind.com/download/HotkeyScreener.zip)

以上是工程许可证判断，不替代正式法律意见。

## 8. 对 KeyCrash 的产品建议

KeyCrash 应明确提供两层能力，而不是二选一：

1. **安全探测**：保持当前标签 + 本地目标键输入，只调用 `RegisterHotKey`，无真实组合、副作用最小；输出“可用/已占用”。
2. **深度定位**：只在已占用时出现，启动双位数 `WH_GETMESSAGE` Helper 并真实触发一次；输出进程路径与证据等级，提前警告可能执行动作。

对用户当前反馈，正确回应不是继续辩解“Windows 不提供 PID”，而是承认当前只完成第一层，并实现第二层。UAC 确实与覆盖管理员 owner 有关，但只有“UAC + 双位数进程内消息 Hook + 一次真实触发”组合起来，才能接近 Hotkey Screener 的能力。
