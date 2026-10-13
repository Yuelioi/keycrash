# KeyCrash

**Find out why your shortcut isn't working.**

[简体中文](README.md) · English · [日本語](README.ja.md)

KeyCrash checks for global hotkey registration conflicts and tries to identify the app using it.

For **Windows 10 / 11 (64-bit) and macOS**. Available in Chinese, English, and Japanese.

v0.2.0 was withdrawn. v0.2.1 adds Mac app attribution for exclusive and shared hotkeys. See the [macOS guide](docs/MACOS.md) (Chinese) for scope and validation.

![KeyCrash identifies QQ as the app using Ctrl + Alt + A (Chinese interface)](docs/images/keycrash-screenshot.png)

## Download and run

Download `keycrash-v<version>-windows-x64.zip` from [Releases](https://github.com/Yuelioi/keycrash/releases).

Extract the entire archive and run `keycrash.exe`. Keep the other files and the `owner-x86` folder alongside it.


For M-series Macs, download `macos-arm64.zip`; for Intel Macs, use `macos-x86_64.zip`. Extract and run `KeyCrash.app`. Mac packages are ad-hoc signed and not notarized.

## How to use

1. Select **Ctrl / Alt / Shift / Win** as needed. Click again to deselect, or leave all four off.
2. Click **Start detection**, then press only the target key inside KeyCrash. To check `Ctrl + Shift + K`, select Ctrl and Shift, then press K.
3. Read the result. If a registration conflict is found, choose **Locate**. When an app is found, you can open its file location.

Press **Esc** to cancel waiting for a key. Use the top-right language menu or **?** for help.

## Please note

A normal check does not trigger the full shortcut. **Locating an app may trigger the shortcut's original action** and may request administrator permission on Windows. No registration conflict does not rule out keyboard hooks or other listeners. On Windows, F12 is shown as system-reserved. Temporary registrations by other apps count as conflicts while they remain active.

On Mac, use ⌃ Control / ⌥ Option / ⇧ Shift / ⌘ Command. Choose Locate after checking and allow KeyCrash in Accessibility settings when prompted. Shared hotkeys may allow temporary registration, so Locate is available in that result too. No matching receiver means attribution is unknown; app-local shortcuts and arbitrary listeners are outside this scope.
