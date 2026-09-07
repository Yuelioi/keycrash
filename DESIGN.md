---
name: KeyCrash
description: 一按，让快捷键冲突成为一条可读的信号。
colors:
  collision-red: "#c9342a"
  ready-teal: "#007a7e"
  unsupported-ochre: "#8b6b16"
  error-rust: "#a33b32"
  ink: "#11171c"
  canvas-cool-gray: "#dce2e5"
  panel-frost: "#edf1f2"
  readout-white: "#f8fafb"
  inverse-white: "#ffffff"
  logo-white: "#f3f6f7"
  border-steel: "#b8c2c7"
  tag-border: "#aab6bc"
  tag-text: "#344249"
  body-muted: "#45545b"
  subtitle-muted: "#536168"
  evidence-muted: "#5e6c72"
  footer-muted: "#59676d"
  grid-active-horizontal: "#cdd5d8"
  grid-active-vertical: "#d7dddf"
  grid-idle-horizontal: "#e4e9ea"
  grid-idle-vertical: "#e8eced"
typography:
  headline:
    fontSize: "25px"
    fontWeight: 700
  brand:
    fontSize: "16px"
    fontWeight: 700
  shortcut:
    fontSize: "15px"
    fontWeight: 650
  body:
    fontSize: "13px"
  action:
    fontSize: "13px"
    fontWeight: 650
  modifier-tag:
    fontSize: "12px"
    fontWeight: 650
  label:
    fontSize: "10px"
    fontWeight: 700
rounded:
  indicator: "2px"
  brand: "8px"
  tag: "9px"
  control: "10px"
  panel: "14px"
spacing:
  micro-gap: "7px"
  brand-gap: "10px"
  control-gap: "14px"
  panel-inset: "18px"
  window-gutter: "28px"
components:
  action-button:
    backgroundColor: "{colors.collision-red}"
    textColor: "{colors.inverse-white}"
    typography: "{typography.action}"
    rounded: "{rounded.control}"
    height: "44px"
    width: "154px"
  instrument-panel:
    backgroundColor: "{colors.panel-frost}"
    rounded: "{rounded.panel}"
    height: "142px"
    width: "404px"
  shortcut-readout:
    backgroundColor: "{colors.readout-white}"
    textColor: "{colors.ink}"
    typography: "{typography.shortcut}"
    rounded: "{rounded.control}"
    height: "62px"
    width: "162px"
  modifier-tag-selected:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.inverse-white}"
    typography: "{typography.modifier-tag}"
    rounded: "{rounded.tag}"
    height: "30px"
    width: "58px"
  modifier-tag-unselected:
    backgroundColor: "{colors.readout-white}"
    textColor: "{colors.tag-text}"
    typography: "{typography.modifier-tag}"
    rounded: "{rounded.tag}"
    height: "30px"
    width: "58px"
---

# Design System: KeyCrash

## Overview

**Creative North Star: "维修台信号仪"**

KeyCrash 像一台放在 Windows 桌面上的便携维修仪：冷灰外壳保持安静，浅色测量面板承载证据，墨黑读数保证判断清晰。用户先用 Ctrl、Alt、Shift 标签组成虚拟修饰键，再让当前窗口接收一个目标键；进入等待阶段时，单一状态色沿三条短轨汇入快捷键读数。它有仪器感，但不模拟厚重硬件，也不走深色霓虹控制台路线。

界面保持紧凑、直接和可信。视觉强度来自可编辑的修饰键标签、精确的轨迹、网格、状态词与一枚主按钮，而不是导航、仪表盘或信息堆叠。安全探测落定后轨迹停止；用户继续定位占用软件时轨迹再次出现，命中后直接显示进程名、路径和 PID/TID 证据。未观察到匹配消息时必须明确说未命中，不能猜测 owner。

规范来源是 `ui/app-window.slint` 中的 Slint 实现，并以 `safe-idle-final4.png`、`safe-waiting-final4.png`、`owner-locating.png`、`owner-found-3.png` 和 `owner-missed.png` 校验；运行时状态文案和状态色由 `src/main.rs` 注入。

**Key Characteristics:**

- 冷灰、浅霜白与墨黑构成克制的维修仪表底盘。
- 同一时刻只有一个状态色贯穿标志、轨迹、读数边框、状态标签、指示点和主操作。
- 可增删的修饰键标签与一个窗口内目标键先完成安全查询；只有用户选择定位软件后才真实触发组合。
- 三路短轨在等待目标键或定位占用进程时汇入快捷键胶囊，是系统最具识别度的动态签名。
- 单窗口、单任务、单主操作；结果与证据始终同屏。

## Colors

