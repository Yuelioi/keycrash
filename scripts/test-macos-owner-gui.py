#!/usr/bin/env python3
"""Run the native owner experiment through Launch Services and its TCC identity.

Posts only the fixture's Control+Option+Shift+Command+F18/F19 combinations.
The fixed diagnostic app launches the changing owner-query binary beside it.
"""
import argparse
import json
import os
import plistlib
import subprocess
import sys
import tempfile
import time
from contextlib import ExitStack
from pathlib import Path


def require_unlocked_desktop():
    """Fail before permission/UI operations when the console is locked."""
    root = plistlib.loads(subprocess.check_output(['ioreg', '-n', 'Root', '-d1', '-a']))
    if isinstance(root, list):
        root = root[0]
    sessions = [s for s in root.get('IOConsoleUsers', [])
                if s.get('kCGSSessionUserIDKey') == os.getuid()
                and s.get('kCGSSessionOnConsoleKey')]
    if not sessions or any(s.get('CGSSessionScreenIsLocked', False) for s in sessions):
        raise RuntimeError('Mac desktop is locked or unavailable; unlock it before running GUI tests')


class AppProcess:
    def __init__(self, app, directory, name, mode, value=None, hold=False):
        self.log = directory / f"{name}.jsonl"
        self.log.write_text("")
        self.offset = 0
        self.fd = None
        self.fifo = None
        args = ["open", "-n", "-W", "--stdout", str(self.log)]
        if hold:
            self.fifo = directory / f"{name}.fifo"
            os.mkfifo(self.fifo)
            self.fd = os.open(self.fifo, os.O_RDWR | os.O_NONBLOCK)
            args += ["--stdin", str(self.fifo)]
        args += [str(app), "--args", mode]
        if value is not None:
            args.append(str(value))
        self.process = subprocess.Popen(args)

    def record(self, timeout=10):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            data = self.log.read_text()[self.offset:]
            if "\n" in data:
                line = data.split("\n", 1)[0]
                self.offset += len(line) + 1
                result = json.loads(line)
                print(json.dumps(result), flush=True)
                return result
            if self.process.poll() is not None:
                raise RuntimeError(f"Native app exited without a record: {self.log.name}")
            time.sleep(0.02)
        raise RuntimeError(f"Timed out waiting for native record: {self.log.name}")

    def close(self):
        if self.fd is not None:
            os.write(self.fd, b"release\n")
            os.close(self.fd)
            self.fd = None
        # The native fixture exits by itself after at most 60 seconds.
        self.process.wait(timeout=65)
        if self.fifo is not None:
            self.fifo.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument("--trigger-fixture", action="store_true", required=True)
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("Requires a logged-in macOS desktop")
    app = args.app.resolve(strict=True)
    root = Path(".task/owner-proof")
    root.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="gui-", dir=root) as temp, ExitStack() as cleanup:
        directory = Path(temp).resolve()

        def launch(name, mode, value=None, hold=False):
            process = AppProcess(app, directory, name, mode, value, hold)
            cleanup.callback(process.close)
            return process

        permission = launch("permission", "authorize").record()
        if permission.get("post") is not True:
            raise RuntimeError("Diagnostic application requires Accessibility permission")
        owners = {}
        for key in (79, 80):
            owner = launch(f"owner-{key}", "hold", key, hold=True)
            ready = owner.record()
            if ready.get("status") != 0:
                raise RuntimeError("Fixture registration failed")
            owner.record()
            owners[key] = (owner, ready["pid"])
        # No PID is supplied to the monitor: discovery must be independent.
        monitor = launch("monitor", "monitor", 0)
        if monitor.record().get("kind") != "monitor-ready":
            raise RuntimeError("Event monitoring unavailable")
        for key in (79, 80, 90):
            posted = launch(f"post-{key}", "post-fixture", key).record()
            if posted != {"kind": "posted", "key": key}:
                raise AssertionError(f"Fixture input was not posted: {posted}")
        monitor.process.wait(timeout=10)
        events = [json.loads(line) for line in monitor.log.read_text().splitlines()[1:]]
        for event in events:
            print(json.dumps(event), flush=True)
        def receivers(records, key):
            return {e["target"] for e in records if e.get("type") == 14
                    and e.get("subtype") == 6 and e.get("marker") == 0x4B430000 + key}
        for key, (owner, pid) in owners.items():
            received = owner.record()
            if received.get("kind") != "received" or received.get("pid") != pid:
                raise AssertionError("Known owner did not receive its shortcut")
            if receivers(events, key) != {pid}:
                raise AssertionError(f"Wrong receiver for {key}: {receivers(events, key)}")
        if receivers(events, 90):
            raise AssertionError("Unregistered negative control identified an owner")
        for owner, _ in owners.values():
            owner.close()
        monitor = launch("monitor-released", "monitor", 0)
        if monitor.record().get("kind") != "monitor-ready":
            raise RuntimeError("Event monitoring unavailable")
        for key in (79, 80):
            launch(f"released-{key}", "post-fixture", key).record()
        monitor.process.wait(timeout=10)
        events = [json.loads(line) for line in monitor.log.read_text().splitlines()[1:]]
        if receivers(events, 79) or receivers(events, 80):
            raise AssertionError("Owner still identified after unregistration and exit")
    print("PASS: two independently discovered owners, negative control, and release")


if __name__ == "__main__":
    main()
