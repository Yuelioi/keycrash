# KeyCrash v1

**Status:** Open

## Goal

把 KeyCrash 推进为可信、可测试、可分发的 Windows v1：既安全确认快捷键冲突，也在标准全局热键链路中给出占用软件，并明确非 `WM_HOTKEY` 场景的边界。

## Current

- Rust + Microsoft `windows` / `windows-sys` crates + Slint 的原生应用已实现。
- 主流程为：虚拟 modifier tags + 本地目标键 → `RegisterHotKey` 安全探测 → 已占用时单击“定位占用软件” → x64/x86 进程内 `WH_GETMESSAGE` 归因。
- 安全探测阶段不安装全局 Hook，也不发送完整组合；深度定位由用户显式触发，普通/管理员双权限与 x64/x86 双位数最多发送四次，并尽量把匹配 `WM_HOTKEY` 改为 `WM_NULL`。
- Release GUI 已改为 `asInvoker`；普通 x64/x86 定位均未命中时，才用一次 UAC 启动管理员 x64 coordinator 并覆盖管理员 x64/x86 注册者。
- 可用、占用或系统保留、不支持、错误状态已实现；默认检测不猜测 owner PID。
- Ctrl / Alt / Shift 可单击选择或移除；检测期间 tags 禁用，Esc 或取消按钮返回待机。
- 目标键支持字母、数字、F1–F24、Space、Enter、Tab、方向键等；单独输入修饰键或完整组合会被拒绝。
- 第三方全局热键可能吞掉 F1、`PrtSc` 等普通窗口事件；等待态对全部受支持目标键使用仅限 KeyCrash 前台进程的 8ms `GetAsyncKeyState` 按下边沿兜底，不安装全局 Hook，也不发送组合。
- 探测结果结构化为状态、错误码和系统规则；失败 1409 可继续获得进程文件名、完整路径、架构与 PID/TID。
- owner 成功态标题去掉 `.exe`，只显示“软件名占用了它”；完整路径保留，并提供“打开文件位置”在资源管理器中选中可执行文件。证据不再宣称原动作已拦截。
- 右上角“？”打开同窗三步使用说明，“×”或 Esc 返回；定位过程中入口禁用。
- 描边次操作区分“焦点所有权”和“焦点环显示”：鼠标不夺取焦点；Tab 或辅助技术聚焦时显示黑色焦点环。
- 默认进程快照与低置信 owner 猜测已移除，避免把时间相关性呈现为归因证据。
- x64 与 x86 受控 `RegisterHotKey` fixtures 均返回了精确 PID，且目标程序未收到原 `WM_HOTKEY`；正式线程消息 fixture 另覆盖 Ctrl、裸键、裸键 + `MOD_NOREPEAT`。
- 18 项测试、workspace 测试、格式检查、Clippy、Slint 检查、owner fixture、Release 构建、三态视觉检查与 UIA 功能验证已通过。
- 主界面已支持中文、English、日本語三语切换；帮助、可访问性和运行时结果同步本地化，切换保留当前状态。
- 用户已完成 QQ `Ctrl+Alt+A` 实机验收：只按目标键即可得到 1409，QQ 未被触发。
- 产品、视觉方向和实际设计 tokens 已记录；恢复后的快照与焦点语义修复已分别提交。

## Next

让用户目视验收中、英、日三种语言的待机、帮助和 owner 结果态；随后继续 QQ `Ctrl+Alt+A` 归因验收，并补正式 elevated owner fixture。实现入口见 [界面实现](../../../ui/app-window.slint) 与 [语言包](../../../src/i18n.rs)。

## Execution pointer

[Plan 4 — 高级 owner 归因原型](plan.md#4-高级-owner-归因原型)

## Recent progress

- 纠正“只判冲突即可”的产品偏差，确认 Hotkey Screener 使用全局消息 Hook + 真实触发来返回响应应用。
- 用一次性原型验证 `PM_REMOVE + WM_HOTKEY` 可精确记录 PID 并改写为 `WM_NULL`；随后将结论独立重写进正式 x64/x86 Helper/DLL。
- 双架构受控 fixtures、按需 UAC、异步 UI 归因状态和可复现 release 脚本均已通过验证；当前 release SHA-256 为 `307F4D4E3762CC6D29DB55E397C22BC9E66D6B3CA6E8FC7F2C4F0CD0495D9A40`，等待 UI 收尾、QQ 与 elevated owner 最终实机验收。
- 用户验收发现物理 `PrtSc`、F1 都可被第三方全局热键在 Winit 前吞掉；已将前台等待态异步键状态兜底扩展到全部受支持目标键，并覆盖按住不重复、预先按住、物理修饰键和各按键族。
- 差分复现确认：管理员前台窗口会让普通 Snipaste 的 F1 注入链返回 NOT_FOUND；同一 helper 在普通前台精确返回 Snipaste PID 14456。GUI 已按研究结论改为普通权限，管理员覆盖收进按需 helper seam；旧红灯退出码 2 在新版普通前台变为 0。
- UIA + 80ms F1 红灯从 `WAITING=True / CAPTURED_F1=False` 转为 `WAITING=False / CAPTURED_F1=True / BLOCKED=True`；随后自动定位显示 Snipaste.exe 与版本化路径，完整输入到 owner 链路已转绿。
- 按用户验收清单新增 owner 文件位置动作与同窗帮助，移除 `.exe`、本地捕获范围和拦截成功文案；真实 F5 owner fixture 验证标题、路径按钮、Explorer 打开、帮助 Esc 返回与可访问性树，待机/帮助/owner 三态视觉检查无溢出。
- 用 Slint 1.17 的 `focus-on-click: false` 与 `FocusReason` 分离焦点所有权和视觉；真实鼠标点击后焦点为 `False`，Tab 聚焦为 `True`，截图确认黑色焦点环仍显示。
- 新增中 / EN / 日语言切换，集中语言包覆盖主流程结果，Slint 覆盖帮助、固定标签与可访问性；20 项测试、格式、Clippy 与 Release 构建通过。

## References

- [稳定上下文](context.md)
- [占用者归因研究](references/owner-attribution-research.md)
- [Windows 快捷键调研](../../../../docs/research/windows-hotkey-conflict-detector.md)
- [界面实现](../../../ui/app-window.slint)
