# KeyCrash

一按，找到快捷键冲突。

KeyCrash 是一个 Windows 快捷键冲突与占用软件检测工具。单击选择 Ctrl、Alt、Shift，点击“开始检测”后只在 KeyCrash 当前窗口按一个目标键；程序先用 `RegisterHotKey` 判断组合是否冲突，已占用时可继续定位接收匹配 `WM_HOTKEY` 的软件。

## 当前范围

- Windows 10/11 x64
- 可关闭的 Ctrl / Alt / Shift tags
- 当前窗口单目标键捕获，不真实发送完整组合
- 区分可用、已占用或系统保留、不支持和系统错误
- 安全检测阶段不安装全局 Hook、不真实发送完整组合
- Release GUI 以普通权限启动；普通 x64/x86 定位未命中时，才请求一次管理员权限继续覆盖高完整性注册者
- 已占用时可进一步定位接收 `WM_HOTKEY` 的软件、显示路径并打开文件位置
- 深度定位最多真实触发四次组合；原动作可能执行
- 右上角“？”提供同窗使用说明
- 不猜测进程；未观察到匹配消息时明确显示技术边界

> 发布前需要明确选择 Slint 的 GPLv3、Royalty-free attribution 或商业许可路径。

## 开发

```powershell
cargo run
cargo test
./scripts/build-release.ps1
```
