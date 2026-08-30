# KeyCrash

一按，找到快捷键冲突。

KeyCrash 是一个 Windows 快捷键冲突与占用软件检测工具。单击选择 Ctrl、Alt、Shift，点击“开始检测”后只在 KeyCrash 当前窗口按一个目标键；程序先用 `RegisterHotKey` 判断组合是否冲突，已占用时可继续定位接收匹配 `WM_HOTKEY` 的软件。

## 当前范围

- Windows 10/11 x64
- 可关闭的 Ctrl / Alt / Shift tags
- 当前窗口单目标键捕获，不真实发送完整组合
- 区分可用、已占用或系统保留、不支持和系统错误
- 安全检测阶段不安装全局 Hook、不真实发送完整组合
- Release 默认请求管理员权限，用于短时安装 x64/x86 进程内消息 Hook
- 已占用时可进一步定位接收 `WM_HOTKEY` 的软件并显示路径
- 深度定位最多真实触发两次组合，并尽量拦截标准热键动作
- 不猜测进程；未观察到匹配消息时明确显示技术边界

> 发布前需要明确选择 Slint 的 GPLv3、Royalty-free attribution 或商业许可路径。

## 开发

```powershell
cargo run
cargo test
./scripts/build-release.ps1
```
