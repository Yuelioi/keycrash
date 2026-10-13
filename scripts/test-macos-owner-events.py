#!/usr/bin/env python3
"""Observe annotated event target PIDs while a known fixture receives its hotkey.

Explicitly posts ONLY Control+Option+Shift+Command+F18 and an F19 negative control.
Does not claim observed PIDs are owners. This tests that hypothesis independently.
"""
import argparse
import json
import selectors
import subprocess
import sys
from contextlib import ExitStack
from pathlib import Path

from importlib.util import module_from_spec, spec_from_file_location

spec = spec_from_file_location("query_driver", Path(__file__).with_name("test-macos-owner-query.py"))
query_driver = module_from_spec(spec)
spec.loader.exec_module(query_driver)


def read_record(process):
    with selectors.DefaultSelector() as selector:
        selector.register(process.stdout, selectors.EVENT_READ)
        if not selector.select(10):
            raise RuntimeError("Native process startup timed out")
    line = process.stdout.readline().decode("utf-8")
    print(line, end="", flush=True)
    return json.loads(line)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixture", type=Path)
    parser.add_argument("--trigger-fixture", action="store_true", required=True)
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("Requires macOS")
    fixture = str(args.fixture.resolve(strict=True))
    with ExitStack() as cleanup:
        owner = subprocess.Popen([fixture, "hold", "79"], stdin=subprocess.PIPE,
                                 stdout=subprocess.PIPE, bufsize=0)
        cleanup.callback(query_driver.stop, owner)
        ready = read_record(owner)
        if ready.get("status") != 0:
            raise RuntimeError("Fixture registration failed")
        read_record(owner)  # Same-process read-only query result.
        monitor = subprocess.Popen([fixture, "monitor", "0"], stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, bufsize=0)
        cleanup.callback(query_driver.stop, monitor)
        if read_record(monitor).get("kind") != "monitor-ready":
            raise RuntimeError("Event observation unavailable; permissions required")
        for key in (79, 80):
            subprocess.run([fixture, "post-fixture", str(key)], check=True, timeout=5)
        output, _ = monitor.communicate(timeout=10)
        output = output.decode("utf-8")
        print(output, end="", flush=True)
        received = read_record(owner)
        if received.get("kind") != "received" or received.get("pid") != owner.pid:
            raise AssertionError("Known owner did not receive the test hotkey")
        events = [json.loads(line) for line in output.splitlines()]
        matches = [event for event in events if event.get("target") == owner.pid]
        if not matches:
            raise AssertionError("No observed target PID identified the known receiver")
        if any(event.get("key") == 80 for event in matches):
            raise AssertionError("Negative control was incorrectly routed to fixture owner")
    print("OBSERVED: known receiver appeared in event targets; production attribution remains unproven")


if __name__ == "__main__":
    main()
