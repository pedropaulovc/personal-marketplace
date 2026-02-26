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
BIN_DIR = os.path.join(HOOKS_DIR, 'bin')

PLATFORM_TARGETS = [
    {'triple': 'x86_64-unknown-linux-gnu', 'ext': '', 'zigbuild': True},
    {'triple': 'x86_64-pc-windows-msvc', 'ext': '.exe', 'zigbuild': False},
]

CRATES = [
    'unrelated-issue-detector',
    'windows-bash-guard',
    'mediocrity-detector',
]


def build_target(crate_dir: str, triple: str, zigbuild: bool = False) -> None:
    cmd = 'zigbuild' if zigbuild else 'build'
    print(f"Building for {triple} (cargo {cmd})...")
    subprocess.run(
        ['cargo', cmd, '--release', '--target', triple],
        cwd=crate_dir,
        check=True,
    )


def copy_binary(crate_dir: str, crate_name: str, triple: str, ext: str) -> None:
    src = os.path.join(crate_dir, 'target', triple, 'release', crate_name + ext)
    dst = os.path.join(BIN_DIR, crate_name + ext)
    os.makedirs(BIN_DIR, exist_ok=True)
    shutil.copy2(src, dst)
    print(f"Copied {src} -> {dst}")


def main() -> None:
    for crate_name in CRATES:
        crate_dir = os.path.join(HOOKS_DIR, crate_name)
        for target in PLATFORM_TARGETS:
            try:
                build_target(crate_dir, target['triple'], target['zigbuild'])
                copy_binary(crate_dir, crate_name, target['triple'], target['ext'])
            except subprocess.CalledProcessError:
                print(f"WARNING: failed to build {crate_name} for {target['triple']}, skipping", file=sys.stderr)
                continue

    print("Done.")


if __name__ == '__main__':
    main()
