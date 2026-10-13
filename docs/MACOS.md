# macOS 开发与验收

v0.2.0 因缺少占用软件定位已撤回。后续实现包含注册检查与按需主动定位；Release 构建先生成草稿，真实桌面验收后才公开。

macOS 适配以 Apple Silicon（包括 Mac mini M4）为优先验证目标，同时提供 Intel 和 Universal 2 构建入口。部署目标设为 macOS 11.0；这是构建下限，尚不能代替各系统版本的实机兼容性验证。v0.2.0 起的 tag 发布流程汇总 Windows、Mac arm64 和 Mac x86_64 包。

## 构建与运行

在 Mac 安装 Rust stable、Python 3 和 Xcode Command Line Tools 后，在仓库根目录运行：

```sh
cargo run --locked -p keycrash --target-dir .task/macos-dev/target
```

构建可双击的 `.app` ZIP：

```sh
python3 scripts/build-macos.py --arch arm64
# Intel Mac：--arch x86_64；同时适用两种架构：--arch universal2
```

输出在 `dist/keycrash-v<版本号>-macos-<架构>.zip`，解压后得到 `KeyCrash.app`。脚本验证架构、Info.plist 和 ad-hoc 签名；没有 Developer ID 签名或 Apple 公证，不能宣称已经满足公开分发条件。脚本不发布文件、不触发 Git 操作。

打包默认在独立 `.task/macos-package-*` 目录构建并在结束时清理；需要复用编译缓存可传 `--target-dir .task/macos-dev/target`，该目录会保留。开发结束后确认没有构建或运行进程，再删除自己创建的 `.task/macos-dev`。

`.github/workflows/macos.yml` 在 Apple Silicon 与 Intel runner 上分别编译、测试和打包，上传 Actions 构建产物。分支 CI 不发布；tag 的 `release.yml` 复用该工作流，在 Windows 和两个 Mac 构建全部成功后统一创建 Release。只有提交被授权推送或工作流另行触发后才会运行，本地添加工作流不代表 CI 已通过。

Mac 只构建主包（`-p keycrash`，也是默认 workspace member）。`native/owner-*` 和 Windows fixture 是 Win32 专用组件，不应在 Mac 上运行 `cargo test --workspace`。

## 检测范围

查询对象仅为全局快捷键及其注册、监听、拦截路径。应用内菜单、本地 monitor、窗口快捷键不在冲突定位范围；开发夹具中的本地窗口只作为事件是否继续传递的对照。

v0.2.2 的定位尚未覆盖所有常见机制。[机制调研](MACOS-SHORTCUT-RESEARCH.md)与[逐项实测](MACOS-SHORTCUT-TESTS.md)确认：Carbon 接收者能够归因，但三种位置的主动 event tap 即使吞掉了目标按键也未被定位；上游吞键还会令已注册的 Carbon 接收者不再出现在结果中。未命中不能解释为“没有软件处理此组合”。

