# macOS 快捷键机制与测试依据

调研日期：2026-10-13。本文用于确定测试范围，不把文档推导当作实机通过记录。当前产品实现见 [macOS 开发与验收](MACOS.md)，实际接收证据与能力缺口见[逐项实测](MACOS-SHORTCUT-TESTS.md)。

用户已明确产品只查全局快捷键。应用内菜单、local monitor、Qt 窗口快捷键仅保留为机制区分的背景资料，不是待实现功能。当前全局过滤路径的进一步研究与实测见[全局过滤归因](MACOS-GLOBAL-ATTRIBUTION.md)。

## 结论

macOS 的“快捷键”至少涉及系统热键注册、原始事件监听或过滤、应用内部命令、系统功能和设备输入几条不同路径。不能只换一批字母键测试 Carbon，就宣称覆盖全部常用软件。应先测试机制，再测试真实软件配置。

KeyCrash v0.2.2 的归因条件是：主动发送带唯一标记的组合，在 annotated-session 的只读 event tap 中观察 **system-defined、subtype 6、标记相同、有效目标 PID**。这个条件已用于 Carbon 路由的实机验证；Apple 公开文档没有承诺所有快捷键处理机制都会产生这种事件。没有命中只能说明本次没有观察到符合条件的接收证据。[产品实现](../src/macos/owner_probe.rs)

