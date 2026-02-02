#!/usr/bin/env python3
"""
Pre-tool-use hook that auto-wraps bash commands containing shell expansion
syntax in `bash -c '...'` to prevent Claude Code's escaping from mangling them.

Install: Add to ~/.claude/settings.json:
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{"type": "command", "command": "python3 ~/.claude/hooks/bash-autofix.py"}]
    }]
  }
}
"""
import json
import re
import sys


def needs_wrapping(command: str) -> bool:
    """Detect shell expansion patterns that get mangled by Claude Code."""
    cmd = command.strip()

    # Already wrapped - skip
    if cmd.startswith("bash -c ") or cmd.startswith("bash  -c "):
        return False

    # Patterns that get mangled:
    # - $() command substitution
    # - $var variable references
    # - `cmd` backtick substitution
    patterns = [
        r'\$\([^)]+\)',              # $(command)
        r'\$[a-zA-Z_][a-zA-Z0-9_]*', # $variable
        r'`[^`]+`',                  # `command`
    ]

    return any(re.search(p, cmd) for p in patterns)


def wrap_command(command: str) -> str:
    """Wrap command in bash -c, properly escaping single quotes."""
    # Standard technique: replace ' with '"'"'
    # This ends the single quote, adds a double-quoted single quote,
    # then starts a new single quote
    escaped = command.replace("'", "'\"'\"'")
    return f"bash -c '{escaped}'"


def main():
    try:
        input_data = json.load(sys.stdin)
    except json.JSONDecodeError:
        sys.exit(0)

    tool_name = input_data.get("tool_name", "")
    tool_input = input_data.get("tool_input", {})
    command = tool_input.get("command", "")

    # Only process Bash tool calls with commands that need fixing
    if tool_name != "Bash" or not command or not needs_wrapping(command):
        sys.exit(0)

    fixed_command = wrap_command(command)

    # Return modified input only, let Claude Code handle permissions normally
    output = {
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "updatedInput": {
                "command": fixed_command,
                "description": tool_input.get("description", ""),
            }
        }
    }
    print(json.dumps(output))
    sys.exit(0)


if __name__ == "__main__":
    main()
