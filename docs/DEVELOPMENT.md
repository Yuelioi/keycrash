# 开发与构建

macOS 的构建、打包、CI 与原生验收入口见 [macOS 开发与验收](MACOS.md)。下述 PowerShell 命令和 workspace 测试用于 Windows。

版本号由根目录 `Cargo.toml` 的 `package.version` 管理，自动显示在窗口标题和界面品牌名右侧。

```powershell
cargo run --target-dir (./scripts/get-target-root.ps1)
cargo test --locked --workspace --target-dir (./scripts/get-target-root.ps1)
./scripts/build-release.ps1
```

Windows 专业模式使用 x64/x86 被动消息观察组件和独立序列执行进程。调试完整功能先运行 `./scripts/build-release.ps1 -Profile debug`，再启动解析出的 `debug/keycrash.exe`。仅 `cargo run` 不复制这些组件，缺失时明确报错。`keycrash_trace64_v3.dll` 与 `owner-x86/keycrash_trace32_v3.dll` 使用不同名字及V3映射，避免跨位数和旧驻留模块混淆；必须整包更新。

`native/sequence_protocol.rs` 是编排与执行契约：点按/按下/松开/等待、有效按键目录、32步/5秒/等待20–2000ms、配对与系统保留组合验证。GUI与helper都验证，未通过不发送。GUI通过stdin发送KSP1计划给同目录 `keycrash-owner-probe.exe --professional-sequence <parentpid>`；KST1固定记录返回READY、SendInput接受的事件及时间、STEP_DONE、END状态。准备完成后GUI工作线程倒计时3秒，再发送GO；关闭控制管道取消。输入helper不安装全局键盘Hook，检查初始按键未按住，每步检查取消与父进程；退出释放本次按键。只有输入helper调用SendInput并按实际held集合清理，GUI依据发送和清理确认判断剩余持有状态；组件异常退出且不能确认释放时，明确提示用户手动恢复，不按整份计划额外发送松开事件。旧Windows全局录制入口和旧Mac录制主路径已删除；两端共用编排与Job，分别执行原生输入。

“以管理员权限监听”默认关闭，通过runas启动x64被动监听coordinator，再启动继承权限的x86 host，验证管理员token和双位数READY；GUI先以普通权限创建映射，准备/倒计时状态不采集，发送时激活。仅监听提权，发送helper仍普通权限；SendInput受UIPI限制，失败不自动提权或重放。停止通过active卸载两位数Hook，父进程结束时清理；无响应不表示可用。发送记录是系统接受证据，不是目标处理或动作完成证明。接口依据[SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)及[KEYBDINPUT](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-keybdinput)。

响应监听启动失败时，V3原有error字保存首个失败的位数、阶段及Win32系统错误码（低16位，与HRESULT_FROM_WIN32一致）；不改变映射布局。OpenProcess、LoadLibraryW、GetProcAddress、SetWindowsHookExW失败立即保存GetLastError，启动失败分支与子进程观察保留首错，GUI在清理前读取并显示。x86子进程提前退出单列为观察组件退出，不把退出码误称Windows错误。GUI显示诊断细节，三语切换保留API名和数字；没有自动降权或重试输入。

活动绑定读取当前仅支持 GoldenDict 1.5.1 官方 `QT_5123.64bit` 与 `QT_486` 包中已核验的两份 GdHotkeys.dll 代码 profile，见 `native/owner-hook/src/goldendict.rs`。在目标进程取消息的回调内平衡持有模块引用，要求同名模块唯一，验证 PE 元数据、导出 RVA 和撤销已知重定位后的 `.text` SHA-256，再通过当前进程的 ReadProcessMemory 连读两次完整配置；不读取磁盘用户配置、不扫描地址、不修改目标。新增 profile 需重新核验原件、重定位与数组布局，不能仅按产品版本或文件名放行。

