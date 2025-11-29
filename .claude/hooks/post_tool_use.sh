#!/bin/bash
# PostToolUse Hook - Logs tool execution after it completes

# Read JSON payload from stdin
INPUT=$(cat)

# Parse tool information from JSON (flattened extra)
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // "unknown"')
TOOL_USE_ID=$(echo "$INPUT" | jq -r '.tool_use_id // "unknown"')

# Log to file
LOG_FILE="/tmp/codex_plus_hooks.log"
echo "[$(date)] PostToolUse: tool=$TOOL_NAME id=$TOOL_USE_ID completed" >> "$LOG_FILE"

# Output success
cat <<EOF
{
  "decision": "proceed",
  "feedback": "PostToolUse hook: $TOOL_NAME completed successfully"
}
EOF

exit 0
