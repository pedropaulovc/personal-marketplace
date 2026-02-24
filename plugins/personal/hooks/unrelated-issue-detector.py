#!/usr/bin/env python3
"""
PostToolUse hook that detects when the agent dismisses issues as "unrelated"
or "pre-existing" and forces investigation via a parallel worktree agent.

Fires after every tool call. Reads only NEW transcript content since the
last check (tracked via a per-session offset file) so each dismissal is
caught exactly once without re-triggering on old matches.
"""
import json
import os
import re
import sys
import tempfile

# Patterns indicating the agent is dismissing an issue as unrelated.
DISMISSAL_PATTERNS = [
    r"(?:existing|pre-existing|preexisting)\s+(?:issue|bug|problem|error|defect)",
    r"(?:not|isn'?t|is\s+not)\s+(?:related|caused|introduced)\s+(?:to|by)\s+(?:this|our|the|my)",
    r"unrelated\s+(?:issue|bug|problem|error|to\s+(?:this|our|the))",
    r"separate\s+(?:issue|bug|problem|concern|matter)",
    r"(?:outside|beyond)\s+(?:the\s+)?scope\s+of\s+(?:this|our|the)",
    r"(?:was\s+)?already\s+(?:present|broken|failing|there)\s+(?:before|on\s+main|in\s+main)",
    r"known\s+(?:issue|bug|problem|limitation)",
    r"not\s+something\s+we\s+introduced",
    r"(?:this|the)\s+(?:issue|bug|problem|error)\s+(?:is|was|appears?)\s+(?:to\s+be\s+)?(?:pre-existing|preexisting|unrelated)",
]

INVESTIGATION_INSTRUCTIONS = """\
STOP. You just dismissed an issue as "unrelated" or "pre-existing". \
You MUST investigate before moving on.

Use the Task tool to spawn a parallel investigation agent with these exact parameters:
- subagent_type: "general-purpose"
- isolation: "worktree"
- model: "haiku"
- run_in_background: true

In the prompt, include the FULL description of the issue you dismissed \
(error messages, symptoms, affected code, reproduction steps — everything \
the investigation agent needs to work independently).

The investigation agent must follow these steps:

1. **Identify the issue**: Clearly state what was dismissed as unrelated.
2. **Reproduce on main**: The worktree starts on HEAD. Checkout the main \
branch (`git checkout main`) and attempt to reproduce the exact same \
issue/error.
3. **Report findings**:
   - **If the issue REPRODUCES on main** (truly pre-existing): \
file a GitHub issue using `gh issue create` with:
     - Clear title prefixed with the affected component
     - Full description of the issue
     - Steps to reproduce on main
     - Expected vs actual behavior
     - Note: "Discovered while working on [branch-name]"
     - Label: `bug`
   - **If the issue does NOT reproduce on main** (introduced by current \
changes): exit and report that the issue is NOT pre-existing and the \
original agent MUST fix it.

After the investigation agent completes:
- If a bug was filed (truly unrelated): you may proceed with your current task.
- If the issue is NOT pre-existing: you MUST fix it before continuing. \
Do NOT dismiss it again.

Do NOT skip this investigation. Do NOT dismiss issues without evidence."""


def offset_path(session_id: str) -> str:
    """Per-session file tracking the last scanned transcript byte offset."""
    return os.path.join(tempfile.gettempdir(), f"unrelated-issue-{session_id}.offset")


def read_offset(session_id: str) -> int:
    try:
        with open(offset_path(session_id), "r") as f:
            return int(f.read().strip())
    except (IOError, ValueError):
        return 0


def save_offset(session_id: str, offset: int) -> None:
    try:
        with open(offset_path(session_id), "w") as f:
            f.write(str(offset))
    except OSError:
        pass


def extract_assistant_text(entry: dict) -> str:
    """Extract text content from a transcript JSONL entry."""
    role = entry.get("role", "")
    msg_type = entry.get("type", "")

    content = None
    if role == "assistant":
        content = entry.get("content", "")
    elif msg_type == "assistant":
        content = entry.get("message", {}).get("content", "")

    if content is None:
        return ""
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return " ".join(
            item.get("text", "")
            for item in content
            if isinstance(item, dict) and item.get("type") == "text"
        )
    return ""


def detect_dismissal(text: str) -> bool:
    if not text:
        return False
    text_lower = text.lower()
    return any(re.search(p, text_lower) for p in DISMISSAL_PATTERNS)


def main():
    try:
        input_data = json.load(sys.stdin)
    except (json.JSONDecodeError, EOFError):
        sys.exit(0)

    session_id = input_data.get("session_id", "unknown")
    transcript_path = input_data.get("transcript_path", "")
    if not transcript_path:
        sys.exit(0)

    last_offset = read_offset(session_id)

    # Read only new transcript content since last check.
    try:
        with open(transcript_path, "r", encoding="utf-8") as f:
            f.seek(0, 2)
            current_size = f.tell()
            if current_size <= last_offset:
                sys.exit(0)
            f.seek(last_offset)
            new_content = f.read()
    except (IOError, OSError):
        sys.exit(0)

    # Always advance the offset so we never re-scan the same content.
    save_offset(session_id, current_size)

    # Extract assistant text from new transcript entries.
    texts = []
    for line in new_content.strip().split("\n"):
        line = line.strip()
        if not line:
            continue
        try:
            entry = json.loads(line)
        except json.JSONDecodeError:
            continue
        text = extract_assistant_text(entry)
        if text:
            texts.append(text)

    combined = "\n".join(texts)

    if not detect_dismissal(combined):
        sys.exit(0)

    # Inject investigation instructions into the agent's next loop iteration.
    print(json.dumps({"decision": "block", "reason": INVESTIGATION_INSTRUCTIONS}))


if __name__ == "__main__":
    main()
