# KeyCrash v1 Plan

## 1. 安全快捷键输入

**Status:** Completed

- [x] Ctrl / Alt / Shift 使用可选择、可移除的虚拟 tags。
- [x] 点击开始后只在 KeyCrash 当前窗口读取一个普通目标键，再内部合成待测组合。
- [x] 不安装全局键盘 hook，不发送完整快捷键，不做默认 owner 进程猜测。
- [x] 覆盖目标键解析、修饰键拒绝、Esc 取消、占用探测与格式顺序。
- 验收：格式、测试、Clippy、release 构建全部通过。
- [x] 实机验收：点选 Ctrl 与 Alt 后只按 `A`，QQ 未触发截图，KeyCrash 返回错误 1409。
- 结论：安全输入实现、自动验证和 QQ 实机验收均已完成。

## 2. 证据分类与系统保留规则

**Status:** Completed

- 扩展可解释的 OS reserved/system behavior 规则。
- 统一结果证据、错误码和面向用户的恢复建议。
- 不把配置声明或静态规则冒充运行时 owner。

## 3. 安全来源缩小

**Status:** Pending

- [ ] 设计并实现脱敏诊断报告模型与导出入口。
- [ ] 评估是否存在不靠时间相关性猜测的来源缩小证据。
- [x] 默认路径不猜测、终止、暂停或修改第三方进程。

## 4. 高级 owner 归因原型

**Status:** In progress

- [x] 验证 `WH_GETMESSAGE` 在 `PM_REMOVE` 时可观察匹配 `WM_HOTKEY`、记录目标 PID，并改写为 `WM_NULL`。
- [x] 独立实现 x64/x86 Helper + DLL；使用命名共享内存与 Event 传递固定 POD 证据。
- [x] Release 嵌入管理员清单；深度定位最多按 x64 → x86 顺序触发两次组合。
- [x] x64/x86 受控 fixtures 均返回精确 PID，且标准热键动作被拦截。
- [x] UI 显示进程名、路径、架构、PID/TID 和 suppression 证据；未命中时不猜测。
- [ ] QQ `Ctrl+Alt+A` 实机归因：确认是否命中 QQ 的 `WM_HOTKEY`，以及截图动作是否被拦截。
- [ ] 为 elevated owner 和线程型 `RegisterHotKey(NULL, ...)` 增加正式可重复 fixture。

## 5. Windows v1 交付

**Status:** Pending

- 完成许可证选择、版本信息、安装/便携包和代码签名决策。
- 建立 Windows 10/11、DPI 和升级冒烟测试。
- 形成可复现 release 检查与用户文档。
