#!/usr/bin/env python3
"""Verify guardian restore survives both controller SIGKILL and stdout EPIPE."""
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
gui=importlib.util.module_from_spec(spec);spec.loader.exec_module(gui)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--app',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--trigger-fixture',action='store_true',required=True)
    p.add_argument('--fill-output',action='store_true',help='Keep an unread, full output pipe instead of closing it')
    args=p.parse_args();app=args.app.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='guardian-pipe-',dir=app.parent) as tmp, ExitStack() as cleanup:
        work=Path(tmp)
        sink=gui.AppProcess(app,work,'sink','mechanism','foreground',hold=True)
        cleanup.callback(sink.close);sink.record()
        owner=gui.AppProcess(app,work,'owner','mechanism','tap-1-drop-pair',hold=True)
        cleanup.callback(owner.close);ready=owner.record();assert ready['status']==0

        def observe(label):
            out=work/(label+'.json')
            subprocess.run(['open','-n','-W','--stdout',str(out),str(app),'--args','global-chain'],timeout=10,check=True)
            value=json.loads(out.read_text());assert value['ready'] and not value['interrupted']
            return value

        before=observe('before')
        input_pipe=work/'input';output_pipe=work/'output'
        os.mkfifo(input_pipe);os.mkfifo(output_pipe)
        input_fd=os.open(input_pipe,os.O_RDWR|os.O_NONBLOCK)
        output_fd=os.open(output_pipe,os.O_RDONLY|os.O_NONBLOCK)
        control=subprocess.Popen(['open','-n','-W','--stdin',str(input_pipe),'--stdout',str(output_pipe),
            str(app),'--args','tap-toggle-fixture',str(ready['pid'])])
        try:
            data=b'';deadline=time.monotonic()+10
            while b'\n' not in data:
                try: data+=os.read(output_fd,16384)
                except BlockingIOError: pass
                assert time.monotonic()<deadline,'No guardian readiness record'
                time.sleep(.02)
            disabled=json.loads(data.split(b'\n')[0]);assert not disabled['enabled'] and disabled['disable_code']==0
            during=observe('disabled')
            if args.fill_output:
                writer=os.open(output_pipe,os.O_WRONLY|os.O_NONBLOCK)
                try:
                    while True:
                        try: os.write(writer,b'x'*4096)
                        except BlockingIOError: break
                    while True:
                        try: os.write(writer,b'x')
                        except BlockingIOError: break
                finally: os.close(writer)
            else:
                # No remaining stdout reader. Final restoration logging gets EPIPE.
                os.close(output_fd);output_fd=None
            os.kill(disabled['pid'],signal.SIGKILL)
            control.wait(timeout=10)
            time.sleep(.15)
            after=observe('after')
            deadline=time.monotonic()+3
            while True:
                state=subprocess.run(['ps','-p',str(disabled['guardian_pid']),'-o','stat='],capture_output=True,text=True).stdout.strip()
                if not state or state.startswith('Z'): break
                assert time.monotonic()<deadline,f'Guardian stuck after restoration: {state}'
                time.sleep(.02)
            states=lambda c:{t['id']:t['enabled'] for t in c['before'] if t['pid']==ready['pid']}
            assert states(before)==states(after) and all(states(after).values())
            assert not any(states(during).values())
            points=lambda c:{(e['location'],e['position']) for e in c['events'] if e['type']==10}
            assert len(points(during))==6 and len(points(before))<6 and points(after)==points(before)
            args.output.write_text(json.dumps(dict(owner=ready,disabled=disabled,before=before,during=during,after=after,
                full_pipe=args.fill_output,guardian_final_state=state),indent=2)+'\n')
            print('PASS: controller SIGKILL plus unavailable stdout, both taps restored and guardian exited')
        finally:
            if output_fd is not None: os.close(output_fd)
            os.write(input_fd,b'release\n');os.close(input_fd)
            control.wait(timeout=12)


if __name__=='__main__':main()
