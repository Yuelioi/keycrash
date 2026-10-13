# macOS 快捷键逐项实测

2026-10-13，在 Mac mini M4、arm64、macOS 15.5（24F74）的真实登录桌面测试。依据见[机制调研](MACOS-SHORTCUT-RESEARCH.md)。本轮发现明确的定位缺口，不能将结果概括为“完整支持 macOS”。

范围修订：用户随后明确只查全局快捷键。本文应用内场景保留为历史阴性对照，不计入产品缺失能力或后续验收要求；机制脚本默认仅运行全局场景。下列命令加 `--include-local-controls` 才复跑历史完整矩阵。

## 测试对象与判定

产品后端为 v0.2.2 的 `examples/macos-owner-production.rs` 原生可执行程序，直接调用实际 Rust 检测与定位实现；本轮未修改运行时代码。保留二进制 SHA-256：`cd99cfcb7b96f931f5a4fab0971eb3e9bfdae37b96931786852e5c215381ab21`。Windows 工作区基线为 `6303b26`；产品版本 tag 为 `d32811d`，后续提交只改验收夹具。

使用已授权的固定 Diagnostics 启动器，经 Launch Services 启动独立原生夹具和产品后端。没有更改权限设置或重签启动器。本轮验证的是后端机制，不是新一轮正式应用 UI 或首次授权验收。

夹具独立记录实际回调 PID；前台安全窗口另记收到的目标组合，区分放行与吞键。结果与产品返回的 PID 集合交叉核对，退出夹具后再查一次。默认组合为 `⌃⌥⇧⌘F18`；应用内常用组合只执行夹具的记录动作，不访问剪贴板。所有监听限时、退出时移除。

累计运行 31 次机制测试，覆盖 28 个不同场景；其中三项重复用于补充“只移除独占者或拦截器”的因果对照。另运行 10 个真实常用组合。脚本执行成功不等于产品全部通过，下表保留未覆盖结果。

## 注册与拦截机制

| 场景 | 独立接收证据 | v0.2.2 结果 | 判定 |
| --- | --- | --- | --- |
| Carbon 共享 | 单注册者收到一次 | 临时注册成功；定位 PID 正确 | 归因通过，临时注册成功不表示空闲 |
| Carbon 独占 | 单注册者收到一次 | 冲突 -9878；定位 PID 正确 | 通过 |
| 两个 Carbon 共享 | 两个不同 PID 均收到 | 两个 PID 均被定位 | 通过 |
| 两个 Carbon 独占 | 第二次注册 -9878，仅第一个收到 | 仅定位第一个 | 通过 |
| 共享后独占 | 均注册成功，仅独占者收到 | 仅定位独占者 | 接收者归因正确；不枚举被抑制的共享注册 |
| 独占后共享 | 均注册成功，仅独占者收到 | 仅定位独占者 | 同上 |
| HID / session / annotated 的 listen-only tap（3 项） | 各监听回调收到，前台也收到 | 可临时注册；NotFound | 未枚举被动监听者；被动监听不等于占用 |
| 三位置 active pass tap（3 项） | 各回调收到，前台也收到 | 可临时注册；NotFound | 未定位原始事件处理者 |
| 三位置 active drop tap（3 项） | 各回调收到，前台收不到 | 可临时注册；NotFound | **确认漏检实际吞键软件** |
| Carbon 共享 + session drop | 注册成功；tap 收到，Carbon 与前台均收不到 | 可临时注册；NotFound | **吞键导致注册者归因也消失** |

三项补充对照均成立：两种混合注册顺序中，只退出独占者，剩余共享者恢复接收且被正确定位；Carbon + drop 场景中，只退出 drop 进程，原 Carbon 进程恢复接收且被定位。所有机制场景的全部夹具退出后，检查均恢复可临时注册，定位均为 NotFound，没有残留接收 PID。

第一批三个 drop 夹具只吞目标 key-down；补充交叉场景使用同时吞匹配 key-up 的版本，两者都得到相同的归因缺失。最后检查系统 held-key 数组为空。

## AppKit 与应用内命令

| 场景 | 实际接收 | 产品归因 | 判定 |
| --- | --- | --- | --- |
| NSEvent global，另一窗口前台 | 监听者与前台均收到 | NotFound | 未枚举监听者 |
| NSEvent global，自身前台 | global 回调未收到 | NotFound | 符合 global 排除自身的作用域 |
| NSEvent local pass，自身前台 | local 回调收到 | NotFound | 本应用处理，不是全局注册者 |
| NSEvent local drop，自身前台 | local 回调收到并返回 nil | NotFound | 本应用过滤，不是全局注册者 |
| local pass / drop，另一窗口前台（2 项） | local 回调均未收到 | NotFound | 作用域对照成立 |
| 菜单 F18，自身前台 | 菜单 action 触发 | NotFound | 应用内命令，不应伪造全局占用者 |
| 菜单 F18，另一窗口前台 | 菜单 action 未触发 | NotFound | 作用域对照成立 |
| 菜单 `⌘C / ⌘V / ⌘F / ⌘,`（4 项） | 各记录 action 触发一次 | 均 NotFound | 验证菜单路径；不是实际编辑器复制、粘贴功能测试 |

