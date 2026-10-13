#!/usr/bin/env python3
"""Observe selected real desktop shortcuts, then cancel transient UI.

Requires explicit --trigger-real-shortcuts. Does not trigger capture-to-file,
clipboard copy/paste, lock, quit, logout or shutdown. The original input source
and foreground app are restored. QQ combinations follow the user's settings.
"""
import argparse
import importlib.util
import json
import subprocess
import tempfile
import time
from pathlib import Path

spec = importlib.util.spec_from_file_location('gui', Path(__file__).with_name('test-macos-owner-gui.py'))
gui = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gui)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--app', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--trigger-real-shortcuts', action='store_true', required=True)
    args = p.parse_args()
    app = args.app.resolve(strict=True)
    cases = [('QQ Shift+Command+F', 3, 768), ('QQ Control+Command+A', 0, 4352),
             ('Command+Space', 49, 256), ('Control+Space', 49, 4096),
             ('Shift+Command+4', 21, 768), ('Shift+Command+5', 23, 768),
             ('Control+Up', 126, 4096), ('Control+Down', 125, 4096),
             ('Control+Command+Space', 49, 4352), ('Command+Tab', 48, 256)]
    with tempfile.TemporaryDirectory(prefix='common-', dir=app.parent) as tmp:
        work = Path(tmp)
        serial = 0

        def invoke(*arguments):
            nonlocal serial
            serial += 1
            out = work / f'{serial}.txt'
            subprocess.run(['open', '-n', '-W', '--stdout', str(out), str(app), '--args', *map(str, arguments)],
                           timeout=10, check=True)
            return out.read_text().strip() if out.exists() else ''

        original = json.loads(invoke('desktop-state'))
        assert not original['secure_input'], 'Secure Input active; do not alter it'
        results = []
        try:
            for name, key, flags in cases:
                sink = gui.AppProcess(app, work, f'sink-{serial}', 'mechanism', 'foreground', hold=True)
                try:
                    ready = sink.record()
                    assert ready['status'] == 0
                    before = json.loads(invoke('desktop-state'))
                    checked = invoke('production', 'check', key, flags)
                    located = invoke('production', 'locate', key, flags)
                    time.sleep(.3)
                    after = json.loads(invoke('desktop-state'))
                    result = {'case': name, 'key': key, 'carbon_modifiers': flags,
                              'before': before, 'check': checked, 'locate': located, 'after': after}
                    results.append(result)
                    args.output.write_text(json.dumps(results, indent=2, ensure_ascii=False) + '\n')
                    print(json.dumps(result, ensure_ascii=False), flush=True)
                finally:
                    invoke('mechanism', 'escape')
                    sink.close()
                    restored = json.loads(invoke('desktop-restore', original['front_pid'], original['input_source']))
                    assert restored['input_status'] == 0, restored
        finally:
            invoke('desktop-restore', original['front_pid'], original['input_source'])
        print('RESTORED:', invoke('desktop-state'), flush=True)


if __name__ == '__main__':
    main()