色彩以冷中性层次建立可信的仪器背景，再让红、青、赭和锈红分别承担真实状态；每种状态色都与文字结论同时出现。

### Primary

- **碰撞红**：默认、等待目标键与“已占用或系统保留”状态的强调色，也用于主按钮和品牌下划线。

### Secondary

- **就绪青**：“当前可用”结果的确认色，让完成态明确但不带庆祝感。

### Tertiary

- **受限赭**：标准 Windows 全局热键机制无法表达或检测该组合时使用。
- **错误锈红**：系统错误或检测无法启动时使用，与可确认的占用状态保持差异。

### Neutral

- **仪表墨黑**：标题、快捷键读数、品牌键帽和键盘焦点轮廓的最高对比基准。
- **冷灰机身**：窗口底色，将应用塑造成一件轻量桌面仪器。
- **霜白面板**：测量区域的主体表面；比窗口底色更亮，但不使用纯白大卡片。
- **读数白**：快捷键胶囊与面板之间的局部抬升层。
- **钢灰边界**：测量面板的一像素结构边框。
- **标签钢线**：未选择 ModifierTag 的一像素边框；选中后边框与背景一同切换为仪表墨黑。
- **标签文字灰**：未选择 ModifierTag 的前景色；选中后改用纯白。
- **分级静音文字**：正文、品牌副标题、证据和页脚各使用相邻冷灰，维持小字号可读性又不争夺结论。
- **测量网格**：仅等待目标键时加深，待机和结果态减弱；横纵线使用相邻但不完全相同的冷灰。

### Named Rules

**The One Active Signal Rule.** 同一状态只使用一个强调色，并让它同步驱动品牌下划线、轨迹、读数边框、状态标签、指示点和主按钮。

**The Color Never Stands Alone Rule.** 可用、占用、受限和错误都必须同时提供中文结论与文字证据，不能只靠青、红、赭或锈红表达；ModifierTag 也必须同时通过 `+` / `×` 文案表达未选与已选。

## Typography

**Display Font:** Slint 平台默认字体（未显式指定字体族）  
**Body Font:** Slint 平台默认字体（未显式指定字体族）  
**Label/Mono Font:** Slint 平台默认字体（英文状态标签未使用等宽字体）

**Character:** 字体系统依赖 Windows 熟悉的系统字形，靠字号、字重和语义位置建立层级。标题坚实，读数紧凑，证据轻声；不通过装饰性字体制造仪器感。

### Hierarchy

- **Headline**（700，25px）：当前检测阶段或最终结论，是每个状态的首要阅读点。
- **Brand**（700，16px）：仅用于 KeyCrash 品牌名；键帽字母使用相近的 17px / 700 处理。
- **Shortcut**（650，15px）：快捷键胶囊中的核心读数。
- **Body**（默认字重，13px）：状态解释；主按钮沿用 13px，但提高到 650。
- **Modifier Tag**（650，12px）：Ctrl、Alt、Shift 的添加与移除标签。
- **Label**（700，10px）：`STANDBY`、`LOCAL INPUT`、`READY`、`BLOCKED`、`ERROR` 等短状态词；证据文字同为 10px，但保持默认字重。

### Named Rules

**The Readout Before Metadata Rule.** 用户先读中文结论和快捷键，再读英文状态词与证据；小型仪器标签不得超过主要读数的视觉重量。

## Layout

主窗口固定为 460×380，采用 28px 左右窗边距和清晰的三段纵向结构：顶部品牌与状态区、中部测量面板、底部状态提示与主操作。顶部从 24px 开始，右上角保留 30×30 的“？”使用说明入口；测量面板位于 y=138px，尺寸为 404×142；底部操作行位于 y=314px，高 44px。帮助态保留品牌栏，并用同一块 404×286 霜白仪器面板替换任务内容，不弹出第二窗口。

测量面板左上以 7px 间距排列三个 58×30 ModifierTag，非工作态在其下方显示添加或移除说明；`waiting-key` 与 `locating` 时标签锁定变灰，说明让位给三路轨迹。右侧固定为 162×62 的目标键读数胶囊。18px 面板内缩控制标签组、网格与证据基线；底部使用 14px 间距把状态提示和 154×44 主按钮分开。当前实现没有断点或响应式重排，尺寸依赖 Slint/Windows DPI 缩放。

**The One-Screen Instrument Rule.** 主要检测路径必须在单个 460×380 窗口内完成，不加入导航、滚动表格或第二个并列主操作。

## Elevation & Depth

系统使用“色调分层 + 单一结构阴影”的混合策略。冷灰窗口、霜白测量面板和更亮的读数胶囊形成三级表面；只有测量面板获得阴影，按钮和读数不通过额外悬浮阴影争夺注意力。

