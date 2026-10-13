#!/usr/bin/env python3
"""Verify controlled isolation across independent app/TCC identities.

Creates only the exact named test bundle beside the fixed diagnostics host.
Grants its own Accessibility switch through the normal Settings UI, runs the
fixture matrix, then revokes the temporary grant. Never touches TCC storage.
"""
import argparse
import importlib.util
import json
import os
import plistlib
import shutil
import subprocess
import sys
import time
from pathlib import Path

spec=importlib.util.spec_from_file_location('gui',Path(__file__).with_name('test-macos-owner-gui.py'))
gui=importlib.util.module_from_spec(spec);spec.loader.exec_module(gui)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--app',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--trigger-fixture',action='store_true',required=True)
    args=p.parse_args();diagnostics=args.app.resolve(strict=True)
    output=args.output.resolve();output.mkdir(parents=True,exist_ok=True)
    fixture=diagnostics.parent/'KeyCrash Permission Test Filter.app'
    assert not fixture.exists(),f'Refusing to overwrite {fixture}'
    identifier='com.yueli.keycrash.fixture-filter.guardian-20261013'
    (fixture/'Contents/MacOS').mkdir(parents=True)
    shutil.copy2(diagnostics.parent/'owner-mechanisms',fixture/'Contents/MacOS/owner-mechanisms')
    with (fixture/'Contents/Info.plist').open('wb') as f:
        plistlib.dump(dict(CFBundleIdentifier=identifier,CFBundleExecutable='owner-mechanisms',
            CFBundleName=fixture.stem,CFBundleDisplayName=fixture.stem,CFBundlePackageType='APPL',
            CFBundleVersion='1',LSUIElement=True),f)
    subprocess.run(['codesign','--force','--sign','-',str(fixture)],check=True)
    subprocess.run(['codesign','--verify','--strict',str(fixture)],check=True)
    result={'bundle':str(fixture),'identifier':identifier,'permission':[],'tests':[]}
    process=None

    def invoke(mode,value=None,extra=None):
        log=output/'settings-last.jsonl'
        log.write_text('')
        command=['open','-n','-W','--stdout',str(log),str(diagnostics),'--args',mode]
        if value is not None: command.append(str(value))
        if extra is not None: command.append(str(extra))
        subprocess.run(command,timeout=12,check=True)
        return [json.loads(x) for x in log.read_text().splitlines()]

    desktop=invoke('desktop-state')[0]
    try:
        process=gui.AppProcess(fixture,output,'permission','request-permission',hold=True)
        ready=process.record();result['first_permission']=ready
        assert not ready['ax'],'Expected a fresh independent app identity'
        subprocess.run(['open','x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility'],check=True)
        deadline=time.monotonic()+12
        while time.monotonic()<deadline:
            pid=subprocess.check_output(['pgrep','-x','System Settings'],text=True).strip()
            records=invoke('settings-ui',pid,fixture.stem)
            switches=[r for r in records if r.get('kind')=='permission-switch']
            if switches:
                result['permission']=switches
                assert any(r['status']==0 and r['before']=='0' for r in switches),switches
                break
            time.sleep(.25)
        else: raise AssertionError('No exact test-app Accessibility row found')
        process.close();process=None
        for label,options in [('drop',['--case','tap-1-drop-pair']),('pass',['--case','tap-1-pass-pair']),
                              ('kill',['--case','tap-1-drop-pair','--kill-control'])]:
            command=[sys.executable,str(Path(__file__).with_name('test-macos-tap-toggle.py')),
                '--app',str(diagnostics),'--fixture-app',str(fixture),'--output',str(output/(label+'.json')),
                '--trigger-fixture',*options]
            with (output/(label+'.log')).open('w') as log:
                subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,timeout=65,check=True,
                    env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1'})
            result['tests'].append(label);print('PASS independent '+label,flush=True)
    finally:
        if process is not None: process.close()
        result['revoked']={}
        for service in ('Accessibility','PostEvent'):
            completed=subprocess.run(['tccutil','reset',service,identifier],capture_output=True,text=True)
            result['revoked'][service]={'code':completed.returncode,'output':completed.stdout.strip()}
        result['desktop_before']=desktop
        result['desktop_restore']=invoke('desktop-restore',desktop['front_pid'],desktop['input_source'])
        # Keep the evidence but avoid retaining an unrelated Settings tree.
        (output/'settings-last.jsonl').unlink(missing_ok=True)
        (output/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    assert all(v['code']==0 for v in result['revoked'].values()),result


if __name__=='__main__':main()
