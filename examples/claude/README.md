# Codex Extensions Examples

This directory contains example slash commands, hooks, and configuration for the Codex CLI extensions system.

## Quick Start

To use these examples:

1. Copy the entire `claude` directory to your home directory or project root:
   ```bash
   cp -r examples/claude ~/.claude
   # or for project-specific config:
   cp -r examples/claude /path/to/your/project/.claude
   ```

2. Make hook scripts executable:
   ```bash
   chmod +x ~/.claude/hooks/*.py
   ```

3. Test a slash command:
   ```bash
   codex "/hello World"
   ```

## Directory Structure

```
.claude/
├── commands/           # Slash command definitions (markdown files)
├── hooks/              # Hook scripts (executable files)
└── settings.json       # Hook and extension configuration
```

## Slash Commands

Slash commands are markdown files with YAML frontmatter located in `.claude/commands/`.

### Available Commands

- `/hello [name]` - Simple greeting command
- `/echo [text]` - Echo test command
- `/copilot [PR]` - Autonomous PR processing
- `/consensus` - Consensus/voting functionality
- `/cons` - Cons listing
- `/history` - History viewing
- `/reviewdeep` - Deep code review
- `/test-args` - Argument testing

### Command Format

```markdown
---
name: command-name
description: What this command does
---

# Command Instructions

Tell Claude what to do when this command is executed.

You can use $ARGUMENTS to reference the arguments passed to the command.
```

### Creating Custom Commands

1. Create a new `.md` file in `.claude/commands/`
2. Add YAML frontmatter with `name` and `description`
3. Write instructions for Claude in the body
4. Use `$ARGUMENTS` placeholder for user input

Example:
```markdown
---
name: mycommand
description: My custom command
---

# My Command

When this command is executed:

1. Do something with: $ARGUMENTS
2. Provide detailed output
```

## Hooks

Hooks are executable scripts that run at specific points in the Codex CLI lifecycle.

### Available Hooks

- `add_context.py` - UserPromptSubmit hook that adds context to prompts
- `post_add_header.py` - Post-output hook example
- `shared_utils.py` - Shared utilities for hooks

### Hook Events

- **UserPromptSubmit**: Before sending user input to Claude
- **PreToolUse**: Before executing a tool
- **PostToolUse**: After executing a tool
- **SessionStart**: When a Codex session starts
- **SessionEnd**: When a Codex session ends
- **Stop**: When conversation ends
- **PreCompact**: Before compacting conversation history
- **Notification**: On system notifications

### Hook Input/Output

Hooks receive JSON input via stdin:
```json
{
  "session_id": "session-123",
  "transcript_path": "/path/to/transcript",
  "cwd": "/current/working/dir",
  "hook_event_name": "UserPromptSubmit",
  "prompt": "user input text"
}
```

Hooks output JSON to stdout:
```json
{
  "decision": "allow" or "block" or "deny",
  "reason": "Why blocked (if blocking)",
  "hookSpecificOutput": {
    "hookEventName": "UserPromptSubmit",
    "additionalContext": "Extra context to inject"
  }
}
```

### Creating Custom Hooks

1. Create an executable script in `.claude/hooks/`
2. Read JSON from stdin
3. Perform your logic
4. Write JSON to stdout
5. Exit with 0 (success), 1 (error), or 2 (block)

Example Python hook:
```python
#!/usr/bin/env python3
import json
import sys

# Read input
input_data = json.load(sys.stdin)

# Your logic here
result = {"decision": "allow"}

# Output result
print(json.dumps(result))
sys.exit(0)
```

Example Shell hook:
```bash
#!/bin/bash
# Read stdin
input=$(cat)

# Your logic here

# Output JSON
echo '{"decision": "allow"}'
exit 0
```

## Configuration (settings.json)

The `settings.json` file configures hooks and extensions.

### Example Configuration

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "add_context.py",
            "timeout": 5
          }
        ]
      }
    ]
  },
  "statusLine": {
    "type": "command",
    "command": "git branch --show-current",
    "timeout": 2,
    "mode": "prepend"
  }
}
```

### Hook Configuration

Each hook event can have multiple entries:
- `matcher`: Optional regex pattern for tool names (for PreToolUse/PostToolUse)
- `hooks`: Array of hook commands to execute
- `timeout`: Maximum execution time in seconds (default: 5)

### Status Line Configuration

Optional status line shown before Claude's responses:
- `type`: Always "command"
- `command`: Shell command to execute
- `timeout`: Maximum execution time in seconds (default: 2)
- `mode`: "prepend" or "append"

## Configuration Precedence

Settings are loaded with the following precedence (later overrides earlier):
1. `~/.claude/settings.json` (user home)
2. `.claude/settings.json` (project directory)
3. `.codexplus/settings.json` (highest priority)

## Tips

1. **Test hooks independently**: Run them directly with test JSON input
   ```bash
   echo '{"session_id":"test"}' | .claude/hooks/add_context.py
   ```

2. **Debug hooks**: Write to stderr for debugging (won't affect JSON output)
   ```python
   print("Debug info", file=sys.stderr)
   ```

3. **Timeout handling**: Keep hooks fast (under 5 seconds) to avoid timeouts

4. **Security**: Review hook scripts before execution, especially from untrusted sources

5. **Permissions**: Ensure hook scripts are executable (`chmod +x`)

## Troubleshooting

**Hook not executing**:
- Check file permissions (`ls -la .claude/hooks/`)
- Verify settings.json syntax (use a JSON validator)
- Check hook event name spelling

**Hook timing out**:
- Reduce timeout in settings.json
- Optimize hook script performance
- Check for hanging subprocesses

**Command not found**:
- Verify command file exists in `.claude/commands/`
- Check YAML frontmatter has `name` field
- Ensure frontmatter is valid YAML

## More Information

- [Extension System Architecture](../../DESIGN.md)
- [Implementation Details](../../SPEC.md)
- [Source Code](../../codex-rs/extensions/)

## License

These examples are provided under the same license as the Codex project (Apache-2.0).
