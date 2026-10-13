#!/usr/bin/env python3
"""Test the shipping filter coordinator using separately signed global fixtures.

Requires the authorized Diagnostics host, compiled filter-production backend,
owner-query, owner-mechanisms and owner-global-chain beside it. Creates only a
fresh KeyCrash Permission Test Filter.app and grants its own Settings switch.
All test input is Ctrl+Opt+Shift+Cmd+F18. No user application is isolated.
"""
import argparse
import importlib.util
import json
import os
import plistlib
import shutil
import subprocess
import sys
import tempfile
import time
from contextlib import ExitStack
from pathlib import Path

spec=importlib.util.spec_from_file_location('gui',Path(__file__).with_name('test-macos-owner-gui.py'))
gui=importlib.util.module_from_spec(spec);spec.loader.exec_module(gui)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--app',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--trigger-fixture',action='store_true',required=True)
    p.add_argument('--keep-fixture',action='store_true',help='Keep the immutable test identity/grant through backend and UI work, including failed tests; revoke at task cleanup')
    p.add_argument('--reuse-fixture',action='store_true',help='Reuse only this exact task-owned signed fixture after a failed test')
    p.add_argument('--authorized-fixture',action='store_true',help='Require the retained test grant; do not change Settings')
    p.add_argument('--case',action='append',help='Run named cases after diagnosing a prior failure')
    p.add_argument('--replacement-host',action='store_true',help='Use the corrected replacement helper under Diagnostics identity; keep the independent fixture signature unchanged')
    args=p.parse_args();app=args.app.resolve(strict=True)
    gui.require_unlocked_desktop()
    out=args.output.resolve();out.mkdir(parents=True,exist_ok=True)
    fixture=app.parent/'KeyCrash Permission Test Filters Verified.app'
    identifier='com.yueli.keycrash.fixture-filters-verified-20261013d'
    if args.reuse_fixture:
        with (fixture/'Contents/Info.plist').open('rb') as f: assert plistlib.load(f)['CFBundleIdentifier']==identifier
        subprocess.run(['codesign','--verify','--strict',str(fixture)],check=True)
    else:
        assert not fixture.exists(),fixture
        (fixture/'Contents/MacOS').mkdir(parents=True)
        shutil.copy2(app.parent/'owner-mechanisms',fixture/'Contents/MacOS/owner-mechanisms')
        with (fixture/'Contents/Info.plist').open('wb') as f:
            plistlib.dump(dict(CFBundleIdentifier=identifier,CFBundleExecutable='owner-mechanisms',
                CFBundleName=fixture.stem,CFBundleDisplayName=fixture.stem,CFBundlePackageType='APPL',
                CFBundleVersion='1',LSUIElement=True),f)
        subprocess.run(['codesign','--force','--sign','-',str(fixture)],check=True)
    results=[];permission=None;complete=False

    def invoke(label,*arguments,timeout=35):
        log=out/(label+'.jsonl');log.write_text('')
        subprocess.run(['open','-n','-W','--stdout',str(log),'--stderr',str(out/(label+'.err')),
            str(app),'--args',*map(str,arguments)],timeout=timeout,check=False)
        rows=[json.loads(x) for x in log.read_text().splitlines()]
        assert rows,(label,(out/(label+'.err')).read_text())
        return rows

    desktop=invoke('desktop-before','desktop-state')[0]
    try:
        permission=gui.AppProcess(fixture,out,'permission','request-permission',hold=True)
        initial=permission.record()
        if args.authorized_fixture:
            assert initial['ax'],initial
        else:
            assert not initial['ax'],initial
            # Reopen the pane before operating the current test-app switch.
            subprocess.run(['open','x-apple.systempreferences:com.apple.preference.general'],check=True)
            time.sleep(.8)
            subprocess.run(['open','x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility'],check=True)
            time.sleep(1)
            deadline=time.monotonic()+12
            while time.monotonic()<deadline:
                settings=subprocess.check_output(['pgrep','-x','System Settings'],text=True).strip()
                rows=invoke('settings','settings-ui',settings,fixture.stem,timeout=12)
                switches=[r for r in rows if r.get('kind')=='permission-switch']
                if switches:
                    assert any(r['before']=='0' and r['status']==0 for r in switches),switches
                    (out/'permission-switch.json').write_text(json.dumps(switches)+'\n')
                    confirmation=invoke('settings-confirmation','settings-ui',settings,timeout=12)
                    if any(r.get('role')=='AXSheet' for r in confirmation):
                        raise RuntimeError('System Settings requires password confirmation on the Mac; the switch alone does not grant permission')
                    break
                time.sleep(.25)
            else: raise AssertionError('Test app permission row missing')
            assert permission.record(timeout=12)['kind']=='permission-granted'
            (out/'settings.jsonl').unlink()
        permission.close();permission=None
        cases=[('hid',['tap-0-drop']),('session',['tap-1-drop']),('annotated',['tap-2-drop']),
            ('pass',['tap-1-pass']),('double-drop',['tap-1-drop','tap-1-drop']),
            ('pass-drop',['tap-1-pass','tap-1-drop']),('replace-drop',['tap-1-replace','tap-2-drop']),
            ('multi',['tap-1-drop-pair']),('disabled',['tap-1-drop-disabled']),
            ('cancel',['tap-1-drop']),('cancel-after',['tap-1-drop-pair']),('carbon-drop',['tap-1-drop']),
            ('changed',['tap-1-drop-rebuild'])]
        if args.case:
            assert set(args.case)<={label for label,_ in cases},args.case
            cases=[case for case in cases if case[0] in args.case]
        with tempfile.TemporaryDirectory(prefix='filter-product-',dir=app.parent) as tmp:
            work=Path(tmp)
            for label,modes in cases:
                gui.require_unlocked_desktop()
                with ExitStack() as cleanup:
                    sink=gui.AppProcess(app,work,label+'-sink','mechanism','foreground',hold=True)
                    cleanup.callback(sink.close);sink.record()
                    owners=[]
                    for index,mode in enumerate(modes):
                        if args.replacement_host and mode=='tap-1-replace':
                            owner=gui.AppProcess(app,work,f'{label}-{index}','replacement-fixture',hold=True)
                        else:
                            owner=gui.AppProcess(fixture,work,f'{label}-{index}',mode,hold=True)
                        cleanup.callback(owner.close);ready=owner.record();assert ready['status']==0 and ready['ax']
                        owners.append((owner,ready))
                    carbon=None
                    if label=='carbon-drop':
                        process=gui.AppProcess(app,work,'carbon','hold',79,hold=True)
                        cleanup.callback(process.close);carbon=process.record();assert carbon['status']==0
                    pids={r['pid'] for _,r in owners}
                    listing=invoke(label+'-list','filter-production','list',79,6912,'-')[0]
                    assert pids <= {c['pid'] for c in listing['candidates']},listing
                    before=invoke(label+'-before','global-chain')[0]
                    command=label if label.startswith('cancel') else 'locate'
                    result=invoke(label+'-result','filter-production',command,79,6912,','.join(map(str,sorted(pids))))[0]
                    after=invoke(label+'-after','global-chain')[0]
                    states=lambda c:{t['id']:t['enabled'] for t in c['before'] if t['pid'] in pids}
                    if label=='changed':
                        assert states(before)!=states(after) and all(states(after).values())
                        assert 'error' in result and 'filters' not in result,result
                    else:
                        assert states(before)==states(after),(label,before,after)
                    if label.startswith('cancel'):
                        assert 'error' in result and '恢复' not in result['error'],result
                    elif label!='changed':
                        expected={r['pid'] for _,r in owners if '-drop' in r['mode'] or '-replace' in r['mode']}
                        assert {o['pid'] for o in result['filters']}==expected,result
                        if carbon: assert {o['pid'] for o in result['receivers']}=={carbon['pid']},result
                    traces=[{'ready':r,'trace':[json.loads(x) for x in o.log.read_text().splitlines()]} for o,r in owners]
                    results.append(dict(case=label,result=result,owners=traces,before=before,after=after))
                    (out/'matrix.json').write_text(json.dumps(results,indent=2)+'\n')
                    print('PASS '+label,flush=True)
        complete=True
    finally:
        if permission is not None: permission.close()
        invoke('desktop-restored','desktop-restore',desktop['front_pid'],desktop['input_source'])
        if not args.keep_fixture:
            for service in ('Accessibility','PostEvent'):
                subprocess.run(['tccutil','reset',service,identifier],check=True)
        (out/'retention.json').write_text(json.dumps(dict(bundle=str(fixture),identifier=identifier,
            grant_retained=args.keep_fixture,reason='Ongoing backend/UI acceptance' if args.keep_fixture else 'Bundle cleanup pending'))+'\n')


if __name__=='__main__':main()
