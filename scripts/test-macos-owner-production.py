#!/usr/bin/env python3
"""Exercise the shipping Rust attribution backend in an authorized GUI host.

The separate Carbon owners and optional real Ctrl+Option+A shortcut can execute
their registered actions. --trigger-fixture explicitly opts into fixture input.
"""
import argparse
import importlib.util
import subprocess
import tempfile
import time
from contextlib import ExitStack
from pathlib import Path
spec = importlib.util.spec_from_file_location('gui_driver', Path(__file__).with_name('test-macos-owner-gui.py'))
gui_driver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gui_driver)
AppProcess = gui_driver.AppProcess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app', type=Path, required=True)
    parser.add_argument('--trigger-fixture', action='store_true', required=True)
    parser.add_argument('--user-shortcut', action='store_true')
    args = parser.parse_args()
    app = args.app.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='product-', dir=app.parent) as temp, ExitStack() as cleanup:
        work = Path(temp)
        serial = 0

        def probe(mode, key, modifiers=6912):
            nonlocal serial
            serial += 1
            output = work / f'probe-{serial}.txt'
            errors = work / f'probe-{serial}.err'
            process = subprocess.Popen(['open', '-n', '-W', '--stdout', str(output), '--stderr', str(errors), str(app),
                '--args', 'production', mode, str(key), str(modifiers)])
            process.wait(timeout=10)
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                if output.exists() and output.read_text().strip():
                    text = output.read_text().strip()
                    print(f'{mode} {key} {modifiers}: {text}', flush=True)
                    return text
                time.sleep(0.02)
            raise RuntimeError('Product backend returned no result: ' + (errors.read_text() if errors.exists() else ''))

        def pids(result):
            return {int(line.split('\t')[1]) for line in result.splitlines() if line.startswith('FOUND\t')}

        assert probe('cancel', 79) == 'Cancelled'
        owners = {}
        for key in (79, 80, 0, 49, 36, 123, 75, 82):
            owner = AppProcess(app, work, f'owner-{key}', 'hold-exclusive' if key == 79 else 'hold', key, hold=True)
            cleanup.callback(owner.close)
            ready = owner.record()
            assert ready.get('status') == 0, ready
            owner.record()
            owners[key] = (owner, ready['pid'])
        for key, (owner, pid) in owners.items():
            checked = probe('check', key)
            if key == 79:
                assert 'Blocked' in checked
            else:
                assert 'Available' in checked or 'Blocked' in checked
            for _ in range(2 if key in (79, 80) else 1):
                result = probe('locate', key)
                assert pids(result) == {pid}, result
                assert 'owner-query' in result, result
                assert owner.record().get('pid') == pid
        assert probe('locate', 90) == 'NotFound'
        for owner, _ in owners.values():
            owner.close()
        for key in owners:
            assert 'Available' in probe('check', key)
            assert probe('locate', key) == 'NotFound'
        if args.user_shortcut:
            print('USER Ctrl+Option+A:', probe('check', 0, 6144), flush=True)
            print('USER Ctrl+Option+A:', probe('locate', 0, 6144), flush=True)
    print('PASS: product receiver/path, repeat, negative, release, pre-cancel')


if __name__ == '__main__':
    main()
