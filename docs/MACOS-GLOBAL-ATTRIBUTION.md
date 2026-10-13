# macOS 全局快捷键：事件过滤归因的公开 API 边界

调研日期：2026-10-13。本轮范围仅为全局快捷键；应用内快捷键、local monitor 和菜单不作为功能目标。本文研究 `CGEventTap` 主动过滤/吞键能否精确归因到 PID，区分文档事实、可实测假设与无法保证的结论。机制总览见 [macOS 快捷键机制与测试依据](MACOS-SHORTCUT-RESEARCH.md)。

## 结论与优先方案

后续进展：网络复核与本机私有接口检查找到了新路线。`SLEventTapIDEnable` 的**受控启停对照**已在 M4 自有夹具中定位单个与多个全局过滤进程；详见本文末尾“按 ID 隔离实测”。这需要主动改变 tap 状态，不是下述公开只读查询的突破，也尚未接入正式产品。

**本次查阅的公开 API 没有提供“这个事件被哪个 tap 丢弃”的查询接口。** `CGGetEventTapList` 能识别创建 tap 的进程，但不能读取回调的按键匹配规则或本次返回值。链头/链尾观察可以进一步缩小发生变化的区间；仅靠这些信息，无法对任意第三方主动过滤器保证精确 PID 归因。[Apple：tap 列表](https://developer.apple.com/documentation/coregraphics/cggeteventtaplist(_:_:_:))、[Apple：回调语义](https://developer.apple.com/documentation/coregraphics/cgeventtapcallback)

优先实测 **同一位置的 head/tail 观察 + 两个独立过滤夹具 + 正反对照**，先验证可以可靠报告“本次组合在这段事件链中发生了变化”，再决定是否增加候选进程提示。聚合 latency 只值得做短小的反例实验，不能先作为确定归因算法。单个已知夹具环境可验证具体动作；该环境的唯一性不能外推到用户桌面。

现有产品捕捉带标记的 subtype 6 接收事件是另一种证据：它证明观察到了该次热键路由的接收 PID。全局 filter 返回 NULL 不必产生这样的接收事件。保留已验证的 Carbon 路由归因，同时独立处理过滤链观测失败，不应将监听器列表冒充占用者。[产品实现](../src/macos/owner_probe.rs)

## 公开信息实际表达了什么

| 信息 | 可以支持的结论 | 不支持的结论 |
| --- | --- | --- |
| `tappingProcess` | 该 PID 创建了这个 tap | 该 PID 处理或吞掉了目标组合 |
| `tapPoint`、`options` | 所处 HID/session/annotated 位置，监听或主动过滤能力 | 此次确实拦截、回调具体分支、按键规则 |
| `eventsOfInterest` | 订阅 keyDown/keyUp 等事件类型 | 订阅哪一个字母或修饰组合 |
| `enabled` | 查询瞬间 tap 处于启用状态 | 最近收到了事件、当前权限必然有效、目标回调已运行 |
| `eventTapID` | 标识列表中的 tap，便于对照快照 | 可用它定位任意插入点、获取别人的回调端口或返回值 |
| `min/avg/maxUsecLatency` | tap 的聚合处理耗时信息 | 某个 marker 的处理次数、最后处理时间、返回 NULL、最后吞键 PID |
| 原始事件 `eventTargetUnixProcessID` | 原始事件的投递目标 | 旁路监听者、修改者或吞键者 |
| 事件 `eventSourceUserData` | 调用者设置的 64 位相关标记 | 跨所有过滤器不变的安全身份或不可伪造的因果链 |

字段定义来自 Apple `CGEventTapInformation`/SDK `CGEventTypes.h`；原始事件字段的意义见 [目标 PID](https://developer.apple.com/documentation/coregraphics/cgeventfield/eventtargetunixprocessid)、[用户数据](https://developer.apple.com/documentation/coregraphics/cgeventfield/eventsourceuserdata)。列表公开结构没有“被吞掉”“回调返回值”“目标快捷键”“事件计数”字段。[Apple：tap 信息](https://developer.apple.com/documentation/coregraphics/cgeventtapinformation)

主动 filter 可以返回原事件、修改后的事件、新构造的事件，或者 NULL 删除事件。被动监听返回 NULL 不会影响流。因此，监听器收到了组合，既不等于占用了它，也不等于导致其他应用收不到它。[Apple：CGEventTapCallBack](https://developer.apple.com/documentation/coregraphics/cgeventtapcallback)

`CGEventTapEnable` 接受已有 `CFMachPort`，列表中的整数 `eventTapID` 不是该参数；`CGEventTapProxy` 是回调收到的透明度受限状态，用于从该回调位置发送事件。所查公开 API 未提供把任意外部 tap ID 转为可观察其返回值的接口。本方案不暂停、禁用或改动第三方 tap。[Apple：启停 tap](https://developer.apple.com/documentation/coregraphics/cgevent/tapenable(tap:enable:))、[Apple：proxy](https://developer.apple.com/documentation/coregraphics/cgeventtapproxy)

## head/tail 观察：可做，但需准确陈述

Apple 保证：新 head 放在**同一位置已存在**的 taps 前面，新 tail 放在同一位置已存在的 taps 后面。它没有承诺一个 session 首尾对就包住 HID、annotated 或 per-process 的全部事件处理，也没有防止观察期间其他应用新建 head 或 tail。`CGGetEventTapList` 文档没有给出返回数组按真实执行顺序排列的保证。[Apple：插入位置](https://developer.apple.com/documentation/coregraphics/cgeventtapplacement)、[Apple：事件位置](https://developer.apple.com/documentation/coregraphics/cgeventtaplocation)

一个健康的观察窗口里：

- head 和 tail 均见同一 marker、类型、键码和修饰位：说明匹配事件在两观测点均出现。不能排除旁路监听者触发动作，也不能排除原事件被删除后出现保留标记的替代事件。
- head 有、tail 无：说明窗口内未观察到同标记事件到达 tail。可能是删除，也可能是改写 marker、键码、类型，替换事件、异步延迟，或观察器/权限/链状态失效。
- head 无、tail 有：可能存在从中间位置注入的事件，或被动回调的到达顺序差异；不能依赖回调日志先后作为 WindowServer 的过滤顺序。
- 两者均无：首先判测试条件失败或证据不足，不能报告组合空闲，也不能指定某个驱动/应用为原因。

上述替换/插入可能性不是假想的 API 之外行为：Apple 允许主动回调返回新事件，`CGEventTapPostEvent` 还允许在当前回调之后插入新事件，仅由其后的 taps 观察。[Apple：从 tap 发事件](https://developer.apple.com/documentation/coregraphics/cgevent/tappostevent(_:))

建议链观察同时核对：前后 tap 元数据快照、所有自有 tap 的启用/失效状态、run-loop 正常运行、发送完整 down/up、唯一 marker、相邻无注册安全组合的正对照。只记录匹配本次测试的元数据；不把扩大到全部键盘输入作为默认产品行为。若 marker 消失，不应静默退回“同键码+接近时间”的确定匹配，因为它会把重发或另一次按键混入。

## latency 为什么不能直接给出 PID

Apple 说明每次 `CGGetEventTapList` 调用会将最小/最大 latency 重设到对应平均值；报告的极值反映上次读取或 tap 创建以来的情况。读取本身会影响统计窗口，所以它也不是完全无副作用的事件历史接口。[Apple：latency 窗口](https://developer.apple.com/documentation/coregraphics/cggeteventtaplist(_:_:_:))

可检验假设是：在完全受控、没有其他输入的短窗口里，收到测试事件的夹具，其统计值可能发生可见变化。但即使成立，也只说明某段时间内有统计变化，无法证明回调匹配了组合或返回 NULL：放行与吞键可以执行相同耗时；未变化可能只是新样本等于原平均值或数值精度不足；其他输入会污染窗口。API 不提供样本数、事件 marker 或每次返回结果来消除这些歧义。

不能使用以下规则作为确定归因：

- “最后一个 latency 变化的 tap 就是吞键者”：缺少官方列表排序与事件级相关性保证。
- “耗时最长的进程就是占用者”：放行、日志或调度延迟也能更慢。
- “只有一个候选在列表中，所以必然是它”：快照可能变化，目标也可能被未纳入的其他事件路径处理。
- “探测后只有某个应用到了前台，所以就是注册者”：这是行为相关性，可能由 IPC、异步任务或用户操作引起。

在两个过滤器可以选择相同元数据和处理耗时的情况下，交换哪一个返回 NULL，外部元数据完全可能相同。这是仅靠列表/latency 不能保证唯一答案的反例思路，而不是对当前系统内部实现的断言。

## 开源工具核对

检查了 HotkeyClash 源码提交 `2ce1a5a42db386e67fae8631831de6be28c72a5b`。它的 `ShortcutTester` 在 session 位置安装两个 listen-only taps，按键码与修饰位匹配，等待约 600 ms，再结合应用激活通知形成结果。这提供了一个首尾观测实例，**没有提供每个 tap 的删除 PID**。[实现](https://github.com/Wunderlandmedia/HotkeyClash/blob/2ce1a5a42db386e67fae8631831de6be28c72a5b/HotkeyClash/Services/ShortcutTester.swift)

其 `ShortcutTestVerdict` 的吞键候选来自先前扫描的工具配置，并非读取系统回调返回值；源码在多候选说明中承认无法知道哪个实际处理。唯一已扫描候选时，它把结果标为 confirmed，但扫描覆盖不完整时仍可能漏掉真实处理者。此确定性不能照搬到 KeyCrash。[候选与判定](https://github.com/Wunderlandmedia/HotkeyClash/blob/2ce1a5a42db386e67fae8631831de6be28c72a5b/HotkeyClash/Models/ShortcutTestVerdict.swift#L63-L81)、[显示置信度](https://github.com/Wunderlandmedia/HotkeyClash/blob/2ce1a5a42db386e67fae8631831de6be28c72a5b/HotkeyClash/Models/ShortcutTestVerdict.swift#L128-L160)

HotkeyClash 的全局配置扫描包括 Karabiner、skhd、Keyboard Maestro、BetterTouchTool、Hammerspoon、Alfred。读取配置可以提供“某工具配置了该组合”的独立证据，但不等价于当前进程正在执行该配置；条件、模式、禁用状态、配置重载和多实例都要另行确认。本轮不照搬代码，也不扩大到用户应用内菜单扫描。[配置扫描器](https://github.com/Wunderlandmedia/HotkeyClash/blob/2ce1a5a42db386e67fae8631831de6be28c72a5b/HotkeyClash/Services/ConfigFileScanner.swift#L18-L28)

ShortcutDetective 的原作者页面明确注明并非所有热键都可检测；该页面未提供可核验其过滤器精确 PID 算法的源码。本轮没有找到可以据以承诺“公开 API 可精确归因任意 tap 吞键”的开源实现。[原作者页面](https://www.irradiatedsoftware.com/labs/)

## 优先受控实验

只启动和停止本任务自己的夹具，不暂停用户软件，不更改 TCC 数据，不注入第三方进程。每个夹具使用独立 PID、相同安全组合和有界生命周期，只由自身日志提供真值。观察器不能读取真值后再冒充独立检测结论。

| 顺序 | 实验 | 真值与观测 | 能回答的问题 |
| --- | --- | --- | --- |
| 1 | 无过滤器、单 pass、单 drop | 同位置 head/tail + 夹具自身 down/up 计数 + 元数据快照 | 首尾方法在此机器是否可靠区分基础放行/缺失 |
| 2 | 两进程 pass/pass、pass/drop、drop/pass、drop/drop | 两夹具独立 PID 和接收/返回日志 | 确认接收链行为；检验是否能独立找出首个 dropper，而不是猜候选 |
| 3 | 同样组合，仅改 marker、改键码、返回新事件 | 夹具记录改写动作；观察器仍只按严格 marker 关联 | 验证“head有tail无”不是“被删除”的充分条件 |
| 4 | 删除原事件并从 tap 后方重发 | 新事件分别保留/不保留 marker，限定安全组合 | 验证“tail有”不能证明原事件未被处理或删除 |
| 5 | HID/session/annotated 位置分别重复 | 每个位置独立首尾对，记录 tap 创建是否成功 | 识别观测区间，不把 session 结果推广为整个系统 |
| 6 | 运行中新增/移除自有过滤器 | 测试前后比较 tap ID/PID/位置/选项/启用状态 | 链发生变化时应中止判断或标证据不足 |
| 7 | latency 反例 | pass 与 drop 保持相同工作量；另一夹具人为短延迟但放行 | 证明耗时/变化排序不能可靠代表“占用” |
| 8 | Carbon 注册者 + 上游 drop | Carbon owner 接收日志、产品 subtype 6 结果、dropper 日志 | 区分注册者仍存在与本次事件未到达；恢复后归因应恢复 |

第 7 项每种状态少量重复即可；若出现一次可复现的错误 PID，就足以否定“确定归因”用途，无需长期采样寻找看似稳定的相关性。所有延迟远小于系统 tap 超时，仅用于自有夹具。

## 结果等级

1. **确认接收者**：有与本次标记严格关联的接收事件和 PID；只陈述该证据覆盖的路由。
2. **确认测试夹具行为**：独立夹具自身日志证明它接收/放行/删除；用于检验算法，不能作为对任意第三方的通用能力。
3. **观测到链内变化**：首尾证据完整、观察器健康，但不能唯一定位外部回调；不得用一个候选 PID 替代答案。
4. **候选或配置证据**：列出了可能的全局监听/过滤进程或某工具的组合配置。必须标候选，不显示“已定位”。
5. **没有充分证据**：权限、观察器、标记、链状态或输入路径不满足判定条件。不能解释成组合无人使用。

这里的未保证部分，是当前公开信息和观测方法的证据边界。后续实验可以验证某个系统上的具体行为，但不能用一次命中抹去已知的不可区分情形。

## M4 实测：首尾与进程元数据仍不能精确归因

在同一台 macOS 15.5（24F74）、M4 实机新增 18 个全局事件链场景，共执行 19 次（原地改写额外复测一次）。观察器为独立原生程序，HID/session/annotated 各放一对 listen-only head/tail，仅记录本次唯一 marker；没有将夹具 PID 传给观察器。独立夹具日志给出实际接收真值，前台安全窗口只用作送达对照。没有执行应用内菜单测试、暂停用户软件或禁用第三方 tap。

| 场景 | 首尾观测 | 独立真值与结论 |
| --- | --- | --- |
| 无夹具、全局被动监听、三位置 active pass | 6 个观测点均见原标记 | 前台收到；旁路监听是否触发动作不能由首尾推断 |
| 三位置 active drop | 在对应位置 head 后消失 | drop 回调收到、前台未收到；受控基础案例能缩小变化区间 |
| Carbon 共享 | keyDown 到 session，annotated 转为 subtype 6/9 | Carbon PID 正确；不能把 annotated 无原始 keyDown 误当吞键 |
| Carbon + session drop | 到 session head，后续无原标记 | tap 收到，Carbon 与前台均未收到 |
| 两 tap：pass/pass、pass/drop、drop/pass、drop/drop | 第一种全部通过，其余相同区间缺失 | 接收计数分别为 [1,1]、[0,1]、[1,1]、[0,1]；列表不报告谁返回 NULL |
| 返回新事件，保持按键但更改 marker | 与 session drop 相同的原标记缺失 | **前台实际收到替代按键；缺失不能证明用户输入被吞掉** |
| 丢弃原事件，从 tap 后重发，更改 marker | 同上 | **前台实际收到重发按键；同样不能判作输入被吞掉** |
| 丢弃原事件，从 tap 后重发，保留 marker | 6 个观测点都有原标记 | **看起来与 pass 相同，但原事件实际被替换** |
| 尝试原地修改输入事件 marker | 6 点都有原标记 | 本机回调内立即读取仍为旧值，修改没有生效；不计为改写成功的反例 |

tap 回调与 run-loop 的日志到达顺序确实不等于过滤顺序：部分 listen-only tail 记录先于 head。判断以上结果使用观测点集合，不使用打印顺序。

latency 也出现了直接反例：`pass-drop` 中，drop PID 99440 已记录接收，前台未收到，但前后 min/avg/max 均为 0；同场未收到的 pass PID 99437 统计也全为 0。`drop-pass` 中，放行者平均 latency 约 470.83 µs，高于实际 dropper 的约 309.33 µs。因此“统计变化才表示收到”和“最慢者就是吞键者”在这批实际记录中均不成立。此处只说明本次数据，不把调度相关数值视为稳定性能指标。

实机 SDK `CGEventTypes.h` 同时核实：回调允许返回新事件，调用方在完成回传后释放原事件和返回的新事件；`CGEventTapInformation` 没有事件计数、最后事件标记或 drop 字段。`CGEvent.h` 确认每次读取列表会重置 min/max 的统计基线。

这批实验验证了方法的边界，**没有实现任意全局过滤器的精确 PID 定位**。原型未接入产品，不把唯一候选、首尾缺失或 latency 变化显示成“已定位”。现有 Carbon 接收者归因保留，其他路径仍应如实显示证据不足。

### 复跑

在既有、已授权固定 Diagnostics host 同级编译小型观察器；无需重签 host。完整夹具设置见[机制测试说明](MACOS-SHORTCUT-TESTS.md)。

```sh
clang -fobjc-arc -framework Cocoa -framework Carbon \
  examples/macos-global-event-chain.m -o .task/owner-proof/owner-global-chain
clang -fobjc-arc -framework Cocoa -framework Carbon \
  examples/macos-shortcut-mechanisms.m -o .task/owner-proof/owner-mechanisms
clang -fobjc-arc -framework Cocoa -framework Carbon \
  examples/macos-owner-query.m -o .task/owner-proof/owner-query
mkdir -p .task/global-chain-results
PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-macos-global-chain.py \
  --app '.task/owner-proof/KeyCrash Diagnostics.app' \
  --output .task/global-chain-results/matrix.json --trigger-fixture
```

`--case` 可多次指定，仅复跑局部案例。输入限定 `⌃⌥⇧⌘F18`，检查真实按键已松开、Secure Input 未启用；不为了测试关闭安全输入。每个夹具限时退出，观察器约一秒后释放全部自有 taps。

Windows 证据保留在忽略的 `.task/macos-global-chain-20261013/`：首批 `matrix.json`、补充 `replacements.json` 和交叉核对 `validation.txt`；Mac 副本 `.task/global-chain-20261013/` 保留原始日志。本轮三个 Objective-C 程序编译成功（原有 helper 有废弃 API 警告），两份 Python 脚本 AST/CLI 帮助与 diff 检查通过。没有重新构建产品或触发发布。

收尾确认任务进程为空、held-key 数组为空，删除本轮生成的 `owner-mechanisms`（57792 字节）和 `owner-global-chain`（54800 字节），两路径均已不存在，临时 `chain-*` 目录为空。保留固定 host、既有 helper、源码与必要证据，未重签或新增授权。

## 网络方案复核

本节专查不同于首尾观察、latency 和候选进程枚举的新方法。网上找到的产品宣传、旧私有声明和当前机器导出符号，分别是不同强度的证据：均不能直接算作“已经定位任意全局过滤器”。网络资料核查读取作者页面、原始源码和符号研究资料，没有下载运行第三方检测软件；主执行器随后在 Mac 做了下文单独记录的参数核对与受控实验。

### 现有检测工具与框架

| 来源 | 原始资料实际支持的能力 | 对本项目的意义 |
| --- | --- | --- |
| [Supercharge 作者的 Keyboard shortcut inspector 说明](https://sindresorhus.com/supercharge#keyboard-shortcut-inspector) | 作者称能识别监听或注册某个全局组合的应用，同时明确只能检测多数快捷键，纯修饰键双击不支持。页面没有公开算法、API、命中证据或覆盖 CGEventTap 吞键的独立测试 | 有价值的对照产品，不能据营销中的 listening 推导它拥有任意 tap 的逐事件归因 API；本轮未找到作者公开的该功能实现 |
| [ShortcutDetective 作者页面](https://www.irradiatedsoftware.com/labs/) | 作者描述为查找接收快捷键的应用，并列出并非所有热键均可检测的限制；页面所列软件为 1.0，最低系统为 OS X 10.6 | 可说明老 macOS 已有“接收者探测”产品，不能据此判断其支持现代 CGEventTap 过滤，更不能把未知算法当现成实现 |
| [Hammerspoon 的 hotkey.lua](https://github.com/Hammerspoon/hammerspoon/blob/23e387e2805a9890066366e0ac96c71b27f0cfd5/extensions/hotkey/hotkey.lua) 与 [libhotkey.m](https://github.com/Hammerspoon/hammerspoon/blob/23e387e2805a9890066366e0ac96c71b27f0cfd5/extensions/hotkey/libhotkey.m) | `getHotkeys()` 遍历自身 Lua 热键表；`systemAssigned` 使用 `CopySymbolicHotKeys`；`assignable` 尝试创建并启用自身热键 | 可复用系统组合查询和夹具注册，但没有第三方 tap 的逐事件拥有者查询。文档也明确系统声明并不可靠地决定组合能否被覆盖 |
| [Aditya Vaidyam 的 RegisterEventHotKey 逆向与实现](https://aditya.vaidyam.me/blog/2018/03/16/) | 作者反汇编后用 CGS 私有调用重建自己的热键注册；指出 Carbon 使用分配对象的指针作 native hotkey ID，示例 ID 使用 Swift `Int` | 为本仓库 `uintptr_t` ID 的受控查询提供独立背景；不是全系统热键枚举或 tap 吞键归因方案。该旧系统逆向中的排他语义仍须以当前 SDK 与实测为准 |

HotkeyClash 的源码复核见前文：它提供配置扫描和行为观察的组合，仍不能补齐上述缺口。没有把作者私有商业产品当作开源，也没有把开源框架自己的注册表误称为系统注册表。

### 私有接口的新线索与参数依据

| 路径 | 找到的原始依据 | 可验证假设与限制 |
| --- | --- | --- |
| 按 tap ID 查询或启停 | 主执行器在当前 M4 的 SkyLight 导出中发现 `SLEventTapIDEnable`、`SLEventTapIDIsEnabled`、`SLSSetEventTapOption`。网上精确搜索前两者及 CG 别名未找到声明或调用代码；[CGSInternal 的 missing-symbols 原始讨论](https://github.com/NUIKit/CGSInternal/issues/3) 只列出旧 `CGSSetEventTapOption` 名称，没有 ABI | 这是区别于公开 CFMachPort API 的新实验入口，但导出名不证明能控制其他进程，也不证明参数类型。必须以本机系统函数反汇编及本任务自有 tap 正对照确认 ABI；不从函数名猜签名或选项 |
| 按连接、native ID 查询 Carbon 热键 | [CGSHotKeys.h](https://github.com/NUIKit/CGSInternal/blob/c4f6f559d624dc1cfc2bf24c8c19dbf653317fcf/CGSHotKeys.h) 声明 `CGSGetHotKey(cid, uid, options*, key*, modifiers*)` 等；旧头使用 `int uid`，与上述指针 ID 及本机实测不一致 | 只适合已知 ID 的正负对照，不能靠遍历小整数获得完整注册表。本仓库前述 self 成功、foreign 返回 1002 的实验已经限制这条路；该接口属于注册热键，不包含任意 tap 内部匹配逻辑 |
| 系统 symbolic hotkey 查询 | 同一 [CGSHotKeys.h](https://github.com/NUIKit/CGSInternal/blob/c4f6f559d624dc1cfc2bf24c8c19dbf653317fcf/CGSHotKeys.h) 有 `CGSGetSymbolicHotKeyValue(hotKey, unichar*, unichar*, modifiers*)` 和 `CGSIsSymbolicHotKeyEnabled(hotKey)` | 可用于核对系统命名动作及配置，属于状态查询；没有“处理此次按键的第三方 PID”输出，不能解决 tap 吞键 |
| 读取原始事件记录 | [CGSEvent.h](https://github.com/NUIKit/CGSInternal/blob/c4f6f559d624dc1cfc2bf24c8c19dbf653317fcf/CGSEvent.h) 声明 `CGEventGetEventRecord(event, outRecord, recSize)`，旧结构含 `eventProcess.pid`、appendix `eventOwner`、`passedThrough` | 新的只读调查方向，但字段语义未证明是最后处理者或吞键者；头文件的年代、指针宽度及注释偏移也不能当作 M4 布局。下游拿不到被丢弃事件时，读取自己收到的其他事件未必增加信息；不解引用未知 appendix 指针 |
| 私有通知或更底层事件接口 | 同一 [CGSEvent.h](https://github.com/NUIKit/CGSInternal/blob/c4f6f559d624dc1cfc2bf24c8c19dbf653317fcf/CGSEvent.h) 列出 `CGSRegisterNotifyProc(proc, type, userData)`，keydown 720、system-defined 724、symbolic hotkey changed 760 等；[原始 missing-symbols 讨论](https://github.com/NUIKit/CGSInternal/issues/3) 另列 `CGSGetEventTapList`、`CGSGetNextEventRecord`、`CGSGetHotKeyRepresentation` | 现有声明没有命名为“某 tap 丢弃本事件”的通知、有效载荷结构或 owner 查询。符号清单只能安排后续检查，不能证明隐藏着热键拥有者字段，更不能直接调用未知 ABI |

上述 CGSInternal 是社区逆向资料，固定版本为 `c4f6f559d624dc1cfc2bf24c8c19dbf653317fcf`，不是 Apple 对现代 macOS 的 API 保证。对 `SLEventTapIDEnable` / `SLEventTapIDIsEnabled` 的当前参数，本轮网络资料**没有足够依据给出函数声明**。主执行器已从本机 arm64 wrapper 推断调用约定，其中 `SLEventTapIDEnable` 接收 tap ID 与启用状态，再补默认 connection 进入 MIG；这是本机反汇编依据，仍需自有夹具的返回值、状态和事件对照验证。实测结果由主执行器另行补充，不能回填成网上已证实的事实。

### M4 上最有希望的下一轮实验

1. **优先验证按 ID 操作自有 tap 的能力。** 用本任务独立 fixture 创建一个已知吞掉测试组合的 active tap，以及不吞键的对照 tap。先只读反汇编确认参数，然后核对 `IsEnabled` 与公开列表状态是否一致。若要做 `Enable` 因果实验，仅作用于本任务 fixture 的 PID 和 tap ID，顺序为开启、关闭、恢复；每步都核对状态和带标记测试事件是否到达末端，最终确认原状态。它检验的是“可否进行受控隔离”，不是只读查询，也不能据此承诺产品会自动干预用户软件。
2. **若按 ID 操作成立，验证同一进程两个 tap、两个进程各一个 tap、双吞键和替换事件。** 单独关闭一个 tap 后恢复流通，支持它在该次受控路径上的因果作用；若两个 tap 都吞键，逐个关闭可能都无变化，不能判为都无关。全程保持其他 tap 和状态固定，结果只对相应 fixture 成立。
3. **并行的只读候选是原始事件记录。** 先核对当前函数是否存在、调用约定和 record 长度；使用自有合成事件及自有 tap 比较字段，不复制旧结构后盲读内存。至少区分 source PID、前台 target PID、已知 tap PID；仅观察字段相关还不足以称为 dropper。若 marker 被删改或事件被丢弃后没有可读记录，就明确记录该路径不可观测。
4. **商业工具只作为覆盖对照。** 如后续另行验证，用相同 Carbon shared/exclusive、listen-only、active pass/drop、双过滤器夹具检查输出；特别检查是否把监听任意 keydown 的软件列作目标组合拥有者。单个 QQ 命中、唯一候选或配置匹配都不能替代这组判别。

检索覆盖作者官网、作者的逆向说明与代码、NUIKit 原始头文件和 missing-symbols 讨论、Hammerspoon 源码、yabai 私有接口声明以及三条新 symbol 的精确名称与旧 CG 别名。未发现可直接移植、已证明能只读返回任意 tap 吞键者 PID 的实现；这是本轮检索结果，不是对所有私有接口作不存在证明。新增的具体实验方向是当前系统按 tap ID 的受控隔离，以及核实原始事件记录是否提供额外可观察信息。

## 按 ID 隔离实测：已找到可行的受控路线

在用户 M4、macOS 15.5（24F74）上，用 `dyld_info -exports` 核对 SkyLight 当前导出，发现 `SLEventTapIDEnable` 和 `SLEventTapIDIsEnabled`。读取本进程已加载系统 wrapper 的有限指令，核对 arm64 参数转发：前者接收 tap ID 和 enable，补默认 connection 后进入系统消息调用；后者接收 ID。没有修改系统二进制、读取其他进程内存或更改安全设置。网上没有找到这两个符号的可靠参数声明，因此没有声称这是网上现成、跨版本保证的接口。

实验控制器使用当前机器核对的函数指针类型 `CGError (*)(uint32_t, bool)` 和 `bool (*)(uint32_t)`；实际返回值再与公开 tap 列表状态交叉核对。只允许 PID 对应的完整可执行路径等于任务目录 `owner-mechanisms`，且该进程恰有一个启用的 active keyboard tap。传入本任务 SSH shell PID 被拒绝，退出码 65；没有操作用户应用或系统的 tap。

### 单个过滤器

| 夹具 | 开启时 | 临时停用时 | 恢复后 |
| --- | --- | --- | --- |
| HID drop | 目标组合未到末端 | 恢复到全部 6 个观察点，前台收到 | 再次被拦截 |
| session drop | 同上 | 同上 | 同上 |
| annotated drop | 同上 | 同上 | 同上 |
| session pass | 正常到达末端 | 仍正常到达 | 仍正常到达，不误判为导致缺失 |

四个场景禁用与恢复调用均返回 0，公开列表分别确认 enabled=false / true。接收者夹具在开启前后各收到一次；三个 drop 场景前台只在停用阶段收到一次，pass 场景前台三阶段都收到。

### 多个过滤器

单独停用一个进程并不足以处理双吞键。因此实验采用：先取得全部开启时的基线，再停用**所有获准参与的自有夹具**验证输入恢复；随后逐个只启用一个夹具，与全部停用的对照比较，最后恢复全部原状态。每个阶段前后都通过公开列表断言参与 PID 的实际启用状态，没有把控制请求返回成功当作状态已经生效。

| 两个独立进程 | 检出的路径改变者 | 独立真值 |
| --- | --- | --- |
| pass → drop | 仅 drop PID | 一致 |
| drop → pass | 仅 drop PID | 一致 |
| drop → drop | 两个 PID | 一致，没有漏掉被上游遮蔽的另一个 drop |
| pass → pass | 无 | 一致，没有把监听存在当冲突 |
| HID drop + annotated drop | 两个 PID | 一致 |
| session replace + annotated drop | 两个 PID | 一致；replace 只称“改变标记事件的传递”，不声称丢失用户按键 |

观察器不接收夹具 PID；控制器只将获准测试的 PID 用作操作边界。判定来自输入在开启/停用时的变化，最后才与夹具自身日志的真值核对。因此这是有操作条件的因果实验，不是把名单直接当定位结果。

### 恢复与限制

另两项独立测试验证：控制器保持运行但无人发送释放指令时，8 秒期限到达后恢复；仅向任务控制器发送 SIGTERM 时，信号处理只设置退出标记，主循环随后恢复。两项均恢复 code=0、enabled=true，并复测拦截重新出现。最终 held-key 数组为空。

当前仍是实验代码，不可把以下条件省略后直接用于用户桌面：

- 只验证 macOS 15.5 arm64；没有私有 ABI 的长期兼容保证。调用失败、候选 tap 消失或状态变化必须停止判断。
- 首批夹具与控制器由同一个已授权 Diagnostics host 启动；下节补充了独立签名应用身份。系统 tap 和其他用户进程尚未验证，跨进程成立不等于可任意操作所有应用。
- 首批只验证正常超时和 SIGTERM；下节新增独立恢复进程与控制器 SIGKILL 对照，不能把原来的进程内信号处理称作崩溃恢复。
- 需要在产品中明确告知并由用户发起临时停用哪些软件的全局监听。停用期间其他快捷键、重映射和辅助功能可能暂时失效；目标组合会多次真实触发。当前原型不能接收任意用户应用 PID。
- 被动监听、放行后另行触发动作、保留标记重发、IOHID 等其他路径不保证产生本方法可见的路径变化。不能据此宣称定位所有全局快捷键处理者。
- 结果应区分 Carbon 接收者与通过隔离确认的传递影响者；替换事件不能一概称作吞键。若停用全部获准候选后仍不恢复，不猜测某个候选就是原因。

### 复跑与产物

代码：`examples/macos-tap-toggle-experiment.m`、`scripts/test-macos-tap-toggle.py`、`scripts/test-macos-tap-isolation.py`。需要前文的固定 Diagnostics、`owner-query`、`owner-mechanisms`、`owner-global-chain`。新增控制器编译命令：

```sh
clang -fobjc-arc -framework Cocoa -framework Carbon \
  examples/macos-tap-toggle-experiment.m -o .task/owner-proof/owner-tap-toggle
mkdir -p .task/global-search-results
PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-macos-tap-toggle.py \
  --app '.task/owner-proof/KeyCrash Diagnostics.app' \
  --output .task/global-search-results/toggle.json --trigger-fixture
PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-macos-tap-isolation.py \
  --app '.task/owner-proof/KeyCrash Diagnostics.app' \
  --output .task/global-search-results/isolation.json --trigger-fixture
```

超时和终止对照：第一个脚本指定 `--case tap-1-drop`，分别加 `--wait-for-timeout`、`--terminate-control`。所有调用都只允许本任务夹具，不能用参数放开到第三方软件。

证据位于 Windows `.task/macos-global-search-20261013/` 与 Mac 副本 `.task/global-search-20261013/`，包含四个单过滤器、六个双过滤器、超时及 SIGTERM 两项恢复对照。原型没有进入运行时、Git 提交或发布包；这轮结果是找到并验证了实现方向，不是完成正式产品验收。

编译、Python AST/CLI 与 diff 检查通过。收尾核对任务进程为空、held=[]，删除三个本轮生成的原生夹具/控制器及 ABI 检查对象文件，实际路径均不存在；临时 tap-toggle/isolation 目录已自动清理。保留原始证据、有限 ABI 依据、源文件和既有授权 host，未更改用户应用的权限或监听状态。

## 独立恢复、多监听器与独立应用身份

后续补充仍在同一台 M4 / macOS 15.5 执行。`macos-tap-toggle-experiment.m` 改为通过 `posix_spawn` 启动独立 guardian；**只有 guardian 改变 tap 状态**，控制器仅持有租期管道的写端。控制器退出、崩溃或被 SIGKILL 时，管道断开触发还原；控制器被 SIGSTOP 时，guardian 自己的 8 秒单调时钟期限仍可到达。没有依赖主进程的 finally 或退出回调，也没有在多线程 AppKit 进程 fork 后继续执行框架代码。

变更前记录 PID、进程启动时间、可执行路径及 tap 的 ID/位置/目标 PID/选项/事件掩码；每次还原前重新核对。只处理原先已启用的 active keyboard tap，原本停用的 tap 保持停用。一次控制同一进程的全部符合条件 tap，部分操作失败也逐项尝试还原已经操作的原目标。结果输出区分实际还原与“目标消失，未调用还原”，不把消失报成恢复成功。

标准输出忽略 SIGPIPE，并设置非阻塞，防止结果读取端断开或管道填满阻止恢复、退出。控制对象依然硬编码限定为两条本任务夹具路径：Diagnostics 旁的 `owner-mechanisms`，以及同级 `KeyCrash Permission Test Filter.app/Contents/MacOS/owner-mechanisms`；不能传任意用户软件路径来放开范围。

| 实机对照 | 验证结果 |
| --- | --- |
| 主控制器 SIGKILL | 独立 guardian 还原，公开列表启用，再发组合重新被拦截 |
| 主控制器 SIGSTOP、无人释放 | guardian 自身期限到达后还原，不等主控制器恢复执行 |
| 主控制器 SIGTERM、正常释放 | 断开租期后还原；多个 tap 均恢复 |
| 目标夹具退出 | 跳过已经消失的目标，无旧 ID 写入；事件重新到达全部观察点 |
| 控制器 SIGKILL，同时输出读端关闭 | 两个 tap 恢复，拦截复测正确，guardian 退出 |
| 控制器 SIGKILL，同时输出管道填满且无人读取 | 同上，没有卡在日志写入 |
| 同进程 drop + pass、pass + pass | 全部获准 tap 临时停用并还原，放行不误判 |
| 同进程一个启用、另一个原本停用 | 只操作启用的 tap；原本停用者前中后均保持停用 |
| 两进程隔离回归 | pass/drop 排列、双 drop、双 pass、跨位置双 drop、replace + drop 的六项 PID 集合正确 |

独立应用身份不再借用 Diagnostics：脚本创建独立 bundle ID 和 ad-hoc 签名，首次启动记录 `AX=false`，通过正常 System Settings 辅助功能界面开启**该测试应用自己的开关**，之后运行吞键、放行、控制器 SIGKILL。单 tap 和同进程多 tap 两组均通过；控制器仍使用原 Diagnostics 身份。测试后仅撤销该临时 bundle ID 的 Accessibility/PostEvent 权限，没有重签固定 Diagnostics，也没有读取或修改 TCC 数据库。

复跑新增入口：

```sh
# 先按前节编译三个原生夹具；继续使用固定 Diagnostics 和既有 owner-query。
PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-macos-tap-toggle.py \
  --app '.task/owner-proof/KeyCrash Diagnostics.app' --trigger-fixture \
  --case tap-1-drop-pair --kill-control --output .task/guardian-results/kill.json
PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-macos-guardian-pipe.py \
  --app '.task/owner-proof/KeyCrash Diagnostics.app' --trigger-fixture \
  --fill-output --output .task/guardian-results/full-pipe.json
PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-macos-independent-filter.py \
  --app '.task/owner-proof/KeyCrash Diagnostics.app' --trigger-fixture \
  --output .task/guardian-results/independent
```

先创建输出目录。第一个脚本另支持 `--stop-control`、`--terminate-control`、`--wait-for-timeout`、`--exit-target`；原本停用对照使用 `--case tap-1-drop-disabled`。独立应用脚本拒绝覆盖同名现有 bundle，执行后撤销权限；bundle 本身须核对无进程后按本任务文件清单清理。

这补足了**控制器异常恢复和独立测试应用身份**，尚未接入产品 UI/安装包。独立 guardian 自己被 SIGKILL、系统注销、同一进程重用完全相同 tap 元数据等不在已验证保证内；公开列表没有 tap 世代标识，不能声称彻底排除了所有 ID 重用竞态。真实第三方应用、自行重建监听器的并发变化、媒体键/IOHID、被动执行但不改变事件传递的监听仍不据此宣称覆盖。正式接入需要候选软件选择、明确参与范围、取消/错误状态及真实窗口验收。

证据保留在 Windows 与 Mac 的 `.task/guardian-20261013/`，包含原始 JSON、运行日志及交叉核对结果。运行时代码、版本和发布状态未改变。


## 产品接入与实机验证

本地工作区新增“检查全局监听”入口：默认不选软件，允许选择最多六个候选，以独立恢复进程进行临时隔离，分别显示改变按键传递的软件与 Carbon 接收者。范围、影响及限制见 [Mac 平台说明](MACOS.md#全局监听检查)。本次实现随 v0.2.3 分发。

M4 / macOS 15.5 上最终后端矩阵 13 项通过：HID/session/annotated 三位置拦截、放行阴性、双软件拦截、放行与拦截混合、替换与拦截混合、同进程多 tap、原先停用的 tap、预先取消、检查中取消、Carbon 与上游拦截混合、监听器自行重建。最后一种明确停止归因，未给变化后的监听器输出占用结论。其余场景复核原 tap 状态恢复；放行不误报。

12 项使用已单独授权的 Verified 测试应用。替换事件补充使用 Diagnostics 启动的自有 Replacement helper，与独立身份的下游拦截者组合；不能将该补充说成替换者也验证了独立授权身份。旧替换夹具在同步 tap 回调内新建键盘事件时超时，产品没有假报成功；将事件模板提前建立后，此项 PID 集合和恢复状态均正确。复制收到的事件则保留了本机原序列化行为，不能当成已验证的事件替换样本。

实际 Slint 窗口通过默认不勾选、定位到正确 PID/路径、多 tap 恢复、目标退出后拒绝旧候选、检查中取消、主程序 SIGKILL 后独立恢复，以及结果的中英日切换。原 Carbon 窗口的捕获、共享接收者、释放和取消回归通过。截图发现的结果文本裁切已修正为固定宽度的换行滚动区域，并显示当前检查组合。

集成测试发现 `NSRunningApplication` 在后台线程可能没有目标对象，而 `proc_pidinfo`、路径和 tap 列表仍完整；身份校验已改用内核进程信息，应用 bundle 元数据仅用于系统应用排除与显示名称。权限验收另外补上锁屏检测、系统密码确认框识别和文本输入框日志脱敏，避免把 Settings 开关开启误判为已授权。

复跑入口是 `scripts/test-macos-filter-product.py` 与 `scripts/test-macos-filter-ui.py`，实际后端使用 `examples/macos-filter-production.rs`。原始证据在两端 `.task/filter-product-20261013/`，最终矩阵 `backend-final/matrix.json`；窗口流程与截图另存。Windows 27 项、Mac 5 项单元测试通过；两端 Clippy 与原生模块严格编译通过。仅测试 macOS 15.5 arm64 的实际执行，不将其扩展为所有 macOS/Intel 的运行保证。
