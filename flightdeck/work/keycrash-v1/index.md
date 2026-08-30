# KeyCrash v1

**Status:** Open

## Goal

把 KeyCrash 推进为可信、可测试、可分发的 Windows v1：既安全确认快捷键冲突，也在标准全局热键链路中给出占用软件，并明确非 `WM_HOTKEY` 场景的边界。

## Current

- Rust + Microsoft `windows` / `windows-sys` crates + Slint 的原生应用已实现。
- 主流程为：虚拟 modifier tags + 本地目标键 → `RegisterHotKey` 安全探测 → 已占用时单击“定位占用软件” → x64/x86 进程内 `WH_GETMESSAGE` 归因。
- 安全探测阶段不安装全局 Hook，也不发送完整组合；深度定位由用户显式触发，最多发送两次并尽量把匹配 `WM_HOTKEY` 改为 `WM_NULL`。
- Release 已嵌入 `requireAdministrator` 清单，用于覆盖管理员全局热键注册者。
- 可用、占用或系统保留、不支持、错误状态已实现；默认检测不猜测 owner PID。
- Ctrl / Alt / Shift 可单击选择或移除；检测期间 tags 禁用，Esc 或取消按钮返回待机。
- 目标键支持字母、数字、F1–F24、Space、Enter、Tab、方向键等；单独输入修饰键或完整组合会被拒绝。
- 探测结果结构化为状态、错误码和系统规则；失败 1409 可继续获得进程文件名、完整路径、架构与 PID/TID。
- 默认进程快照与低置信 owner 猜测已移除，避免把时间相关性呈现为归因证据。
- x64 与 x86 受控 `RegisterHotKey` fixtures 均返回了精确 PID，且目标程序未收到原 `WM_HOTKEY`。
- 10 项测试、格式检查、Clippy、双架构 release 构建及新增 UI 状态检查已通过。
- 用户已完成 QQ `Ctrl+Alt+A` 实机验收：只按目标键即可得到 1409，QQ 未被触发。
- 产品、视觉方向和实际设计 tokens 已记录；仓库尚无首次提交。

## Next

启动新版并完成 QQ 实机归因：点选 Ctrl、Alt，只按 `A` 确认 1409，再点“定位占用软件”；预期显示 `QQ.exe`、`C:\Program Files\Tencent\QQNT\QQ.exe` 和 x64 PID/TID，截图动作被 best-effort 拦截。若未命中，按 [归因研究](references/owner-attribution-research.md) 的 QQ 分支记录是低级 Hook/Raw Input 还是模拟输入被忽略。实现入口见 [owner probe](../../../src/owner_probe.rs)。

## Execution pointer

[Plan 4 — 高级 owner 归因原型](plan.md#4-高级-owner-归因原型)

## Recent progress

- 纠正“只判冲突即可”的产品偏差，确认 Hotkey Screener 使用全局消息 Hook + 真实触发来返回响应应用。
- 用一次性原型验证 `PM_REMOVE + WM_HOTKEY` 可精确记录 PID 并改写为 `WM_NULL`；随后将结论独立重写进正式 x64/x86 Helper/DLL。
- 双架构受控 fixtures、UAC manifest、异步 UI 归因状态和可复现 release 脚本均已通过验证；当前 release SHA-256 为 `2EC5C7C5B8E6676474C05D3DC07888BF7D54CCCBB78C465B700D720CF2933249`，等待 QQ 实机归因。

## References

- [稳定上下文](context.md)
- [占用者归因研究](references/owner-attribution-research.md)
- [Windows 快捷键调研](../../../../docs/research/windows-hotkey-conflict-detector.md)
- [界面实现](../../../ui/app-window.slint)
