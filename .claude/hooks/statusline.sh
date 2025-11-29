#!/bin/bash
# StatusLine Hook - Shows hook execution in status output
# Receives UserPromptSubmit event data via stdin

# Read JSON payload from stdin
INPUT=$(cat)

# Parse session_id and prompt from JSON
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // "unknown"')
PROMPT=$(echo "$INPUT" | jq -r '.extra.user_prompt // "no prompt"' | head -c 50)

# Log to file for debugging
LOG_FILE="/tmp/codex_plus_hooks.log"
echo "[$(date)] UserPromptSubmit: session=$SESSION_ID prompt='$PROMPT...'" >> "$LOG_FILE"

# Output success feedback (JSON format)
cat <<EOF
{
  "decision": "proceed",
  "feedback": "StatusLine hook executed for session $SESSION_ID"
}
EOF

exit 0
