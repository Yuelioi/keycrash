#!/usr/bin/env python3
"""Build an ad-hoc signed KeyCrash.app ZIP on macOS; no publishing."""
import argparse
import json
import os
import platform
import plistlib
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {"arm64": "aarch64-apple-darwin", "x86_64": "x86_64-apple-darwin"}


def run(*args, **kwargs):
    return subprocess.run([str(arg) for arg in args], cwd=ROOT, check=True, **kwargs)


def make_icon(work, destination):
    # Reuse the PNG embedded in the existing ICO without an artwork dependency.
    ico = (ROOT / "assets/keycrash-icon.ico").read_bytes()
    count = struct.unpack_from("<H", ico, 4)[0]
    images = []
    for index in range(count):
        width, height, _, _, _, _, size, offset = struct.unpack_from("<BBBBHHII", ico, 6 + 16 * index)
        data = ico[offset:offset + size]
        if data.startswith(b"\x89PNG\r\n\x1a\n"):
            images.append(((width or 256) * (height or 256), data))
    if not images:
        raise RuntimeError("The application ICO contains no PNG image")
    source = work / "icon.png"
    source.write_bytes(max(images, key=lambda item: item[0])[1])
    iconset = work / "KeyCrash.iconset"
    iconset.mkdir()
    for size in [16, 32, 128, 256, 512]:
        for scale in [1, 2]:
            name = f"icon_{size}x{size}{'@2x' if scale == 2 else ''}.png"
            run("sips", "-z", str(size * scale), str(size * scale), source,
                "--out", iconset / name, stdout=subprocess.DEVNULL)
    run("iconutil", "-c", "icns", iconset, "-o", destination)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--arch", choices=[*TARGETS, "universal2"],
                        default="arm64" if platform.machine() == "arm64" else "x86_64")
    parser.add_argument("--target-dir", type=Path,
                        help="Reuse an existing Cargo build directory (preserved)")
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("Build .app bundles on macOS with Xcode Command Line Tools and Rust installed")
    architectures = list(TARGETS) if args.arch == "universal2" else [args.arch]
    metadata = json.loads(run("cargo", "metadata", "--locked", "--no-deps", "--format-version", "1",
                              capture_output=True, text=True).stdout)
    version = next(p["version"] for p in metadata["packages"] if p["name"] == "keycrash")
    task_root = ROOT / ".task"
    task_root.mkdir(exist_ok=True)
    output = ROOT / "dist"
    output.mkdir(exist_ok=True)
    archive = output / f"keycrash-v{version}-macos-{args.arch}.zip"
    env = {**os.environ, "MACOSX_DEPLOYMENT_TARGET": "11.0"}
    with tempfile.TemporaryDirectory(prefix="macos-package-", dir=task_root) as temporary:
        work = Path(temporary)
        target_dir = args.target_dir.resolve() if args.target_dir else work / "target"
        binaries = []
        for architecture in architectures:
            target = TARGETS[architecture]
            run("rustup", "target", "add", target)
            run("cargo", "build", "--locked", "--release", "-p", "keycrash", "--bin", "keycrash",
                "--target", target, "--target-dir", target_dir, env=env)
            binaries.append(target_dir / target / "release/keycrash")
        app = work / "KeyCrash.app"
        executable = app / "Contents/MacOS/keycrash"
        executable.parent.mkdir(parents=True)
        resources = app / "Contents/Resources"
        resources.mkdir()
        if len(binaries) == 1:
            shutil.copy2(binaries[0], executable)
        else:
            run("lipo", "-create", *binaries, "-output", executable)
        executable.chmod(0o755)
        run("lipo", executable, "-verify_arch", *architectures)
        make_icon(work, resources / "KeyCrash.icns")
        info = {
            "CFBundleExecutable": "keycrash", "CFBundleIdentifier": "io.github.yuelioi.keycrash",
            "CFBundleName": "KeyCrash", "CFBundleDisplayName": "KeyCrash", "CFBundlePackageType": "APPL",
            "CFBundleShortVersionString": version, "CFBundleVersion": version,
            "CFBundleIconFile": "KeyCrash.icns", "LSMinimumSystemVersion": "11.0",
            "NSHighResolutionCapable": True,
            "CFBundleDevelopmentRegion": "en", "CFBundleLocalizations": ["zh_CN", "en", "ja"],
        }
        with (app / "Contents/Info.plist").open("wb") as stream:
            plistlib.dump(info, stream)
        run("plutil", "-lint", app / "Contents/Info.plist")
        run("codesign", "--force", "--sign", "-", "--timestamp=none", app)
        run("codesign", "--verify", "--strict", app)
        staged_archive = work / archive.name
        run("ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", app, staged_archive)
        staged_archive.replace(archive)
    print(f"Built {archive}")
    print("Ad-hoc signed for testing; not Developer ID signed or notarized.")


if __name__ == "__main__":
    main()
