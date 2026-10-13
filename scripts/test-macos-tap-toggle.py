#!/usr/bin/env python3
"""Test private tap-ID control only against this task's own independent fixtures.

No general application or system tap is eligible. All sent input is the bounded
Ctrl+Opt+Shift+Cmd+F18 fixture combination. Restore is checked before teardown.
"""
import argparse
import importlib.util
import json
import os
import signal
import subprocess
import tempfile
import time
from contextlib import ExitStack
from pathlib import Path

spec=importlib.util.spec_from_file_location('gui',Path(__file__).with_name('test-macos-owner-gui.py'))
gui=importlib.util.module_from_spec(spec)
spec.loader.exec_module(gui)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--app',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--fixture-app',type=Path,help='Independent task-owned KeyCrash Permission Test Filter.app')
    p.add_argument('--trigger-fixture',action='store_true',required=True)
    p.add_argument('--case',action='append',choices=['tap-0-drop','tap-1-drop','tap-2-drop','tap-1-pass',
        'tap-1-drop-pair','tap-1-pass-pair','tap-1-drop-disabled'])
    stopping=p.add_mutually_exclusive_group()
    stopping.add_argument('--wait-for-timeout',action='store_true',help='Verify the bounded normal-process restore')
    stopping.add_argument('--terminate-control',action='store_true',help='SIGTERM only the task-owned controller and verify restore')
    stopping.add_argument('--kill-control',action='store_true',help='SIGKILL the controller; require the independent guardian to restore')
    stopping.add_argument('--stop-control',action='store_true',help='SIGSTOP the controller; require the independent lease deadline to restore')
    stopping.add_argument('--exit-target',action='store_true',help='Exit the disabled fixture and verify no stale restore is attempted')
    args=p.parse_args();app=args.app.resolve(strict=True)
    results=[]
    with tempfile.TemporaryDirectory(prefix='tap-toggle-',dir=app.parent) as tmp:
        work=Path(tmp)
        for mode in args.case or ['tap-0-drop','tap-1-drop','tap-2-drop','tap-1-pass']:
            with ExitStack() as cleanup:
                sink=gui.AppProcess(app,work,mode+'-sink','mechanism','foreground',hold=True)
                cleanup.callback(sink.close);sink.record()
                if args.fixture_app:
                    fixture=args.fixture_app.resolve(strict=True)
                    assert fixture.name=='KeyCrash Permission Test Filter.app'
                    owner=gui.AppProcess(fixture,work,mode+'-owner',mode,hold=True)
                else:
                    owner=gui.AppProcess(app,work,mode+'-owner','mechanism',mode,hold=True)
                cleanup.callback(lambda owner=owner: owner.close() if owner.fd is not None else None)
                ready=owner.record();assert ready['status']==0
                def chain(phase):
                    out=work/(mode+'-'+phase+'.json')
                    subprocess.run(['open','-n','-W','--stdout',str(out),str(app),'--args','global-chain'],timeout=10,check=True)
                    value=json.loads(out.read_text());assert value['ready'] and not value['interrupted']
                    return value
                before=chain('before')
                toggle=gui.AppProcess(app,work,mode+'-toggle','tap-toggle-fixture',ready['pid'],hold=True)
                cleanup.callback(lambda toggle=toggle: toggle.close() if toggle.fd is not None else None)
                disabled=toggle.record()
                assert disabled['guardian_pid']!=disabled['pid'],disabled
                assert disabled['disable_code']==0 and disabled['present'] and not disabled['enabled'],disabled
                during=chain('disabled') if disabled['present'] and not disabled['enabled'] else None
                started=time.monotonic()
                if args.terminate_control:
                    os.kill(disabled['pid'],signal.SIGTERM)
                if args.kill_control:
                    os.kill(disabled['pid'],signal.SIGKILL)
                if args.stop_control:
                    os.kill(disabled['pid'],signal.SIGSTOP)
                    def resume():
                        try: os.kill(disabled['pid'],signal.SIGCONT)
                        except ProcessLookupError: pass
                    cleanup.callback(resume)
                if args.exit_target:
                    owner.close()
                asynchronous=any((args.wait_for_timeout,args.terminate_control,args.kill_control,args.stop_control,args.exit_target))
                if asynchronous:
                    # open -W may exit with the killed controller before its orphaned
                    # guardian writes the final record. Observe the log independently.
                    deadline=time.monotonic()+12
                    while True:
                        records=[json.loads(x) for x in toggle.log.read_text().splitlines()]
                        if records and records[-1]['kind']=='restored': break
                        assert time.monotonic()<deadline,records
                        time.sleep(.02)
                    if args.stop_control:
                        os.kill(disabled['pid'],signal.SIGCONT)
                toggle.close()
                restored=[json.loads(line) for line in toggle.log.read_text().splitlines()][-1]
                elapsed=time.monotonic()-started
                assert restored['kind']=='restored',restored
                if args.exit_target:
                    assert not restored['restore_attempted'] and not restored['present'],restored
                else:
                    assert restored['restore_code']==0 and restored['present'] and restored['enabled'],restored
                if args.kill_control or args.terminate_control:
                    assert restored['reason']=='controller-disconnected' and elapsed<3,(restored,elapsed)
                if args.stop_control or args.wait_for_timeout:
                    assert restored['reason']=='deadline',restored
                after=chain('restored')
                points=lambda c:{(e['location'],e['position']) for e in c['events'] if e['type']==10}
                full={(i,p) for i in range(3) for p in ('head','tail')}
                assert points(during)==full
                assert (points(before)==full)==('-pass' in mode)
                assert (points(after)==full)==('-pass' in mode or args.exit_target)
                states=lambda c,phase:{t['id']:t['enabled'] for t in c[phase] if t['pid']==ready['pid']}
                original=states(before,'before')
                assert set(disabled['tap_ids'])=={tap for tap,value in original.items() if value},(disabled,original)
                for phase in ('before','after'):
                    assert states(during,phase)=={tap:False for tap in original}
                    assert states(after,phase)==({} if args.exit_target else original)
                result={'case':mode,'owner':ready,'disabled':disabled,'restored':restored,
                        'recovery_seconds':elapsed,
                        'before':before,'during':during,'after':after,
                        'owner_trace':[json.loads(line) for line in owner.log.read_text().splitlines()],
                        'sink_trace':[json.loads(line) for line in sink.log.read_text().splitlines()]}
                results.append(result)
                args.output.write_text(json.dumps(results,indent=2)+'\n')
                print(json.dumps({'case':mode,'disabled':disabled,'restored':restored,'during_available':during is not None}),flush=True)


if __name__=='__main__':main()
