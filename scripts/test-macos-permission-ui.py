#!/usr/bin/env python3
"""First-grant acceptance: actual app, same PID, QQ Shift+Command+F.

Requires a fresh ad-hoc signed KeyCrash Permission Test.app and an already
authorized diagnostics app. Operates only the test app's Accessibility switch
through normal System Settings UI; never accesses TCC storage.
"""
import argparse
import json
import os
import signal
import subprocess
import tempfile
import time
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app', type=Path, required=True)
    parser.add_argument('--diagnostics', type=Path, required=True)
    args = parser.parse_args()
    app = args.app.resolve(strict=True)
    diagnostics = args.diagnostics.resolve(strict=True)
    assert app.name.startswith('KeyCrash Permission Test') and app.suffix == '.app'
    with tempfile.TemporaryDirectory(prefix='permission-ui-', dir=app.parent) as temp:
        work = Path(temp)
        sequence = 0

        def invoke(mode, target, value=None):
            nonlocal sequence
            sequence += 1
            output = work / f'{sequence}.jsonl'
            command = ['open', '-n', '-W', '--stdout', str(output), str(diagnostics),
                       '--args', mode, str(target)]
            if value is not None:
                command.append(value)
            subprocess.run(command, timeout=10, check=True)
            return [json.loads(x) for x in output.read_text().splitlines()]

        process = subprocess.Popen(['open', '-n', '-W', '--stderr', str(work / 'app-stderr.log'), str(app)])
        pid = None
        try:
            time.sleep(1)
            pid = invoke('product-pid', app)[0]['pid']

            def press(label):
                rs = invoke('ui-press', pid, label)
                assert any(r.get('kind') == 'pressed' and r['status'] == 0 for r in rs), label

            def wait_text(text):
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    rs = invoke('ui-tree', pid)
                    if any(r.get('value') == text for r in rs):
                        print(f'PID {pid}: {text}', flush=True)
                        return rs
                    time.sleep(.2)
                raise AssertionError(f'Missing {text}: {rs}')

            wait_text('选择修饰键')
            press('添加 ⇧ Shift')
            press('添加 ⌘ Cmd')
            press('开始检测')
            invoke('ui-key', pid, '3')
            press('定位占用软件')
            wait_text('需要辅助功能权限')
            settings_pid = subprocess.check_output(['pgrep', '-x', 'System Settings'], text=True).strip()
            deadline = time.monotonic() + 10
            switches = []
            while not switches and time.monotonic() < deadline:
                rs = invoke('settings-ui', settings_pid, app.stem)
                switches = [r for r in rs if r.get('kind') == 'permission-switch']
                if not switches:
                    time.sleep(.5)  # System Settings inserts new rows asynchronously.
            print(switches, flush=True)
            assert any(r['status'] == 0 and r['before'] == '0' for r in switches), switches
            time.sleep(1)
            press('定位占用软件')
            rs = wait_text('QQ占用了它')
            assert any('/QQ.app/' in r.get('value', '') for r in rs)
            assert invoke('product-pid', app)[0]['pid'] == pid
            print('PASS: first grant then QQ attribution, original process, no restart', flush=True)
        finally:
            if (work / 'app-stderr.log').exists():
                print((work / 'app-stderr.log').read_text(), flush=True)
            if pid is not None:
                try:
                    os.kill(pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
            process.wait(timeout=5)


if __name__ == '__main__':
    main()
