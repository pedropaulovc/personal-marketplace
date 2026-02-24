## Plugins

- Any changes to plugin code must also bump the plugin's version in its `plugin.json`, NOT the marketplace version.
- When bumping the **superpowers** plugin version, also run:
  ```
  python3 plugins/superpowers/hooks/build-hooks.py
  ```
  This bakes the current `skills/using-superpowers/SKILL.md` into `hooks/hooks.json`.
- When bumping the **personal** plugin version, also rebuild the hook binaries:
  ```
  python3 plugins/personal/hooks/build-hooks.py
  ```
  This cross-compiles the `unrelated-issue-detector` Rust binary for Linux and Windows.