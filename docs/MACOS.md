# macOS 开发与验收

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

- 修饰键采用 ⌃ Control、⌥ Option、⇧ Shift、⌘ Command。
- 只在应用等待目标键时消费 AppKit 本地按键事件，直接使用 `NSEvent.keyCode`。标签使用当前布局的字符；功能键、方向键、小键盘有独立名称。死键或输入法无字符时显示实际键码，不猜成美式键盘字母。
- 先通过 `CopySymbolicHotKeys` 检查系统设置里已启用的组合；匹配时显示系统证据，不尝试临时注册。它不枚举应用内菜单快捷键，也不能从条目推断具体系统功能名称。
- 随后在主线程短时调用 `RegisterEventHotKey`，采用 `kEventHotKeyExclusive`，立即 `UnregisterEventHotKey`。不会发送完整组合或安装全局事件监听器；释放失败不会显示成功。
- `eventHotKeyExistsErr`（-9878）显示注册冲突且占用者未知；其他 OSStatus 显示检测未完成，保留有符号错误码。macOS 15.0/15.1 的 Option / Shift 限制等不能归类成占用。
- 成功只显示“此组合可临时注册”。共享 Carbon 注册、事件监听、应用菜单、驱动及未列出的系统行为不能靠这一结果排除。Apple SDK 对已有非独占注册的描述存在差异，因此实机 fixture 分别记录独占和共享注册者的结果。
- macOS 首版不提供“定位占用软件”，冲突后可直接重新检测；没有辅助功能授权流程，也不会将事件监听器的进程列表冒充该组合的占用者。
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
7. 不同意或未授予辅助功能/输入监控权限时，普通流程仍应可用。运行时不应弹出这些权限请求。

## 依据

- [Apple AppKit 事件监控说明](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/MonitoringEvents/MonitoringEvents.html)：本地事件作用域、主线程回调及移除监听。
- Apple SDK `CarbonEvents.h`：热键注册参数、非线程安全约束、`CopySymbolicHotKeys` 数据及覆盖范围（可在安装的 SDK 中核对；[Apple 头文件镜像](https://github.com/phracker/MacOSX-SDKs/blob/master/MacOSX11.3.sdk/System/Library/Frameworks/Carbon.framework/Versions/A/Frameworks/HIToolbox.framework/Versions/A/Headers/CarbonEvents.h)）。
- [Apple 关于 macOS 15 热键限制及 15.2 修正的讨论](https://developer.apple.com/forums/thread/763878)。
- [CGGetEventTapList](https://developer.apple.com/documentation/coregraphics/cggeteventtaplist(_:_:_:)) 提供监听器列表；该列表本身没有目标组合的占用证据。
