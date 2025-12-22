#!/bin/bash
# PreToolUse Hook - Logs tool execution before it runs

# Read JSON payload from stdin
INPUT=$(cat)

# Parse tool information from JSON (flattened extra)
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // "unknown"')
TOOL_USE_ID=$(echo "$INPUT" | jq -r '.tool_use_id // "unknown"')

# Log to file
LOG_FILE="/tmp/codex_plus_hooks.log"
echo "[$(date)] PreToolUse: tool=$TOOL_NAME id=$TOOL_USE_ID" >> "$LOG_FILE"

# Output success (allow tool to proceed)
cat <<EOF
{
  "decision": "proceed",
  "feedback": "PreToolUse hook: $TOOL_NAME is about to execute"
}
EOF

exit 0
