# KeyCrash

**一按，找到快捷键冲突。**

简体中文 · [English](README.en.md) · [日本語](README.ja.md)

快捷键突然没反应？KeyCrash 帮你检查组合键是否存在注册冲突，并尝试找出占用它的软件。

适用于 **Windows 10 / 11（64 位）和 macOS**；支持中文、英文和日文。

![KeyCrash 检测到 QQ 占用了 Ctrl + Alt + A](docs/images/keycrash-screenshot.png)

## 下载与运行

在 [Releases](https://github.com/Yuelioi/keycrash/releases) 下载 `keycrash-v<版本号>-windows-x64.zip`。

完整解压后运行 `keycrash.exe`。请保留同目录的其他文件和 `owner-x86` 文件夹，不要只复制主程序。

Mac mini M4 / M 系列下载 `macos-arm64.zip`，Intel Mac 下载 `macos-x86_64.zip`，解压运行 `KeyCrash.app`。Mac 包采用 ad-hoc 签名，尚未公证。

## 怎么用

1. 选择需要的 **Ctrl / Alt / Shift / Win**；再次点击可取消，也可以都不选。
2. 点击 **开始检测**，在 KeyCrash 窗口里只按目标键。例如检查 `Ctrl + Shift + K`，先选 Ctrl 和 Shift，再按 K。
3. 查看结果。如果发现注册冲突，点击 **定位占用软件**；找到后可打开它的文件位置。

按 **Esc** 可取消等待按键。右上角可以切换语言，点击 **?** 查看说明。

## 使用前了解

普通检测不会触发完整快捷键；**定位占用软件可能触发快捷键原本的操作**，Windows 必要时会请求管理员权限。

Mac 使用 ⌃ Control / ⌥ Option / ⇧ Shift / ⌘ Command。首次定位时，请在系统设置中允许 KeyCrash 的辅助功能权限。检测后可点击“定位占用软件”，也可点“检查全局监听”并选择参与检查的软件。全局监听检查期间，所选软件的其他快捷键或按键重映射可能短暂停止；检查结束或取消后自动恢复。
