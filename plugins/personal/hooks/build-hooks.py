#!/usr/bin/env python3
"""
Build script: cross-compiles the unrelated-issue-detector Rust binary for
Linux x86_64 and Windows x86_64, then copies the outputs to hooks/bin/.

Run after any change to the Rust source or when bumping the plugin version.

Prerequisites:
  - Rust toolchain with targets:
      rustup target add x86_64-unknown-linux-gnu
      rustup target add x86_64-pc-windows-msvc
  - cargo-zigbuild + zig for Linux cross-compilation from Windows:
      cargo install cargo-zigbuild
      uv tool install ziglang
"""
import os
import shutil
import subprocess
import sys

HOOKS_DIR = os.path.dirname(os.path.abspath(__file__))
CRATE_DIR = os.path.join(HOOKS_DIR, 'unrelated-issue-detector')
BIN_DIR = os.path.join(HOOKS_DIR, 'bin')

TARGETS = [
    {
        'triple': 'x86_64-unknown-linux-gnu',
        'binary': 'unrelated-issue-detector',
        'output': 'unrelated-issue-detector',
        'zigbuild': True,
    },
    {
        'triple': 'x86_64-pc-windows-msvc',
        'binary': 'unrelated-issue-detector.exe',
        'output': 'unrelated-issue-detector.exe',
        'zigbuild': False,
    },
]


def build_target(triple: str, zigbuild: bool = False) -> None:
    cmd = 'zigbuild' if zigbuild else 'build'
    print(f"Building for {triple} (cargo {cmd})...")
    subprocess.run(
        ['cargo', cmd, '--release', '--target', triple],
        cwd=CRATE_DIR,
        check=True,
    )


def copy_binary(triple: str, binary: str, output: str) -> None:
    src = os.path.join(CRATE_DIR, 'target', triple, 'release', binary)
    dst = os.path.join(BIN_DIR, output)
    os.makedirs(BIN_DIR, exist_ok=True)
    shutil.copy2(src, dst)
    print(f"Copied {src} -> {dst}")


def main() -> None:
    for target in TARGETS:
        try:
            build_target(target['triple'], target.get('zigbuild', False))
            copy_binary(target['triple'], target['binary'], target['output'])
        except subprocess.CalledProcessError:
            print(f"WARNING: failed to build for {target['triple']}, skipping", file=sys.stderr)
            continue

    print("Done.")


if __name__ == '__main__':
    main()
