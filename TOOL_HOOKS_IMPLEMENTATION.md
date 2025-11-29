# Tool Hooks Implementation

## Summary

Implemented PreToolUse and PostToolUse hook execution in the tool orchestrator to support hook-based tool interception and auditing across all execution modes (TUI and exec).

## Changes Made

### 1. Core Dependencies (`codex-rs/core/Cargo.toml`)

Added `codex-extensions` dependency to the core crate to enable hook system integration:

```toml
codex-extensions = { workspace = true }
```

### 2. Tool Orchestrator (`codex-rs/core/src/tools/orchestrator.rs`)

#### Imports Added

```rust
use codex_extensions::HookEvent;
use codex_extensions::HookInput;
use codex_extensions::HookSystem;
use codex_extensions::Settings;
use std::collections::HashMap;
```

#### New Functions

**`execute_pre_tool_use_hooks()`** (lines 218-279)

- Executes PreToolUse hooks before tool execution
- Loads settings and checks if PreToolUse hooks are configured
- Builds hook input with tool metadata (tool_name, tool_use_id, tool_input)
- Executes hooks and checks for blocking decisions
- Returns `ToolError::Rejected` if any hook blocks execution

**`execute_post_tool_use_hooks()`** (lines 281-338)

- Executes PostToolUse hooks after tool execution
- Similar structure to PreToolUse but includes tool_response in context
- Logs warnings if PostToolUse hooks attempt to block (informational only)
- Does not fail tool execution if PostToolUse hooks error (line 124)

#### Integration Points

**Initial tool execution** (lines 110-127):

- PreToolUse hooks execute before `tool.run()` (line 112)
- PostToolUse hooks execute after successful execution (lines 118-124)
- PostToolUse errors are logged but don't fail the tool

**Sandbox retry execution** (lines 193-211):

- PreToolUse hooks execute before retry (line 194)
- PostToolUse hooks execute after successful retry (lines 200-208)

## Hook Behavior

### PreToolUse Hooks

- Execute **before** tool runs
- Can **block** tool execution by:
  - Returning exit code 2
  - Returning JSON: `{"decision": "block", "reason": "..."}`
- Blocking returns `ToolError::Rejected` with reason
- Execution stops if blocked

### PostToolUse Hooks

- Execute **after** tool runs successfully
- Can inspect tool results
- **Cannot block** tool execution (tool already ran)
- Errors are logged but don't fail the tool
- Blocking decisions are logged as warnings

## Testing

### Existing Tests

The existing integration tests in `codex-rs/extensions/tests/integration_tests.rs` verify:

- Hook execution with PreToolUse events (line 189-253)
- Hook blocking behavior (exit code 2)
- Multiple hook execution

### Manual Testing Setup

Test configuration files have been created in `.claude/`:

**`.claude/settings.json`**:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "test_tool_hook.sh",
            "timeout": 5
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "test_tool_hook.sh",
            "timeout": 5
          }
        ]
      }
    ]
  }
}
```

**`.claude/hooks/test_tool_hook.sh`**:
Logs hook executions to `/tmp/codex_hook_test.log`

### Running Manual Tests

1. Build codex:

   ```bash
   cd codex-rs && cargo build --bin codex
   ```

2. Clear log file:

   ```bash
   rm -f /tmp/codex_hook_test.log
   ```

3. Run codex exec with tool usage:

   ```bash
   cargo run --bin codex -- exec --yolo "run echo 'test'"
   ```

4. Check hook log:
   ```bash
   cat /tmp/codex_hook_test.log
   ```

Expected output:

```
[timestamp] Hook executed: PreToolUse for tool: Bash
[timestamp] Hook executed: PostToolUse for tool: Bash
```

## Implementation Details

### Error Handling

- PreToolUse hook errors convert `ExtensionError` to `ToolError::Rejected`
- PostToolUse hook errors are silently caught (don't fail tool execution)
- Hook blocking uses `ToolError::Rejected` for consistency

### Session Context

- Uses `turn_ctx.sub_id` for session_id (not `session_id` field)
- Empty JSON objects used for tool_input/tool_response (tools don't implement Serialize)
- Hook input includes: session_id, cwd, hook_event_name, extra metadata

### Sandbox Integration

- Hooks execute in both initial and retry attempts
- Hooks run before sandbox selection
- PreToolUse can prevent sandbox escalation by blocking

## Known Limitations

1. **Tool data not serialized**: tool_input and tool_response are empty JSON objects because tool types don't implement `serde::Serialize`. Hooks receive tool_name and tool_use_id but not full request/response data.

2. **PostToolUse can't block**: PostToolUse hooks execute after the tool completes, so blocking decisions are informational only.

3. **Retry hooks**: Both PreToolUse and PostToolUse execute twice if sandbox retry occurs - once for initial attempt, once for retry.

## Files Modified

- `codex-rs/core/Cargo.toml` - Added codex-extensions dependency
- `codex-rs/core/src/tools/orchestrator.rs` - Added hook execution functions and integration

## Files Created (for testing)

- `.claude/settings.json` - Hook configuration
- `.claude/hooks/test_tool_hook.sh` - Test hook script
- `TOOL_HOOKS_IMPLEMENTATION.md` - This documentation
