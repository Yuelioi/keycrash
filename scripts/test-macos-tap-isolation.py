#!/usr/bin/env python3
"""Causal isolation feasibility: only task-owned global filter processes.

No application under normal user use is touched. The observer gets no fixture
PID. The control helper checks the exact fixture executable before each change.
"""
import argparse
import importlib.util
import json
import subprocess
import tempfile
from contextlib import ExitStack
from pathlib import Path

spec=importlib.util.spec_from_file_location('gui',Path(__file__).with_name('test-macos-owner-gui.py'))
gui=importlib.util.module_from_spec(spec);spec.loader.exec_module(gui)
FULL={(i,p) for i in range(3) for p in ('head','tail')}


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--app',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--trigger-fixture',action='store_true',required=True)
    args=p.parse_args();app=args.app.resolve(strict=True);results=[]
    cases=[['tap-1-pass','tap-1-drop'],['tap-1-drop','tap-1-pass'],
           ['tap-1-drop','tap-1-drop'],['tap-1-pass','tap-1-pass'],
           ['tap-0-drop','tap-2-drop'],['tap-1-replace','tap-2-drop']]
    with tempfile.TemporaryDirectory(prefix='isolation-',dir=app.parent) as tmp:
        work=Path(tmp);serial=0
        for case,modes in enumerate(cases):
            with ExitStack() as cleanup:
                def launch(label,mode,value=None):
                    nonlocal serial
                    serial+=1
                    proc=gui.AppProcess(app,work,f'{case}-{serial}-{label}',mode,value,hold=True)
                    cleanup.callback(lambda proc=proc:proc.close() if proc.fd is not None else None)
                    return proc,proc.record()
                sink,_=launch('sink','mechanism','foreground')
                owners=[launch('owner','mechanism',mode) for mode in modes]
                pids={r['pid'] for _,r in owners}
                assert all(r['status']==0 for _,r in owners)
                def observe(phase,enabled):
                    nonlocal serial
                    serial+=1;out=work/f'chain-{serial}.json'
                    subprocess.run(['open','-n','-W','--stdout',str(out),str(app),'--args','global-chain'],timeout=10,check=True)
                    c=json.loads(out.read_text());assert c['ready'] and c['lists_ok'] and not c['interrupted']
                    for snapshot in ('before','after'):
                        found={t['pid']:t['enabled'] for t in c[snapshot] if t['pid'] in pids}
                        assert found=={pid:pid in enabled for pid in pids},(phase,found,enabled)
                    return c
                def disable(pid):
                    proc,ready=launch('toggle','tap-toggle-fixture',pid)
                    assert ready['disable_code']==0 and ready['present'] and not ready['enabled'],ready
                    return proc
                def restore(proc):
                    proc.close()
                    last=json.loads(proc.log.read_text().splitlines()[-1])
                    assert last['kind']=='restored' and last['present'] and last['enabled'],last
                    return last
                baseline=observe('baseline',pids)
                controls={pid:disable(pid) for pid in sorted(pids)}
                control=observe('all-disabled',set())
                assert {(e['location'],e['position']) for e in control['events'] if e['type']==10}==FULL
                isolated=[]
                for pid in sorted(pids):
                    restore(controls[pid])
                    c=observe('one-enabled',{pid})
                    points={(e['location'],e['position']) for e in c['events'] if e['type']==10}
                    isolated.append({'pid':pid,'changed_marked_path':points!=FULL,'chain':c})
                    controls[pid]=disable(pid)
                restored=[restore(proc) for proc in controls.values()]
                after=observe('all-restored',pids)
                result={'modes':modes,'owners':[{'ready':r,'trace':[json.loads(x) for x in o.log.read_text().splitlines()]} for o,r in owners],
                        'baseline':baseline,'all_disabled':control,'isolated':isolated,'restored':restored,'after':after,
                        'sink_trace':[json.loads(x) for x in sink.log.read_text().splitlines()]}
                results.append(result);args.output.write_text(json.dumps(results,indent=2)+'\n')
                found={x['pid'] for x in isolated if x['changed_marked_path']}
                expected={r['pid'] for _,r in owners if r['mode'].endswith(('-drop','-replace'))}
                assert found==expected,(found,expected)
                print(json.dumps({'modes':modes,'found':sorted(found),'expected':sorted(expected),'restored':restored}),flush=True)


if __name__=='__main__':main()
