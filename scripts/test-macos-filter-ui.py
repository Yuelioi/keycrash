#!/usr/bin/env python3
"""Actual Slint window acceptance for opt-in global listener isolation.

Uses only the task-owned signed filter fixture and fixed Diagnostics host.
Verifies default selection, attribution, stale candidates and in-flight cancel.
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

spec=importlib.util.spec_from_file_location('gui',Path(__file__).with_name('test-macos-owner-gui.py'))
gui=importlib.util.module_from_spec(spec);spec.loader.exec_module(gui)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--app',type=Path,required=True)
    p.add_argument('--fixture',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--trigger-fixture',action='store_true',required=True)
    args=p.parse_args();app=args.app.resolve(strict=True);fixture=args.fixture.resolve(strict=True)
    gui.require_unlocked_desktop()
    assert fixture.name=='KeyCrash Permission Test Filters Verified.app'
    out=args.output.resolve();out.mkdir(parents=True,exist_ok=True)
    snapshots=out/'snapshots';snapshots.mkdir(exist_ok=True)
    serial=0;pid=None;owner=None
    with tempfile.TemporaryDirectory(prefix='filter-ui-',dir=app.parent) as tmp:
        work=Path(tmp)
        output=out/'window.jsonl'
        process=subprocess.Popen(['open','-n','-W','--stdout',str(output),'--stderr',str(out/'window.err'),
            str(app),'--args','product-ui','0',str(snapshots)])
        deadline=time.monotonic()+10
        while not output.exists() or not output.read_text().strip():
            assert time.monotonic()<deadline,'No UI PID'
            time.sleep(.05)
        pid=json.loads(output.read_text().splitlines()[0])['pid']

        def invoke(mode,target=pid,value=None):
            nonlocal serial
            serial+=1;log=out/f'{serial}-{mode}.jsonl'
            command=['open','-n','-W','--stdout',str(log),str(app),'--args',mode,str(target)]
            if value is not None: command.append(str(value))
            subprocess.run(command,timeout=12,check=False)
            return [json.loads(x) for x in log.read_text().splitlines()]

        def press(label):
            records=invoke('ui-press',value=label)
            assert any(r.get('kind')=='pressed' and r['status']==0 for r in records),label

        def wait_text(text,timeout=15):
            deadline=time.monotonic()+timeout
            while time.monotonic()<deadline:
                records=invoke('ui-tree')
                if any(text in r.get('value','') for r in records): return records
                time.sleep(.1)
            raise AssertionError((text,records))

        def launch_owner(label):
            instance=gui.AppProcess(fixture,out,label,'tap-1-drop-pair',hold=True)
            ready=instance.record();assert ready['status']==0 and ready['ax']
            return instance,ready

        def choose(ready):
            press('检查全局监听')
            records=wait_text('选择参与检查的软件')
            label=next(r['title'] for r in records if r.get('role')=='AXCheckBox' and str(ready['pid']) in r.get('title',''))
            row=next(r for r in records if r.get('role')=='AXCheckBox' and r.get('title')==label)
            assert row['value']=='0',row
            press(label)
            time.sleep(1)

        def tap_state(ready):
            return {r['id']:r['enabled'] for r in invoke('tap-state',ready['pid'])[0]['taps']}

        try:
            wait_text('选择修饰键')
            owner,ready=launch_owner('owner')
            original=tap_state(ready);assert len(original)==2 and all(original.values())
            for label in ('添加 ⌃ Ctrl','添加 ⌥ Opt','添加 ⇧ Shift','添加 ⌘ Cmd'): press(label)
            press('开始检测');invoke('ui-key',value=79)
            wait_text('此组合可临时注册')
            choose(ready);press('开始检查')
            records=wait_text('全局监听检查完成',timeout=25)
            details='\n'.join(r.get('value','') for r in records)
            assert '改变按键传递的软件' in details and f'PID {ready["pid"]}' in details,details
            assert tap_state(ready)==original
            time.sleep(.8)
            print('PASS actual UI: opt-in, correct filter PID, both listeners restored',flush=True)

            invoke('ui-language',value='en')
            wait_text('Global listener check complete');time.sleep(1.2)
            invoke('ui-language',value='ja')
            time.sleep(1.2)
            records=invoke('ui-tree')
            assert any('PID '+str(ready['pid']) in r.get('value','') for r in records)
            invoke('ui-language',value='zh')
            wait_text('全局监听检查完成')
            print('PASS live language switching preserves the result',flush=True)

            choose(ready)
            owner.close();owner=None
            press('开始检查')
            records=wait_text('全局监听检查未完成')
            assert any('监听状态已改变' in r.get('value','') for r in records),records
            print('PASS stale selection: no mutation and no attribution',flush=True)

            owner,ready=launch_owner('cancel-owner')
            original=tap_state(ready)
            choose(ready);press('开始检查')
            deadline=time.monotonic()+10
            while time.monotonic()<deadline:
                states=tap_state(ready)
                if states and not any(states.values()): break
                time.sleep(.05)
            else: raise AssertionError('Did not observe active isolation before cancellation')
            press('取消');wait_text('选择修饰键')
            assert tap_state(ready)==original
            time.sleep(1.5);wait_text('选择修饰键')
            print('PASS cancellation after isolation began: restore before returning, no stale repaint',flush=True)

            owner.close();owner=None
            owner,ready=launch_owner('crash-owner')
            original=tap_state(ready)
            press('开始检测');invoke('ui-key',value=79)
            wait_text('此组合可临时注册')
            choose(ready);press('开始检查')
            deadline=time.monotonic()+10
            while time.monotonic()<deadline:
                states=tap_state(ready)
                if states and not any(states.values()): break
                time.sleep(.05)
            else: raise AssertionError('Did not observe active isolation before controller crash')
            os.kill(pid,signal.SIGKILL);pid=None
            deadline=time.monotonic()+5
            while time.monotonic()<deadline:
                if tap_state(ready)==original: break
                time.sleep(.05)
            else: raise AssertionError('Guardian did not restore after actual UI process was killed')
            print('PASS actual UI SIGKILL: independent guardian restored both listeners',flush=True)
        finally:
            if owner is not None: owner.close()
            if pid is not None:
                try: os.kill(pid,signal.SIGTERM)
                except ProcessLookupError: pass
            process.wait(timeout=10)


if __name__=='__main__':main()