`src/sequence.rs` 对完整输入事件重放五槽状态机（Windows为确认发送的测试事件），包括重复、事件之前的修饰状态、第二阶段优先、超时第二键刷新及完成后清空。发送事件记录KEYBDINPUT时间及SendInput返回时刻，响应记录消息时间，均使用DWORD时钟及回绕差值；500ms 边界附近、回调延迟、绑定变化和不完整记录不升级为相符。完成事件按进程启动身份、模块与槽位消费，多个接收者可各自匹配同一次输入。该机制只提供活动绑定与输入的诊断一致性：取消息时采样不排除此前的配置变化、旧通知或伪造发送者，也不证明动作完成和独占。

构建和 fixture 测试脚本读取 Cargo 的 target 根目录（优先使用全局 `CARGO_TARGET_DIR`），再拼接 `keycrash`，release 产物位于 `<根目录>\keycrash\release`。迁移时只需修改全局变量，脚本不写死盘符，也不修改全局变量。未设置变量时遵循 Cargo 配置或默认目录。测试脚本也可通过 `-ReleaseRoot` 指定产物目录。

`scripts/test-owner-fixture.ps1` 的 Win 与四修饰键用例验证实际 `SendInput`；fixture 的主动消息延迟超过探测超时，不能代替真实输入。运行这些用例时，应让普通权限 KeyCrash 保持前台，匹配用户点击“定位占用软件”的环境；其他前台窗口可能改变模拟输入的结果。可用 `-ReleaseRoot` 分别检查 x64 与 x86 产物。

GitHub Actions 分为以下流程：

- `.github/workflows/build.yml`：代码、资源、构建配置变更的分支 push / PR 执行测试与编译检查，不打包、不上传产物、不发布。纯文档提交不触发。
- `.github/workflows/macos.yml`：分支 push / PR 编译、测试并上传 Mac 双架构包；同时供 Release 工作流复用。
- `.github/workflows/release.yml`：推送 `v*` tag 或手动指定已存在的 tag 时执行。验证版本一致，并行构建 Windows 与 Mac，全部成功后创建 GitHub Release，附带三个 ZIP。含 `-` 的预发布版本标记为 prerelease。

发布顺序：修改版本号并更新 `Cargo.lock`，提交并推送 `main`，然后创建并推送对应 tag。例如当前版本为 `0.1.0`：

```powershell
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

不要复用已发布的 tag；后续发布先更新版本号。

发布前需要明确选择 Slint 的 GPLv3、Royalty-free attribution 或商业许可路径。

Mac序列输入由同一可执行文件的 `--professional-sequence <parentpid> <marker>` 子模式执行，入口早于AppWindow创建，不启动第二个GUI。Cocoa/CoreGraphics桥在 `sequence_native.m`，与现有native静态库一起编译。GUI父worker上的listen-only annotated-session tap仅接收带marker的Carbon subtype6，最多256条；未标记键盘事件不采集。子进程预创建按键事件对，每次发送刷新mach时间戳/左右修饰键/数字键盘标志；控制EOF或getppid变化请求释放实际held，辅助功能权限丢失显式报告释放未确认。Mac媒体使用NX系统事件，实际处理依设备；发送日志仅是提交证据，无整段绑定关联。

专业模式序列历史最多两条，使用有界KSH1容器和现有KSP1计划验证；Windows保存在 `%LOCALAPPDATA%/KeyCrash/sequence-history.bin`，Mac在 `~/Library/Application Support/KeyCrash/sequence-history.bin`。仅保存编排，不保存结果；多实例更新先取独占文件锁、读取最新文件，再按完整序列更新或删除并原子写入。锁竞争或读写失败显示错误，不在GUI线程等待，不读取旧格式或迁移其他平台键码。

单键选取由 `src/sequence_key_capture.rs` 的窗口级 Winit event filter 处理；Slint固定1.17.1启用 `unstable-winit-030` API，不新增全局键盘Hook或系统输入。Windows原生scancode按当前布局映射到VK，单列保留小键盘身份；Mac用原生物理码。只在用户显式选取时填写按键，过滤重复/预按住/焦点合成，按已消费的down配对消费up，选取前传播的down对应up仍传播给Slint。两端实际是否收到系统/软件已截获按键不作保证。
