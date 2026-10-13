#!/usr/bin/env python3
"""Explore global interception evidence, with independent pass/drop/rewrite controls.

Uses only Ctrl+Opt+Shift+Cmd+F18. Does not disable or suspend any other app/tap.
No fixture PID is supplied to the observer; receiver logs are validation only.
"""
import argparse
import importlib.util
import json
import subprocess
import tempfile
import time
from contextlib import ExitStack
from pathlib import Path

spec = importlib.util.spec_from_file_location('gui', Path(__file__).with_name('test-macos-owner-gui.py'))
gui = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gui)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--app', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--trigger-fixture', action='store_true', required=True)
    p.add_argument('--case', action='append', help='Select named experiment cases')
    args = p.parse_args()
    app = args.app.resolve(strict=True)
    cases = [('baseline', []), ('carbon', ['carbon-shared']), ('global-monitor', ['nsevent-global'])]
    cases += [(f'tap-{loc}-{mode}', [f'tap-{loc}-{mode}']) for loc in (0, 1, 2) for mode in ('pass', 'drop')]
    cases += [('rewrite', ['tap-1-rewrite']), ('carbon-drop', ['carbon-shared', 'tap-1-drop'])]
    cases += [(m, ['tap-1-'+m]) for m in ('replace', 'repost', 'repost-marked')]
    cases += [(f'{a}-{b}', [f'tap-1-{a}', f'tap-1-{b}']) for a, b in
              [('pass', 'pass'), ('pass', 'drop'), ('drop', 'pass'), ('drop', 'drop')]]
    if args.case:
        cases = [(name, modes) for name, modes in cases if name in args.case]
        assert len(cases) == len(set(args.case)), 'Unknown case'
    results = []
    with tempfile.TemporaryDirectory(prefix='chain-', dir=app.parent) as tmp:
        work = Path(tmp)
        for name, modes in cases:
            with ExitStack() as cleanup:
                sink = gui.AppProcess(app, work, name+'-sink', 'mechanism', 'foreground', hold=True)
                cleanup.callback(sink.close)
                sink_ready = sink.record()
                assert sink_ready['status'] == 0
                owners = []
                for i, mode in enumerate(modes):
                    owner = gui.AppProcess(app, work, f'{name}-{i}', 'mechanism', mode, hold=True)
                    cleanup.callback(owner.close)
                    ready = owner.record()
                    assert ready['status'] == 0, ready
                    owners.append((owner, ready))
                out = work / (name+'-chain.json')
                subprocess.run(['open', '-n', '-W', '--stdout', str(out), str(app), '--args', 'global-chain'],
                               timeout=10, check=True)
                chain = json.loads(out.read_text())
                assert chain['ready'] and chain['lists_ok'] and not chain['interrupted'], chain
                time.sleep(.1)
                def received(process):
                    return [r for line in process.log.read_text().splitlines()
                            if (r := json.loads(line))['kind'] == 'received']
                result = {'case': name, 'owners': [{'ready': r, 'received': received(o),
                          'trace': [json.loads(line) for line in o.log.read_text().splitlines()]}
                          for o, r in owners],
                          'sink_pid': sink_ready['pid'], 'sink_events': received(sink), 'chain': chain}
                results.append(result)
                args.output.write_text(json.dumps(results, indent=2)+'\n')
                print(json.dumps({'case': name, 'received': [len(o['received']) for o in result['owners']],
                                  'sink': len(result['sink_events']), 'events': chain['events']}), flush=True)
    print('Completed global-chain experiment; no candidate is labelled as confirmed owner.')


if __name__ == '__main__':
    main()
