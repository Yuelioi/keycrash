#!/usr/bin/env python3
"""Check Carbon lifetime across separate processes; never synthesize input."""
import argparse
import selectors
import subprocess
import sys
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixture", type=Path, help="compiled macos-hotkey-fixture executable")
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("Run in a logged-in macOS desktop session")
    fixture = str(args.fixture.resolve(strict=True))
    # Control + Option + Shift + Command, F13..F20; find an unused test key.
    modifiers = "6912"

    def run(command, key):
        return subprocess.run([fixture, command, str(key), modifiers], check=True,
                              capture_output=True, text=True, timeout=10).stdout.strip()

    key = next((key for key in [105, 107, 113, 106, 64, 79, 80, 90]
                if run("system", key) == "Ok(false)" and run("probe", key) == "AVAILABLE"), None)
    if key is None:
        raise RuntimeError("No free test key; close shortcut tools and retry")
    for options in [1, 0]:
        owner = subprocess.Popen([fixture, "hold", str(key), modifiers, str(options)],
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, text=True)
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(owner.stdout, selectors.EVENT_READ)
                if not selector.select(timeout=10):
                    raise RuntimeError("Owner startup timed out")
                if owner.stdout.readline().strip() != "READY":
                    raise RuntimeError("Owner failed to register the fixture hotkey")
            result = run("probe", key)
            print(f"owner options={options}: {result}", flush=True)
            if options == 1 and result != "BLOCKED":
                raise AssertionError("Exclusive owner was not detected")
            # SDK documentation differs on shared incumbents. Record the actual
            # behavior without converting success into a claim of no listeners.
            if result not in ("AVAILABLE", "BLOCKED"):
                raise AssertionError(result)
            owner.communicate("release\n", timeout=10)
            if owner.returncode:
                raise RuntimeError(f"Owner exited with {owner.returncode}")
        finally:
            if owner.poll() is None:
                owner.terminate()
                try:
                    owner.communicate(timeout=5)
                except subprocess.TimeoutExpired:
                    owner.kill()
                    owner.communicate()
        assert run("probe", key) == "AVAILABLE", "Key remained occupied after release"
        assert run("probe", key) == "AVAILABLE", "Probe leaked its registration"
    print("PASS: free → exclusive conflict → released; shared-owner behavior recorded")


if __name__ == "__main__":
    main()