`eventTargetUnixProcessID` 表示该事件的目标进程，`eventSourceUserData` 只是发送者提供的 64 位数据。它们都不是“所有监听者”或“谁吞掉了快捷键”的查询 API。把普通 keyDown 的前台目标 PID 当作全局热键占用者会误报。[Apple：目标 PID](https://developer.apple.com/documentation/coregraphics/cgeventfield/eventtargetunixprocessid)、[Apple：用户数据](https://developer.apple.com/documentation/coregraphics/cgeventfield/eventsourceuserdata)

## 机制对照

| 机制 | 向系统登记特定组合 | 作用域与冲突语义 | 监听/注册者所需权限 | 当前 subtype 6 归因的预期 |
| --- | --- | --- | --- | --- |
| Carbon `RegisterEventHotKey`，options=0 | 是 | 跨应用非独占注册；同组合可有多个应用，不能以一次临时注册成功证明无注册者 | 单纯注册通常不弹辅助功能授权 | 已有此类实机证据；共享、多接收者仍需独立测试 |
| Carbon `kEventHotKeyExclusive`，options=1 | 是 | 独占注册存在时抑制非独占通知；同组合第二个独占者应得到冲突 | 同上 | 应验证正确 PID、共享受抑制、释放后恢复 |
| `CGEventTap` listen-only | 否，掩码只指定事件类型 | 可后台观察，不能改写或阻止原事件；多个监听者可并存 | 现代 macOS 的键盘全局监听涉及输入监控授权 | 没有 subtype 6 生成保证；监听回调收到不等于产品能归因 |
| `CGEventTap` active filter | 否 | 可放行、改写或返回 NULL 吞掉事件；顺序及插入位置影响后续接收 | 辅助功能授权；须以实际创建结果确认 | 无保证；吞掉事件可能让后续 Carbon 接收证据消失 |
| `NSEvent` global monitor | 否 | 复制发往其他应用的事件，不包括自身；不能阻止传播 | 全局键盘监听受隐私权限控制；不能只参考旧文档的权限名称 | 无保证，不应把监听者列为确定占用者 |
| `NSEvent` local monitor | 否 | 仅本应用事件，可返回 nil；通常随前台焦点发生 | 观察自身输入不需要全局输入监控授权 | 应作应用内阴性对照 |
| `NSMenuItem.keyEquivalent`、`NSResponder`、Qt `QShortcut` | 不作 Carbon 全局组合注册 | 当前窗口/应用的命令派发，不排斥别的应用配置相同组合 | 自身输入处理无需全局权限 | 不应按全局注册者归因 |
| 系统 symbolic shortcuts | `CopySymbolicHotKeys` 读取系统组合信息 | 系统设置证据，不是运行中所有者表，也不是所有应用菜单表 | 读取本身不需要定位授权 | 每个系统功能待实测；设置匹配不保证接收 PID 或排除第三方 |
| 媒体、亮度、Fn/Globe、IOHID 设备处理 | 可能绕开普通组合注册 | 有专用事件、设备回调或系统处理；不能用普通 F 键替代 | 随监听/过滤/设备访问方式变化 | 媒体 subtype 8 不符合产品 subtype 6 条件；普通键测试不能覆盖 |

下文给出各行的原始依据。表中权限是被测机制自身的权限；KeyCrash 为主动发送组合而取得的辅助功能权限是另一件事。

### Carbon：非独占与独占要分开测

本轮主执行器读取了实机 Apple SDK 的 `CarbonEvents.h`：

```text
/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/System/Library/Frameworks/Carbon.framework/Frameworks/HIToolbox.framework/Headers/CarbonEvents.h
```

`EventHotKeyOptions` 注释描述默认非独占通知可发给所有注册者；独占有效时抑制非独占通知，已有独占时仍允许新增非独占。`RegisterEventHotKey` 的 Discussion 明确同一应用不能重复注册相同组合，但不同应用可以。需要保留一个文档差异：选项注释的冲突措辞较宽，而函数 Result 将 `eventHotKeyExistsErr` 关联到另一个进程的同组合独占注册。不能据此把所有共享者视为独占探测必然失败；四种注册顺序应以实测补齐。

常见封装确实走这一机制：KeyboardShortcuts 的 `HotKeyCenter.registerEventHotKey`、MASShortcut 的 `MASHotKey`、Chromium 普通全局组合均传 `inOptions=0`。这说明测试共享模式有实际意义，但不证明任何未查源码的闭源软件使用同一路径。[KeyboardShortcuts 源码](https://github.com/sindresorhus/KeyboardShortcuts/blob/772133d9dbe800fdac0473226822994c5c162c58/Sources/KeyboardShortcuts/HotKey.swift#L252-L270)、[MASShortcut 源码](https://github.com/cocoabits/MASShortcut/blob/6f2603c6b6cc18f64a799e5d2c9d3bbc467c413a/Framework/Monitoring/MASHotKey.m#L11-L26)、[Chromium 源码](https://github.com/chromium/chromium/blob/9cd4b9f5fcc8e08ebd360f086b07f839682e982a/ui/base/accelerators/global_accelerator_listener/global_accelerator_listener_mac.mm#L232-L239)

KeyboardShortcuts 明示无需权限弹窗；其菜单打开期间还会暂停 Carbon 注册并改走本地原始事件。故同一库也不能只测常态而完全忽略菜单跟踪状态。[库说明](https://github.com/sindresorhus/KeyboardShortcuts#faq)、[模式切换源码](https://github.com/sindresorhus/KeyboardShortcuts/blob/772133d9dbe800fdac0473226822994c5c162c58/Sources/KeyboardShortcuts/HotKey.swift#L314-L363)

### Quartz event tap：监听、拦截与组合注册不同

Apple 将位置分为 HID 进入 WindowServer、进入登录会话、已标注目标应用的会话三个阶段；还提供针对指定进程的 tap。`listenOnly` 与主动 filter 必须分别测试。头插/尾插决定同一位置的先后，不同位置也不能互换。[位置定义](https://developer.apple.com/documentation/coregraphics/cgeventtaplocation)、[创建接口](https://developer.apple.com/documentation/coregraphics/cgevent/tapcreate(tap:place:options:eventsofinterest:callback:userinfo:))

Apple 的旧 `CGEventTapCreate` 页面含早期 OS X 的权限说明；现代权限应结合 WWDC 2019：被动全局键盘监听受输入监控控制，改写事件和发送模拟输入涉及辅助功能授权。被测 tap 创建返回 NULL、事件掩码被权限裁剪、回调超时被禁用，必须记录成条件失败，不能算“没有处理者”。实际系统版本和授予的身份仍需记录。[Apple WWDC 2019](https://developer.apple.com/videos/play/wwdc2019/701/)

`CGGetEventTapList` 可取得已安装 tap 及其进程、位置、选项、事件掩码等信息。它没有每个回调内部的按键判断逻辑，不能凭“监听 keyDown”确定该进程占用了 `⌃⌥A`。列出它们最多形成候选列表；证据级别与已观察到特定组合接收者不同。[Apple：tap 列表](https://developer.apple.com/documentation/coregraphics/cggeteventtaplist(_:_:_:))、[Apple：tap 信息](https://developer.apple.com/documentation/coregraphics/cgeventtapinformation)

### AppKit 与应用内命令

global monitor 收到其他应用事件的副本，不能修改或取消；local monitor 只看本应用派发，可返回修改后事件或 nil。要覆盖自身与其他应用，必须两种监听同时存在。监听自身有事件、后台监听没事件，不一定是夹具坏了，可能是两者作用域本来不同。[Apple：Monitoring Events](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/MonitoringEvents/MonitoringEvents.html)

菜单等价键先经窗口视图的 `performKeyEquivalent:`，再到菜单；普通按键还会走 first responder。`⌘C`、`⌘V` 等可以在许多应用中同时存在，没有全局独占者的含义。Qt `QShortcut` 默认 `WindowShortcut`，其 `ApplicationShortcut` 也限本应用窗口处于活动状态；不要把名字中的 application 解读成整个系统全局。[Apple：按键派发](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/HandlingKeyEvents/HandlingKeyEvents.html)、[Qt QShortcut](https://doc.qt.io/qt-6/qshortcut.html)、[Qt ShortcutContext](https://doc.qt.io/qt-6/qt.html#ShortcutContext-enum)

### 系统设置、媒体键和设备输入

`CopySymbolicHotKeys` 返回组合键码、修饰键和启用状态。KeyboardShortcuts 也用此接口做系统组合冲突提示，并归一化功能键/导航键合成的 Fn 位；这些字段不含任意第三方运行进程的确定归属。系统设置匹配应与主动归因结果并存。[系统组合读取源码](https://github.com/sindresorhus/KeyboardShortcuts/blob/772133d9dbe800fdac0473226822994c5c162c58/Sources/KeyboardShortcuts/HotKey.swift#L502-L527)、[修饰位处理源码](https://github.com/sindresorhus/KeyboardShortcuts/blob/772133d9dbe800fdac0473226822994c5c162c58/Sources/KeyboardShortcuts/Shortcut.swift#L91-L111)

Chromium 对播放/上一首/下一首使用 session active tap，监听 system-defined 并筛选 subtype 8，命中后可以停止传播。Apple 开源头文件也定义媒体、音量、亮度等不同特殊键，以及 `NX_SUBTYPE_AUX_CONTROL_BUTTONS=8`。这与当前产品要求的 subtype 6 明确不同，不能把“F8 已测”说成“播放键已测”。[Chromium 媒体键实现](https://github.com/chromium/chromium/blob/9cd4b9f5fcc8e08ebd360f086b07f839682e982a/ui/base/accelerators/media_keys_listener_mac.mm)、[Apple 特殊键定义](https://github.com/apple-oss-distributions/IOHIDFamily/blob/777ccd9698845aadf711e32d843c8c9b777431d9/IOHIDSystem/IOKit/hidsystem/ev_keymap.h)、[Apple 事件子类型](https://github.com/apple-oss-distributions/IOHIDFamily/blob/777ccd9698845aadf711e32d843c8c9b777431d9/IOHIDSystem/IOKit/hidsystem/IOLLEvent.h#L157-L176)

Electron 的 `globalShortcut` 可在后台工作，但普通组合与媒体组合不能视为统一后端；官方对媒体注册另列辅助功能要求，`isRegistered` 也只回答本应用是否注册，不能查询其他应用归属。[Electron globalShortcut](https://www.electronjs.org/docs/latest/api/global-shortcut)

IOHID 还可以在设备层接收或独占设备输入；Apple 的 TN2150 专门区分设备 seize、event tap、键状态轮询。普通 `CGEventPost` 的合成输入不能被当作实际 HID 设备回调的完整验证。安全输入也可能阻止拦截者收到键盘事件；测试应记录 `IsSecureEventInputEnabled`，不能为了得到结果关闭用户的安全输入。[Apple TN2150](https://developer.apple.com/library/archive/technotes/tn2150/_index.html)

## 逐项实测矩阵

优先使用独立 Objective-C 小夹具，避免装完整 Electron/Qt 工具链来重复验证同一个底层接口。通过机制夹具后，再用现有真实应用验证；夹具结果不自动外推到未测试的库版本、沙盒环境、菜单状态、键盘布局或 macOS 版本。

每条至少记录：系统/架构、夹具 PID、机制与选项、前台 PID、组合键码/修饰位、权限、注册返回值、夹具接收次数、产品结果及 PID/路径、退出/注销后的对照。不能只比较产品是否显示“已注册”。

| 优先级 | 项目 | 夹具与动作 | 判定重点 |
| --- | --- | --- | --- |
| P0 | Carbon 独占 | 独立 owner options=1，产品检测、定位，注销后重测 | 冲突与 PID/路径均正确；注销后不再归因 |
| P0 | Carbon 非独占 | options=0，单 owner 与双 owner | 即使产品可临时注册也能继续定位；接收者集合与夹具日志对应 |
| P0 | Carbon 混合 | 顺序测试共享→独占、独占→共享、独占→独占 | 分清注册成功与通知被抑制；独占注销后共享恢复 |
| P0 | 三位置 passive tap | HID/session/annotated × listenOnly | 创建成功、夹具收到、前台仍收到；产品未归因要明确记录能力缺口 |
| P0 | 三位置 active tap | 三位置 × pass/drop，仅目标组合返回 NULL | 对照前台与后续监听是否收到，不能把监听 PID 或前台 PID 当确定归属 |
| P0 | Carbon + filter 交叉 | 已注册共享 owner 上游增加目标 drop filter | 注册存在但动作被拦截时，观察归因是否消失；移除 filter 后恢复 |
| P0 | NSEvent global | 夹具后台，另一安全窗口前台；再切自身前台 | 只看其他应用事件；无确定归因时不能声称组合无人使用 |
| P0 | NSEvent local | 夹具前台/后台 × pass/nil | 前台才按自身派发处理；作为非全局注册阴性对照 |
| P0 | 菜单等价键 | 测试菜单绑定安全组合，前台触发、后台不触发 | 不误报为全局独占；禁用菜单或改变 responder 后行为随之变化 |
| P0 | QQ 用户组合 | 配置界面确认 `⇧⌘F`、`⌃⌘A`，各触发一次后取消截图界面 | 产品显示 QQ PID/路径；不推测闭源注册方式；不把界面有配置视为接收证据 |
| P1 | 系统常用组合 | `⌘Space`、`⌃Space`、`⇧⌘4`、`⇧⌘5`、`⌃↑`、`⌃↓`、`⌘Tab` | 先确认本机启用状态；分别记录系统设置命中、实际动作、产品归因，不要求每项都有可见 app PID |
| P1 | 应用内常用组合 | 隔离测试窗口的 `⌘C`、`⌘V`、`⌘F`、`⌘,` | 本应用命令确实触发，同时产品不伪造全局所有者；粘贴只用无敏感内容的测试剪贴板 |
| P1 | 布局与修饰键 | 字母/数字/空格/方向/F 键；Control、Option、Shift、Command 代表组合 | 物理键码、显示标签、合成 Fn 位一致；包含中英文输入源切换后复测 |
| P1 | 菜单跟踪 | Carbon 封装正常态与菜单展开态 | 菜单期间换用 local 路径的库，不能凭正常态结果宣称覆盖 |
| P2 | 媒体/亮度/Fn/Globe | 专用事件夹具与真实硬件分别验证 | 与 subtype 6 分开；产品输入不支持时明确标为范围外，不能算通过 |
| P2 | IOHID 设备路径 | 有真实硬件样本再测设备回调；本轮不抢占用户键盘设备 | 合成 CGEvent 未到设备回调不构成实现失败或通过证据 |

系统组合的预期功能来自 Apple 的快捷键列表；本机用户可修改映射，因此测试前确认实际设置。`⇧⌘3` 会立即生成截图，优先用可取消的 `⇧⌘4/5` 做首轮系统截图样本；若测试 `⇧⌘3`，只清理本次确认新建的文件。注销、关机、强制退出等组合不自动触发。[Apple 常用快捷键](https://support.apple.com/en-us/102650)

夹具应仅识别本次安全组合和唯一标记，输出匹配事件的元数据，不记录其他输入；设超时并在退出时注销热键、移除 monitor/tap、释放模拟修饰键。主动 drop 要同时处理目标键的 down/up，避免把所有输入吞掉。权限不足、夹具未收到、产品未归因是三个不同结果，不能合并为“测试通过”。

## 当前不能由资料证明的部分

- 无公开依据保证任意监听器、菜单快捷键、系统功能或驱动都会生成可用于归因的 subtype 6，也无依据保证标记在任意改写/重发后保留。
- QQ 本次具体版本的闭源实现未查明；只能报告实际接收/定位结果，不根据应用技术栈推断其快捷键 API。
- 已知权限允许下，读取 tap 列表也不能确定某个回调是否匹配目标组合。若后续做候选列表，必须与确认接收者区分。
- 受控 Carbon 测试通过并不意味着全部 macOS 热键机制已支持。上述每项需要独立结果；未测、权限阻塞、当前不支持都应直写。
