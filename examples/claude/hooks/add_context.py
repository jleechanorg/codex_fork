#!/usr/bin/env python3
"""
Hook Metadata:
name: add-context
type: UserPromptSubmit
priority: 50
enabled: true
"""
import json
import sys


def main() -> int:
    try:
        payload = json.load(sys.stdin)
    except Exception:
        payload = {}

    original_prompt = ""
    extra = payload.get("extra")
    if isinstance(extra, dict):
        original_prompt = extra.get("prompt", "")

    output = {
        "decision": "allow",
        "hookSpecificOutput": {
            "hookEventName": payload.get("hook_event_name", ""),
            "additionalContext": "CTX-123",
        },
    }

    # Uncomment to demonstrate prompt modification
    # if original_prompt:
    #     output["prompt"] = f"[CTX] {original_prompt}"

    print(json.dumps(output))
    return 0


if __name__ == "__main__":
    sys.exit(main())