### Shadow Vocabulary

- **仪器面板阴影**（Slint：颜色 `#26343c26`，纵向偏移 7px，模糊 18px）：只用于把测量面板从冷灰窗口中轻轻抬起。

### Named Rules

**The Single Lift Rule.** 每个窗口只抬升主测量面板；其他层次由色调、边框和焦点轮廓表达。

## Shapes

形态由紧凑圆角矩形和正交信号轨迹组成。主测量面板使用 14px 圆角，按钮与快捷键读数使用 10px，ModifierTag 使用 9px，品牌键帽使用 8px，状态点使用 2px。轨迹始终由 2px 粗的水平和垂直线段组成，以直角转折保持逻辑分析仪的精确感。

键盘焦点在按钮外扩 3px，以 2px 仪表墨黑轮廓和 13px 圆角显示；它是明确的操作状态，不是装饰光晕。

## Components

### Buttons

- **Shape:** 紧凑、稳定的 10px 圆角矩形，当前主实例为 154×44。
- **Primary:** 背景使用当前状态强调色，文字使用纯白、13px / 650，并保持居中。
- **Hover / Focus:** 悬停透明度为 90%，按下为 78%，禁用为 42%；透明度以 110ms `ease-out` 过渡。键盘焦点显示外扩的墨黑实线轮廓，空格与回车均可触发。
- **Behavior:** 同一按钮复用“开始检测”“取消”“定位占用软件”“定位中…”“再测一次”“重新检测”等真实状态文案。定位中禁用按钮，防止重复触发；按钮始终对应当前画面的唯一下一步。
- **Secondary / Utility:** owner 成功态在主按钮左侧显示描边“打开文件位置”，打开资源管理器并选中可执行文件；右上角描边“？”在帮助态切换为“×”。两者都支持 Tab、Space、Enter；实线焦点轮廓只在键盘导航获得焦点时出现，鼠标点击后保持清爽的默认钢灰描边。

### Modifier Tags

- **Geometry:** Ctrl、Alt、Shift 均为 58×30，使用 9px 圆角、12px / 650 字体，标签间距为 7px。
- **Selected:** 仪表墨黑背景与边框、纯白文字，显示 `Ctrl  ×`、`Alt  ×` 或 `Shift  ×`；再次单击即可移除。
- **Unselected:** 读数白背景、标签文字灰和 1px 标签钢线，显示 `+  Ctrl`、`+  Alt` 或 `+  Shift`。
- **States:** 悬停透明度为 90%，按下为 76%，`waiting-key` / `locating` 锁定态为 58%；背景与透明度以 110ms `ease-out` 过渡。键盘焦点在外扩 2px 处显示 2px 碰撞红轮廓，空格与回车可切换。

### Cards / Containers

- **Corner Style:** 主测量面板为 14px 圆角；快捷键读数胶囊为 10px。
- **Background:** 面板使用霜白，读数胶囊使用更亮的读数白。
- **Shadow Strategy:** 仅主测量面板使用仪器面板阴影。
- **Border:** 面板使用 1px 钢灰边框；读数胶囊使用 1px 当前状态色边框。
- **Internal Padding:** 网格与底部证据基线使用 18px 侧向内缩；读数胶囊文字区使用 14px 左右内边距。

### Signal Trace

三条独立的 2px 正交轨道在快捷键胶囊前汇入一根探针。轨迹只在 `waiting-key` 或 `locating` 状态可见，使用当前状态强调色，并以 160ms `ease-out` 淡入淡出；结果落定后必须消失，让结果态保持安静。

### Shortcut Readout

快捷键读数是面板中的唯一亮白胶囊。待机时显示“等待目标键”；`waiting-key` 时显示所选修饰键加省略目标位，例如“Ctrl + Alt + …”；提交一个目标键后显示完整的标准化名称，例如“Ctrl + Alt + A”。边框与当前状态色同步，但读数文字始终保持墨黑；当前实现没有 Spinner 或独立的检测中画面。

### Local Target-Key Input

开始后，Slint 的 `FocusScope` 只在 `waiting-key` 模式启用，并只接收 KeyCrash 当前窗口中的一个未带修饰键的目标键。`Esc` 取消；包含 Ctrl、Alt、Shift 或 Meta 的按键事件被吞掉而不提交。程序把这个目标键与界面中预选的 ModifierTag 组合后执行安全注册探测；此阶段不安装全局 Hook，也不发送完整快捷键。

**The Local Input Only Rule.** ModifierTag 负责修饰键，窗口焦点只负责单个目标键；两者不得退化为系统范围的完整组合监听。