v0.2.3 的[受控隔离流程](MACOS-GLOBAL-ATTRIBUTION.md#独立恢复多监听器与独立应用身份)已接入“检查全局监听”入口，并通过本轮后端和实际窗口验收；替代仅有 Carbon 接收者定位的旧版。

### 全局监听检查

检测组合后，点击“检查全局监听”，从列表中选择最多六个软件；默认不勾选任何软件。列表只包含当前用户的非系统应用中，已启用的全局键盘主动 event tap。列在候选中不代表占用该组合。

检查会多次发送目标组合，并临时暂停所选软件的全局键盘监听；它们的其他快捷键或按键重映射也可能短暂停止。先比较全部暂停时的事件路径，再逐一恢复软件并重复比较。只有对照稳定且监听状态恢复成功，才展示“改变按键传递的软件”；恢复后出现的 Carbon 接收者单独展示。结果只覆盖参与检查的软件，不用于断言其余软件没有影响。

每个参与软件由独立恢复进程负责暂停和恢复。取消、主程序退出或租期管道断开都会请求恢复，单个租期最长 30 秒。检查前后核对进程启动时间、可执行文件和监听器元数据；发现重建、退出或状态变化就停止归因。原先停用的监听器不启用；不能确认恢复时明确提示重新打开参与软件。

此路径使用私有 `SLEventTapIDEnable`，兼容范围以实测系统为准。它不覆盖 IOHID/驱动、媒体键、仅 key-up 的行为，以及没有改变所观测事件路径的被动动作；也不保证恢复进程自身被强杀或监听器 ID 与相同元数据在同一进程中复用的情况。不得将其称为所有全局快捷键的完整枚举。

开发验收入口为 `scripts/test-macos-filter-product.py` 和 `scripts/test-macos-filter-ui.py`，使用独立签名夹具和实际产品后端/窗口。运行前检查控制台已解锁；辅助功能开关之后若出现系统密码确认框，必须完成确认并以进程的实际权限为准。开关显示开启、编译成功或脚本尚未进入用例都不算行为验证通过。

- 修饰键采用 ⌃ Control、⌥ Option、⇧ Shift、⌘ Command。
- 只在应用等待目标键时消费 AppKit 本地按键事件，直接使用 `NSEvent.keyCode`。标签使用当前布局的字符；功能键、方向键、小键盘有独立名称。死键或输入法无字符时显示实际键码，不猜成美式键盘字母。
- 先通过 `CopySymbolicHotKeys` 检查系统设置里已启用的组合；匹配时显示系统证据，不尝试临时注册。它不枚举应用内菜单快捷键，也不能从条目推断具体系统功能名称。
- 随后在主线程短时调用 `RegisterEventHotKey`，采用 `kEventHotKeyExclusive`，立即 `UnregisterEventHotKey`。不会发送完整组合或安装全局事件监听器；释放失败不会显示成功。
- `eventHotKeyExistsErr`（-9878）显示注册冲突且占用者未知；其他 OSStatus 显示检测未完成，保留有符号错误码。macOS 15.0/15.1 的 Option / Shift 限制等不能归类成占用。
- 成功只显示“此组合可临时注册”。共享 Carbon 注册、事件监听、应用菜单、驱动及未列出的系统行为不能靠这一结果排除。Apple SDK 对已有非独占注册的描述存在差异，因此实机 fixture 分别记录独占和共享注册者的结果。
- “已发现注册冲突”“此组合可临时注册”和“系统设置中有此组合”都可以继续点击“定位占用软件”。系统设置条目不证明实际接收者是系统，也不排除第三方注册。macOS 15.5 实测：已有共享注册时，独占临时注册仍可成功，因此不能仅在注册失败时提供定位入口。
- 定位单独申请辅助功能权限；普通检测不申请。授权后主动发送一次目标组合，原动作可能执行。使用短时只读 annotated-session event tap，仅接受携带本次唯一标记的 system-defined hotkey-down（subtype 6）事件，再读取接收 PID 和可执行文件路径；不把前台窗口、普通 keyDown 目标或所有监听器当成占用者。多个接收进程分别展示。
- 首次授权后返回原窗口点击定位即可。权限检查使用 `AXIsProcessTrusted`；macOS 15.5 的 `CGPreflightPostEventAccess` 会缓存首次拒绝，即使辅助功能已授权仍可能返回 false。发送前仍检查 event tap 创建是否成功。新增实机回归验证同一 PID 从未授权到定位 QQ，不以已授权 Diagnostics 子程序替代首次授权验收。
- 观察在约一秒后结束，也可取消。按下后始终发送释放，并清除模拟修饰键状态；回调、run-loop source 和 event tap 在退出时释放。没有匹配事件时显示“没有观察到占用软件”，不推断组合空闲；目标可能忽略模拟输入。未使用进程注入、调试权限、私有 hotkey 查询或持续录键。
- 音量、亮度、媒体、Fn/Globe 单键等系统事件不属于普通 keyDown 检测范围。需要测试 F 键时，按键盘设置使用 Fn+F 键；Esc 用于取消。

## 原生回归入口

在登录桌面的 Mac 上运行，不需要真的按下测试组合：

```sh
cargo test --locked -p keycrash --target-dir .task/macos-dev/target
cargo build --locked -p keycrash --example macos-hotkey-fixture --target-dir .task/macos-dev/target
python3 scripts/test-macos-hotkeys.py .task/macos-dev/target/debug/examples/macos-hotkey-fixture
```

fixture 为独立进程，自动选择一个空闲的四修饰键 + F13–F20 组合，验证空闲 → 独占注册冲突 → owner 释放 → 连续两次可注册；共享注册单独记录实际结果。超时会结束自己启动的 owner。CI 只编译该 fixture，真实桌面的运行结果另行验收。

## Mac mini 验收清单

1. 记录 `sw_vers` 和 `uname -m`，从 Finder 启动 `.app`，检查图标、窗口与关闭/重新打开。
2. 中文、英文、日文分别查看首页、帮助、等待、可注册、冲突、系统占用和错误状态；确认符号可见、文案可读、按钮没有残留 Windows 定位行为。
3. 测试 ⌘、⌃、⌥、⇧ 各种组合及普通字母、Space、Return、Tab、方向键、F12、Delete、Forward Delete、小键盘。F12 不应套用 Windows 调试器保留规则。
4. 切换英语、非美式布局或中文输入法；字符标签和实际物理按键应一致，死键不能误映射为其他按键。
5. 长按不重复检测，修饰键不算目标键，Esc 取消；切换到其他应用时不捕获输入。Return/Space 不应在完成检测后立即触发下一次操作。
6. 在系统设置启用、禁用或修改一个系统快捷键，确认结果随设置变化；用上述 fixture 检查跨进程注册和释放。
7. 未授予辅助功能权限时，普通检测仍可用，不自动弹权限请求。主动点定位才申请辅助功能；授权前不发送组合，授权后回到同一进程再次定位，不能靠重启通过验收。
8. 分别验证独占和共享注册者、两个不同组合、连续定位、未注册阴性对照、退出后不再命中、取消及真实 `⌃⌥A`。不能把“编译通过”当作归因验收。

## 定位验收工具

`examples/macos-owner-query.m` 是独立 Carbon 注册者；`examples/macos-owner-production.rs` 直接调用产品的 Rust 后端。`scripts/test-macos-owner-production.py` 验证 PID、路径、重复调用、阴性对照、释放与预先取消。`scripts/test-macos-owner-gui.py` 保留原生路由实验。`CGSGetHotKey` 仅在实验源码用于自查，未进入产品。

SSH 子进程和 Launch Services 启动的应用可能有不同的 TCC 归属。实机验证采用固定、已授权的 `KeyCrash Diagnostics.app` 启动器，旁边放 `owner-query`、`owner-production` 和 GUI 测试用的 `keycrash`，这样更新测试二进制无需反复重签启动器。脚本必须显式传 `--trigger-fixture`；`--user-shortcut` 额外实际触发 `⌃⌥A`。示例：

```sh
cargo build --locked -p keycrash --example macos-owner-production
python3 scripts/test-macos-owner-production.py --app '/path/to/KeyCrash Diagnostics.app' --trigger-fixture
python3 scripts/test-macos-owner-ui.py --app '/path/to/KeyCrash Diagnostics.app' --trigger-fixture --qq-shortcut
```

`--qq-shortcut` 实际触发用户配置的 `⇧⌘F`，要求窗口显示 QQ 及其路径。首次授权另用 `scripts/test-macos-permission-ui.py --app '/path/to/KeyCrash Permission Test <unique>.app' --diagnostics '/path/to/KeyCrash Diagnostics.app'`：测试应用须使用当前产品二进制、独立的新 bundle ID、匹配文件名的 CFBundleName，并完成 ad-hoc 签名。脚本通过 Launch Services 单独启动它，在正常系统设置界面开启该测试应用的开关，要求原 PID 从权限提示进入 QQ 定位结果；已授权或重签后残留的旧身份不算首次授权。不要重签固定 Diagnostics，也不要在 CI 绕过 TCC。用完撤销并移除本任务测试身份及临时应用。

原生辅助程序和权限请求只用于开发验收，不打包进用户版本。公开版本由主程序直接完成定位，用户应授权 `KeyCrash` 本身。在 GitHub 托管 runner 上编译夹具，不自动绕过桌面授权。

## 依据

- [Apple AppKit 事件监控说明](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/MonitoringEvents/MonitoringEvents.html)：本地事件作用域、主线程回调及移除监听。
- Apple SDK `CarbonEvents.h`：热键注册参数、非线程安全约束、`CopySymbolicHotKeys` 数据及覆盖范围（可在安装的 SDK 中核对；[Apple 头文件镜像](https://github.com/phracker/MacOSX-SDKs/blob/master/MacOSX11.3.sdk/System/Library/Frameworks/Carbon.framework/Versions/A/Frameworks/HIToolbox.framework/Versions/A/Headers/CarbonEvents.h)）。
- [Apple 关于 macOS 15 热键限制及 15.2 修正的讨论](https://developer.apple.com/forums/thread/763878)。
- [CGGetEventTapList](https://developer.apple.com/documentation/coregraphics/cggeteventtaplist(_:_:_:)) 提供监听器列表；该列表本身没有目标组合的占用证据。
