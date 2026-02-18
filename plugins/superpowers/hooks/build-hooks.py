#!/usr/bin/env python3
"""
Build script: reads skills/using-superpowers/SKILL.md and rewrites
hooks/hooks.json with the skill content baked in as a static string.

Run after any change to using-superpowers/SKILL.md or when bumping
the plugin version.
"""
import json
import os

PLUGIN_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SKILL_PATH = os.path.join(PLUGIN_ROOT, 'skills', 'using-superpowers', 'SKILL.md')
HOOKS_PATH = os.path.join(PLUGIN_ROOT, 'hooks', 'hooks.json')


def shell_single_quote(s: str) -> str:
    """Wrap s in single quotes, escaping any embedded single quotes."""
    return "'" + s.replace("'", "'\"'\"'") + "'"


def main():
    with open(SKILL_PATH, encoding='utf-8') as f:
        skill_content = f.read()

    context = (
        "<EXTREMELY_IMPORTANT>\n"
        "You have superpowers.\n\n"
        "**Below is the full content of your 'superpowers:using-superpowers' skill — "
        "your introduction to using skills. For all other skills, use the 'Skill' tool:**\n\n"
        + skill_content
        + "\n</EXTREMELY_IMPORTANT>"
    )

    hook_output = {
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": context,
        }
    }

    hook_json = json.dumps(hook_output, ensure_ascii=False)
    command = f"printf '%s\\n' {shell_single_quote(hook_json)}"

    hooks = {
        "hooks": {
            "SessionStart": [
                {
                    "matcher": "",
                    "hooks": [
                        {
                            "type": "command",
                            "command": command,
                        }
                    ],
                }
            ]
        }
    }

    with open(HOOKS_PATH, 'w', encoding='utf-8') as f:
        json.dump(hooks, f, indent=2, ensure_ascii=False)
        f.write('\n')

    print(f"Updated {HOOKS_PATH}")


if __name__ == '__main__':
    main()
