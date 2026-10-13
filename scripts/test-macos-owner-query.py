#!/usr/bin/env python3
"""Test whether read-only native queries distinguish two actual hotkey owners.

An experiment, not a release acceptance test. Only fixture processes are queried;
no real input is sent, no settings are changed, and children have bounded lifetimes.
"""
import argparse
import json
import selectors
import subprocess
import sys
from contextlib import ExitStack
from pathlib import Path


def stop(process):
    if process.poll() is None:
        try:
            process.communicate("release\n" if process.text_mode else b"release\n", timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.communicate()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixture", type=Path)
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("This experiment requires macOS")
    fixture = str(args.fixture.resolve(strict=True))
    with ExitStack() as cleanup:
        owners = []
        # Four modifiers + F18 / F19; query only, without triggering either.
        for key in (79, 80):
            owner = subprocess.Popen([fixture, "hold", str(key)], text=True,
                                     stdin=subprocess.PIPE, stdout=subprocess.PIPE)
            cleanup.callback(stop, owner)
            with selectors.DefaultSelector() as selector:
                selector.register(owner.stdout, selectors.EVENT_READ)
                if not selector.select(10):
                    raise RuntimeError("Fixture startup timeout")
                ready = json.loads(owner.stdout.readline())
            print(json.dumps(ready), flush=True)
            if ready.get("status") != 0:
                raise RuntimeError("Fixture could not register test shortcut")
            owners.append((owner, key))
        for owner, key in owners:
            query = subprocess.run([fixture, "query", str(owner.pid)],
                                   text=True, capture_output=True, timeout=20)
            print(query.stdout, end="", flush=True)
            if query.returncode:
                raise RuntimeError(f"Native query failed: {query.returncode}")
            result = json.loads(query.stdout)
            # An observation of the OTHER owner's key would invalidate attribution.
            keys = {entry["key"] for entry in result["entries"]}
            other_key = 80 if key == 79 else 79
            if key not in keys or other_key in keys:
                raise AssertionError(f"Cross-process attribution unproven: expected {key}, observed {keys}")
    print("PASS: native query distinguished independent owners; release-after-query and wider UID coverage still need validation")


if __name__ == "__main__":
    main()
