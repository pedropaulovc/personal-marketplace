#!/usr/bin/env python3
"""
Build script: cross-compiles the hook Rust binaries for Linux x86_64 and
Windows x86_64, copies the outputs to hooks/bin/, then rewrites hooks.json
with the correct dispatch commands.

Run after any change to the Rust source or when bumping the plugin version.

Prerequisites:
  - Rust toolchain with targets:
      rustup target add x86_64-unknown-linux-gnu
      rustup target add x86_64-pc-windows-msvc
  - cargo-zigbuild + zig for Linux cross-compilation from Windows:
      cargo install cargo-zigbuild
      uv tool install ziglang
"""
import base64
import json
import os
import shutil
import subprocess
import sys

HOOKS_DIR = os.path.dirname(os.path.abspath(__file__))
BIN_DIR = os.path.join(HOOKS_DIR, 'bin')
HOOKS_JSON_PATH = os.path.join(HOOKS_DIR, 'hooks.json')

PLATFORM_TARGETS = [
    {'triple': 'x86_64-unknown-linux-gnu', 'ext': '', 'zigbuild': True},
    {'triple': 'x86_64-pc-windows-msvc', 'ext': '.exe', 'zigbuild': False},
]

# Each hook definition: event type, matcher, and which binary to dispatch to.
HOOK_DEFS = [
    {'event': 'PreToolUse',  'matcher': 'Bash', 'binary': 'windows-bash-guard'},
    {'event': 'PostToolUse', 'matcher': '',     'binary': 'unrelated-issue-detector'},
    {'event': 'Stop',        'matcher': '',     'binary': 'mediocrity-detector'},
]

CRATES = sorted({h['binary'] for h in HOOK_DEFS})


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


def dispatch_command(binary_name: str) -> str:
    """Build a cross-platform dispatch command for a hook binary.

    Same strategy as the superpowers plugin: the dispatch logic is
    base64-encoded so the outer command contains only safe chars
    ([A-Za-z0-9+/=]) — no variable expansions, no quoting to mangle,
    works identically in Git Bash and native Linux.
    """
    # Shell script with the actual dispatch logic — safe inside base64.
    script = (
        f'b="${{CLAUDE_PLUGIN_ROOT}}/hooks/bin/{binary_name}";'
        ' if [ -f "$b.exe" ]; then exec "$b.exe"; fi;'
        ' exec "$b"'
    )
    b64 = base64.b64encode(script.encode()).decode('ascii')
    # Outer command: only printf, pipe, base64, eval — no special chars.
    return f"eval \"$(printf '%s' {b64} | base64 -d)\""


def write_hooks_json() -> None:
    hooks: dict[str, list] = {}
    for h in HOOK_DEFS:
        entry = {
            'matcher': h['matcher'],
            'hooks': [
                {
                    'type': 'command',
                    'command': dispatch_command(h['binary']),
                }
            ],
        }
        hooks.setdefault(h['event'], []).append(entry)

    with open(HOOKS_JSON_PATH, 'w', encoding='utf-8') as f:
        json.dump({'hooks': hooks}, f, indent=2, ensure_ascii=False)
        f.write('\n')

    print(f"Updated {HOOKS_JSON_PATH}")


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

    write_hooks_json()
    print("Done.")


if __name__ == '__main__':
    main()
