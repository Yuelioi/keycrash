#!/usr/bin/env python3
"""Measure coverage by mechanism; a valid fixture is not a product coverage pass.

Requires the authorized Diagnostics host, owner-query, owner-mechanisms and
the actual owner-production backend beside it. Sends all-four-modifiers+F18.
Opt-in local controls also send Command+C/V/F/comma to harmless fixture menus.
Results include receiver acknowledgements, product output and release.
Default scope is global mechanisms; local/menu controls require an explicit flag.
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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--trigger-fixture', action='store_true', required=True)
    parser.add_argument('--case', action='append', help='Run only named cases')
    parser.add_argument('--include-local-controls', action='store_true',
                        help='Include historical app-local/menu negative controls')
    args = parser.parse_args()
    app = args.app.resolve(strict=True)
    results = []
    cases = [(x, [x]) for x in ('carbon-shared', 'carbon-exclusive')]
    cases += [('carbon-two-shared', ['carbon-shared', 'carbon-shared']),
              ('carbon-two-exclusive', ['carbon-exclusive', 'carbon-exclusive']),
              ('carbon-shared-then-exclusive', ['carbon-shared', 'carbon-exclusive']),
              ('carbon-exclusive-then-shared', ['carbon-exclusive', 'carbon-shared'])]
    cases += [(f'tap-{loc}-{behavior}', [f'tap-{loc}-{behavior}'])
              for loc in (0, 1, 2) for behavior in ('listen', 'pass', 'drop')]
    cases += [('nsevent-global', ['nsevent-global'])]
    cases += [('carbon-with-filter', ['carbon-shared', 'tap-1-drop'])]
    if args.include_local_controls:
        cases += [(x, [x]) for x in ('nsevent-local-pass', 'nsevent-local-drop', 'menu')]
        cases += [('nsevent-global-self', ['nsevent-global-self']),
                  ('nsevent-local-pass-background', ['nsevent-local-pass']),
                  ('nsevent-local-drop-background', ['nsevent-local-drop']),
                  ('menu-background', ['menu'])]
        cases += [(x, [x]) for x in ('menu-c', 'menu-v', 'menu-f', 'menu-comma')]
    if args.case:
        cases = [(name, modes) for name, modes in cases if name in args.case]
        assert len(cases) == len(set(args.case)), 'Unknown case name'
    with tempfile.TemporaryDirectory(prefix='mechanisms-', dir=app.parent) as tmp:
        work = Path(tmp)
        sequence = 0

        key, modifiers = 79, 6912
        def probe(mode):
            nonlocal sequence
            sequence += 1
            out = work / f'probe-{sequence}.txt'
            subprocess.run(['open', '-n', '-W', '--stdout', str(out), str(app), '--args',
                            'production', mode, str(key), str(modifiers)], timeout=10, check=True)
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                if out.exists() and out.read_text().strip():
                    return out.read_text().strip()
                time.sleep(.02)
            raise RuntimeError('Backend produced no result')

        for name, modes in cases:
            key = {'menu-c': 8, 'menu-v': 9, 'menu-f': 3, 'menu-comma': 43}.get(name, 79)
            modifiers = 256 if name.startswith('menu-') and name != 'menu-background' else 6912
            with ExitStack() as cleanup:
                sink = gui.AppProcess(app, work, name + '-sink', 'mechanism', 'foreground', hold=True)
                cleanup.callback(sink.close)
                sink_ready = sink.record()
                owners = []
                for number, mode in enumerate(modes):
                    owner = gui.AppProcess(app, work, f'{name}-{number}', 'mechanism', mode, hold=True)
                    cleanup.callback(lambda owner=owner: owner.close() if owner.fd is not None else None)
                    ready = owner.record()
                    owners.append((owner, ready))
                if name.endswith('-background'):
                    subprocess.run(['open', '-n', '-W', str(app), '--args', 'mechanism-activate',
                                    str(sink_ready['pid'])], timeout=10, check=True)
                checked = probe('check')
                located = probe('locate')
                time.sleep(.1)
                records = []
                for owner, ready in owners:
                    seen = [json.loads(x) for x in owner.log.read_text().splitlines()]
                    received = [x for x in seen if x['kind'] == 'received']
                    records.append({'ready': ready, 'received': received})
                sink_events = [json.loads(x) for x in sink.log.read_text().splitlines()
                               if json.loads(x)['kind'] == 'received']
                result = {'case': name, 'check': checked, 'locate': located, 'owners': records,
                          'sink_pid': sink_ready['pid'], 'sink_events': sink_events}
                if len(owners) == 2 and name != 'carbon-two-exclusive':
                    # Remove only the exclusive registrant or the swallowing tap.
                    removed = 0 if name == 'carbon-exclusive-then-shared' else 1
                    owners[removed][0].close()
                    result['after_one_removed'] = probe('locate')
                    result['remaining_events'] = [json.loads(x) for x in owners[1 - removed][0].log.read_text().splitlines()
                                                  if json.loads(x)['kind'] == 'received']
            result['after_release_check'] = probe('check')
            result['after_release_locate'] = probe('locate')
            results.append(result)
            args.output.write_text(json.dumps(results, indent=2, ensure_ascii=False) + '\n')
            print(json.dumps({'case': name, 'registration': [r['ready']['status'] for r in records],
                              'received': [len(r['received']) for r in records], 'locate': located,
                              'after_release': result['after_release_locate']}, ensure_ascii=False), flush=True)
    # Coverage gaps are findings, not a reason to erase or rewrite the result.
    assert all(x['after_release_locate'] == 'NotFound' for x in results), 'Receiver remained after teardown'
    assert all(r['ready']['status'] == 0 or x['case'] == 'carbon-two-exclusive'
               for x in results for r in x['owners']), 'Fixture setup failed; inspect evidence'
    print('Completed mechanism matrix; inspect received events versus located PIDs for coverage.')


if __name__ == '__main__':
    main()
