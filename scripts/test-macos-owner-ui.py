#!/usr/bin/env python3
"""Drive the actual Mac window through Accessibility and a local target key.

Requires an already authorized fixed diagnostics host and sibling keycrash.
Only this script's app and registered fixture are controlled or terminated.
"""
import argparse
import importlib.util
import json
import os
import signal
import subprocess
import tempfile
import time
from pathlib import Path

spec = importlib.util.spec_from_file_location('gui', Path(__file__).with_name('test-macos-owner-gui.py'))
gui = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gui)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app', type=Path, required=True)
    parser.add_argument('--trigger-fixture', action='store_true', required=True)
    parser.add_argument('--qq-shortcut', action='store_true',
                        help='Exercise the user-configured Shift+Command+F and require QQ attribution')
    args = parser.parse_args()
    app = args.app.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='ui-', dir=app.parent) as temp:
        work = Path(temp)
        ui = gui.AppProcess(app, work, 'window', 'product-ui', 0)
        pid = ui.record()['pid']
        owner = None
        sequence = 0

        def invoke(mode, value=None):
            nonlocal sequence
            sequence += 1
            output = work / f'action-{sequence}.jsonl'
            command = ['open', '-n', '-W', '--stdout', str(output), str(app), '--args', mode, str(pid)]
            if value is not None:
                command.append(str(value))
            subprocess.run(command, timeout=10)
            time.sleep(.2)
            return [json.loads(line) for line in output.read_text().splitlines()] if output.exists() else []

        def press(label):
            records = invoke('ui-press', label)
            assert any(r.get('kind') == 'pressed' and r['status'] == 0 for r in records), label

        def wait_text(expected):
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                records = invoke('ui-tree')
                if any(r.get('value') == expected for r in records):
                    print('UI:', expected, flush=True)
                    return records
            raise AssertionError(f'Missing UI text: {expected}: {records}')

        try:
            wait_text('选择修饰键')
            if args.qq_shortcut:
                for label in ('添加 ⇧ Shift', '添加 ⌘ Cmd'):
                    press(label)
                press('开始检测')
                invoke('ui-key', 3)
                records = invoke('ui-tree')
                assert any(r.get('title') == '定位占用软件' for r in records), records
                press('定位占用软件')
                records = wait_text('QQ占用了它')
                assert any('/QQ.app/' in r.get('value', '') for r in records), records
                print('PASS: actual QQ Shift+Command+F owner and executable path', flush=True)
                return
            for label in ('添加 ⌃ Ctrl', '添加 ⌥ Opt', '添加 ⇧ Shift', '添加 ⌘ Cmd'):
                press(label)
            owner = gui.AppProcess(app, work, 'owner', 'hold', 79, hold=True)
            ready = owner.record()
            assert ready['status'] == 0
            owner.record()
            press('开始检测')
            wait_text('请按一个目标键')
            invoke('ui-key', 79)
            wait_text('此组合可临时注册')
            press('定位占用软件')
            records = wait_text('owner-query占用了它')
            assert any(f'PID {ready["pid"]}' in r.get('value', '') for r in records)
            assert any(r.get('title') == '打开文件位置' for r in records)
            assert owner.record()['kind'] == 'received'
            owner.close()
            owner = None
            press('再测一次')
            invoke('ui-key', 79)
            wait_text('此组合可临时注册')
            press('定位占用软件')
            wait_text('没有观察到占用软件')
            press('重新检测')
            invoke('ui-key', 79)
            wait_text('此组合可临时注册')
            press('定位占用软件')
            press('取消')
            wait_text('选择修饰键')
            time.sleep(1)
            wait_text('选择修饰键')  # A cancelled result must not repaint the UI.
            press('开始检测')
            wait_text('请按一个目标键')
            press('取消')
            wait_text('选择修饰键')
            print('PASS: real window capture, shared owner PID/path, release, cancel', flush=True)
        finally:
            if owner is not None:
                owner.close()
            # PID came from the fixture immediately before exec of our GUI.
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            ui.process.wait(timeout=5)


if __name__ == '__main__':
    main()