没有把所有 NotFound 计为失败，也没有将应用内命令的 NotFound 当成“快捷键不存在”。本轮未单独测试 NSResponder、Qt 库、Electron 库或菜单展开时库切换监听方式。

## 真实常用组合

输入源为搜狗拼音。每项记录检测结果及带本次输入标记的目标进程；完成后发 Esc 取消临时界面，恢复原输入源及前台应用。

| 组合 | 普通检查 | 主动定位结果 |
| --- | --- | --- |
| QQ `⇧⌘F` | 系统设置命中 | QQ，PID 85768 |
| QQ `⌃⌘A` | 注册冲突 -9878 | QQ，PID 85768 |
| `⌘Space` | 系统设置命中 | Spotlight，PID 555 |
| `⌃Space` | 系统设置命中 | TextInputMenuAgent，PID 672 |
| `⇧⌘4` | 系统设置命中 | SystemUIServer，PID 496 |
| `⇧⌘5` | 系统设置命中 | SystemUIServer，PID 496 |
| `⌃↑` | 注册冲突 -9878 | Dock，PID 494 |
| `⌃↓` | 注册冲突 -9878 | Dock，PID 494 |
| `⌘Tab` | 系统设置命中 | Dock，PID 494；前台应用确实切换 |
| `⌃⌘Space` | 系统设置命中 | **NotFound** |

QQ 的实际路径为 `/Volumes/fanx/Applications/chat/QQ.app/Contents/MacOS/QQ`。表格只报告观察到的接收进程，不根据闭源应用推断内部注册 API；系统接收进程也不一定是最终界面进程。除 `⌘Tab` 的前台变化外，没有逐项验证最终功能界面或业务动作。因此 `⌃⌘Space` 只能记录“系统设置有组合，未归因”，不能宣称表情面板已正常触发，也不能据此断言没有处理者。

## 复跑与证据

需要已授权、固定签名的 Diagnostics host，以及对应当前产品源码编译的 `owner-production`。不要重签现有授权启动器；首次权限另按 [Mac 验收文档](MACOS.md) 测试。在 Mac 仓库根目录编译小夹具到 host 同级目录：

```sh
clang -fobjc-arc -framework Cocoa -framework Carbon \
  examples/macos-shortcut-mechanisms.m -o .task/owner-proof/owner-mechanisms
clang -fobjc-arc -framework Cocoa -framework Carbon \
  examples/macos-owner-query.m -o .task/owner-proof/owner-query
mkdir -p .task/mechanisms-results
PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-macos-mechanisms.py \
  --app '.task/owner-proof/KeyCrash Diagnostics.app' \
  --output .task/mechanisms-results/matrix.json --trigger-fixture --include-local-controls
PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-macos-common-shortcuts.py \
  --app '.task/owner-proof/KeyCrash Diagnostics.app' \
  --output .task/mechanisms-results/common.json --trigger-real-shortcuts
```

第二个脚本会实际触发上表组合，使用前确认当前配置适合执行。本轮没有触发锁屏、退出、关机或立即保存截图的组合。`--case <名称>` 可重复传给机制脚本，仅复跑指定场景。

本机证据保留于忽略的 `.task/macos-mechanisms-20261013/`：`matrix.json`、`extended.json`、`common.json`、`validation.txt`。Mac 原始日志在任务副本 `.task/mechanisms-20261013/`。逐项核对了实际注册返回、回调数量、Carbon PID 集合、吞键与放行时前台是否收到、移除单个处理者和全部退出后的结果。新夹具 Objective-C 编译、Python 语法和 Git diff 检查另行执行；没有因为增加测试文档重跑整个产品构建。

收尾核对所有本任务夹具已退出、held-key 数组为空，移除了本轮生成的 `owner-mechanisms` 二进制（57472 字节，路径已不存在）。固定授权 host、既有 helper、修改中的源码和必要证据保留；复跑时按上述命令重新编译夹具。

## 明确未覆盖与后续方向

- 媒体、音量、亮度、Fn/Globe 和 IOHID 实际设备输入未测；产品也不支持将这些专用输入当作普通目标键。合成 CGEvent 不能代替真实硬件验证。
- 本轮未新增 Intel、其他 macOS 版本、非美式布局、输入源切换前后、Secure Input 开启状态或沙盒应用的验证。未为测试关闭安全输入。
- 需要补的是原始事件拦截路径的归因证据；仅列出 tap 进程不能证明它匹配或吞掉了目标组合。当前证据也不足以承诺任意软件都能精确归因。
- 发布验收应保留这些独立机制场景，不能仅用 Carbon 换一批键码或成功编译代替完整定位验证。

本轮只增加研究、夹具与验证记录，没有提交、推送、修改产品版本或发布新版本。
