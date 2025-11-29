#!/usr/bin/env python3
"""
Hook Metadata:
name: post-add-header
type: PostOutput
priority: 50
enabled: true
"""
import json
import logging
import sys


def main() -> int:
    try:
        payload = json.load(sys.stdin)
    except Exception:
        payload = {}

    output = {
        "decision": "allow",
        "hookSpecificOutput": {"x_hooked": "1"},
    }

    # This example does not modify streaming responses; add logic here if needed.
    try:
        print(json.dumps(output))
    except Exception as exc:  # noqa: BLE001
        logging.getLogger(__name__).debug("Hook failed: %s", exc)
        print(json.dumps({"decision": "allow"}))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        raise
    except Exception as exc:  # noqa: BLE001
        logging.getLogger(__name__).debug("Hook failed: %s", exc)
        print(json.dumps({"decision": "allow"}))
        sys.exit(0)