### Owner Attribution

只有运行时探测返回 1409 且未命中明确系统规则时，主按钮才切换为“定位占用软件”。用户点击后，普通权限 x64/x86 Helper 先定位普通软件；均未命中时才请求一次 UAC，由管理员 x64 coordinator 覆盖 x64/x86 高完整性注册者。四个 Helper 最多各触发一次完整组合；匹配位数的 `WH_GETMESSAGE` DLL 在目标进程取走对应 `WM_HOTKEY` 时记录 PID/TID，并把消息改为 `WM_NULL`。成功态必须显示不带 `.exe` 的软件名、完整路径、架构与 PID/TID，并提供打开文件位置；界面不宣称原动作已拦截。未命中态必须列出低级 Hook、Raw Input、驱动或忽略模拟输入等边界。定位期间按钮禁用，完成或超时后 Hook 立即卸载。

**The Observed Owner Rule.** 只有进程内观察到精确匹配的 `WM_HOTKEY` 才能命名占用软件；不可用性、前台进程或时间相关性都不能冒充 owner 证据。

### Status Messaging

- **Idle / STANDBY:** “选择修饰键”与“单击 Ctrl、Alt、Shift；开始后只按一个目标键。”；读数为“等待目标键”，证据为“安全输入 · 不发送完整组合”。
- **Waiting Key / LOCAL INPUT:** “请按一个目标键”与“只按 A、F12、Space 等目标键；不要再按修饰键。”；证据位不重复说明窗口范围，页脚提示“Esc 取消 · 只按目标键”。
- **Available / READY:** “这个组合当前可用”；证据为“运行时探测 · 可直接注册”。
- **Occupied / BLOCKED:** “已被占用或系统保留”；明确定位会真实触发组合且原动作可能执行，主按钮为“定位占用软件”。
- **Locating / LOCATING:** “正在定位占用软件”；证据为“普通 / 管理员 × x64 / x86”，按钮禁用。
- **Owner Found / OWNER:** 标题直接使用“`QQ占用了它`”形式；下方显示完整路径，证据只显示架构与 PID/TID，底部提供“打开文件位置”。
- **Owner Missed / ERROR:** “已占用，但没有观察到软件”；解释低级 Hook、Raw Input、驱动和模拟输入边界，不猜测进程。
- **Unsupported / STANDBY:** “没有识别到目标键”；证据为“本地输入 · 未发送完整组合”。
- **Error / ERROR:** “检测没有完成”；证据显示实际 Windows 系统错误码。

### In-Window Help

右上角“？”打开同窗帮助面板，按顺序说明选择修饰键、输入目标键、定位占用软件，并明确定位可能执行原快捷键动作。“×”或 Esc 返回原状态；定位进行中禁用帮助入口，避免遮蔽进行中的系统操作。

## Do's and Don'ts

### Do:

- **Do** 在界面品牌名右侧以 11px 次级文字显示 `v<版本号>`，窗口标题同步显示；版本来自 `Cargo.toml`，不在 UI 中写死。
- **Do** 将右上角语言入口收敛为单个 112×30px 语言菜单，使用“简体中文 / English / 日本語”原语言名称与小箭头；保留 1px 浅灰细边框与浅色底，鼠标选择后不显示黑色焦点框，仅键盘导航时加深细边框。定位进行中禁用，切换时保留检测结果；支持方向键、回车选择、Esc 及点击外部关闭。
- **Do** 让当前状态色贯穿所有信号节点，并让结果文字与颜色同步更新。
- **Do** 让 Ctrl、Alt、Shift 通过可再次单击移除的 ModifierTag 明确表达当前选择。
- **Do** 只在 `waiting-key` / `locating` 期间显示网格强化与信号轨迹，结果落定后恢复低噪声。
- **Do** 保持结论、快捷键和证据同屏，并为每个状态提供清晰的下一步按钮文案。
- **Do** 保留 Slint 原生键盘焦点、标签的空格/回车切换，以及 `waiting-key` 中的 `Esc` 取消行为。

### Don't:

- **Don't** 把维修台信号仪做成深色霓虹、发光 HUD 或多卡片仪表盘。
- **Don't** 加入全量快捷键列表、导航侧栏或需要滚动查找的主路径。
- **Don't** 用装饰动画让结果态持续闪烁；轨迹应在结论落定后停止。
- **Don't** 在安全检测阶段安装全局 Hook，或让短时归因 Hook 在完成/超时后继续存在。
- **Don't** 仅凭 1409、前台窗口或进程时间相关性命名占用软件；必须有匹配 `WM_HOTKEY` 的进程内证据。
