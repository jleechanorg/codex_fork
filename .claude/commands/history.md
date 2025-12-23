---
description: /history Command
type: llm-orchestration
execution_mode: immediate
---
## ⚡ EXECUTION INSTRUCTIONS FOR CLAUDE
**When this command is invoked, YOU (Claude) must execute these steps immediately:**
**This is NOT documentation - these are COMMANDS to execute right now.**
**Use TodoWrite to track progress through multi-phase workflows.**

## 🚨 EXECUTION WORKFLOW

### Phase 1: Permission Error Recovery Workflow

**Action Steps:**
1. **Direct Access Attempt**: Try normal file operations first
2. **Permission Error Detected**: Immediately switch to Python fallback
3. **Python File Discovery**: Use `glob.glob(os.path.expanduser('~/.codex/sessions/**/*/*.jsonl', recursive=True))` as needed to capture nested directories
4. **Python File Reading**: Use Python Read tool or subprocess for JSONL parsing
5. **Never Give Up**: Always provide results using Python if direct access fails

The `/history` command is designed to be 100% reliable regardless of file permission configurations.

## 📋 REFERENCE DOCUMENTATION

# /history Command

**Usage**: `/history [search_query] [--options]`

**Purpose**: Search through Codex CLI + VS Code conversation history stored in `~/.codex/sessions` using intelligent execution optimization and multiple search strategies.

## 🚨 CRITICAL: Permission Handling Protocol

**MANDATORY**: If you encounter ANY permission issues while accessing `~/.codex/sessions/`, immediately use Python to read the files:

1. **Use Python subprocess** for all file operations if permission denied
2. **Never give up** on file access - always try Python fallback
3. **Read files with Python** using the provided fallback commands below
4. **All file discovery must use Python** if direct access fails

This command is designed to be resilient to permission issues by automatically falling back to Python-based file operations.

## Command Architecture

**`/history` uses `/execute`** for intelligent workflow optimization as requested by the user.

**Composition Pattern**: This command delegates to `/execute` which provides:
- Task complexity analysis 
- Execution strategy optimization
- Parallel processing when beneficial
- Resource-efficient search coordination

## Usage Examples

```bash

# Basic keyword search

/history "git merge conflicts"

# Search with date filter  

/history "database migration" --date "2025-08-01"

# Search specific project or working directory metadata

/history "authentication bug" --cwd "/home/user/workspace/worldarchitect.ai"

# Search by message type

/history --type "assistant" --keyword "pytest"

# Complex search with multiple filters

/history "performance issue" --date "2025-08" --limit 10 --branch "dev"

# Search by git branch metadata when present

/history "feature branch" --branch "dev1754"

# Fuzzy search for typos

/history "database migration" --fuzzy

# Recent conversations only

/history "latest deployment" --recent 7  # last 7 days
```

## Search Capabilities

### Text Matching Strategies

1. **Exact Match**: Default behavior for precise keyword search
2. **Case-Insensitive**: Automatic case normalization  
3. **Tokenized Search**: Finds keywords within sentences
4. **Fuzzy Search**: `--fuzzy` flag for handling typos and variations

### Filter Options

#### Time-Based Filters

- `--date "YYYY-MM-DD"` - Exact date match
- `--date "YYYY-MM"` - Month match
- `--after "YYYY-MM-DD"` - After specific date
- `--before "YYYY-MM-DD"` - Before specific date  
- `--recent N` - Last N days

#### Project/Context Filters

- `--project "name"` - Filter by project directory path (when metadata provides project field)
- `--cwd "/path"` - Filter by working directory stored in session metadata
- `--branch "branch-name"` - Filter by git branch stored in session metadata

#### Message Type Filters  

- `--type "user|assistant|summary|tool_use"` - Filter by message type (including tool executions)
- `--user-type "external"` - Filter by user type when available
- `--has-tools` - Convenience flag equivalent to `--type "tool_use"` or messages with tool activity
- `--has-errors` - Messages with error indicators

#### Output Control

- `--limit N` - Maximum results (default: 20)
- `--context N` - Lines of context around matches (default: 2)
- `--format "json|text|table"` - Output format (default: text)
- `--files-only` - Show only matching file names

## Implementation Details

### Performance Optimization (Based on Research)

1. **Streaming JSONL Processing**: Line-by-line parsing to avoid memory issues
2. **Pre-filtering Pipeline**: Filter by high-selectivity metadata first (date, cwd, branch)
3. **Parallel File Processing**: Process multiple conversation files concurrently
4. **Smart File Discovery**: Efficiently locate candidate files by directory structure

### Search Algorithm

```
1. Parse search query and extract filters
2. Discover candidate files based on metadata filters
3. For each file (in parallel):
   - Stream parse JSONL line by line
   - Pre-filter by metadata (timestamp, cwd, branch)
   - Apply text search to message content
   - Collect matches with context
4. Aggregate, rank, and format results
5. Apply output limits and formatting
```

### File Format Understanding

Codex session history lives under `~/.codex/sessions` where conversations are stored as JSONL files named like `rollout-<timestamp>-<uuid>.jsonl`. Subdirectories may follow a `YYYY/MM/DD` pattern.

**Directory Structure Example**:
```
~/.codex/sessions/
├── 2024/10/05/
│   ├── rollout-20241005-123456-uuid.jsonl
│   └── ...
└── rollout-20240930-abcdef-uuid.jsonl
```

**JSONL Format** (per conversation file):
```jsonl
{"type":"request","input":"user prompt","timestamp":"2024-10-05T12:34:56.789Z","metadata":{"cwd":"/workspace/project","branch":"feature-x"}}
{"type":"response","output":"assistant reply","timestamp":"2024-10-05T12:34:57.123Z"}
{"type":"tool","name":"bash","command":"pytest","timestamp":"2024-10-05T12:35:10.000Z","status":"success"}
```

Detection rules:
- Tool usage: records with `type:"tool"`, or entries containing `tool`/`toolCalls` metadata
- Errors: `error:true`, `status:"error"`, or standardized error markers within the content
Document any repository-specific variations here to keep the filters accurate.

### Integration with `/execute`

The `/history` command delegates its execution to `/execute` with a comprehensive task description that includes secure file access and robust permission handling:

```
/execute Search through Codex CLI + VS Code conversation history with robust file access:
- Query: [user_search_query]
- Filters: [parsed_filter_options] 
- Strategy: Python-first file access with permission error handling
- Primary Access: Use Python subprocess with explicit file operations
- Fallback Strategy: If permission denied or file access fails, ALWAYS use Python3 directly
- File Discovery: python3 -c "import glob, os; print('\n'.join(glob.glob(os.path.expanduser('~/.codex/sessions/**/*.jsonl'), recursive=True)))"
- Content Reading: Use Python Read tool or Python subprocess to read JSONL files
- Permission Recovery: On ANY permission error, immediately switch to Python-based file operations
- Error Handling: Explicit fallback instructions - "Use Python to read files if you encounter permission issues"
- Algorithm: Python file discovery → JSONL parsing → Text search → Context extraction → Results formatting
- Output: Formatted search results with conversation context
```

**CRITICAL PERMISSION HANDLING PROTOCOL:**
If Claude encounters ANY of the following issues:
1. **Permission Denied**: Use Python subprocess to read files directly
2. **File Access Errors**: Switch to Python-based file operations immediately  
3. **Directory Access Issues**: Use `python3 -c "import os; os.listdir(os.path.expanduser('~/.codex/sessions'))"`
4. **JSONL Parse Errors**: Use Python json module for robust parsing

**Python Fallback Commands:**
```bash

# Directory listing

python3 -c "import os; print('\n'.join(os.listdir(os.path.expanduser('~/.codex/sessions'))))"

# File discovery  

python3 -c "import glob, os; print('\n'.join(glob.glob(os.path.expanduser('~/.codex/sessions/**/*.jsonl'), recursive=True)))"

# File content reading (single line executable)

python3 - "$HOME/.codex/sessions/rollout-20241005-123456-uuid.jsonl" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as fh:
    for line in fh:
        data = line.strip()
        if not data:
            continue
        print(json.dumps(json.loads(data)))
PY

# File content reading (with error handling)

python3 - "$HOME/.codex/sessions/rollout-20241005-123456-uuid.jsonl" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as fh:
    for line in fh:
        data = line.strip()
        if not data:
            continue
        try:
            print(json.dumps(json.loads(data)))
        except json.JSONDecodeError as exc:
            print(f"Parse error: {exc.msg} | line={data}", file=sys.stderr)
PY
```

This allows `/execute` to:
- Analyze the search complexity (number of files, query complexity)
- Choose optimal execution strategy (parallel vs sequential)
- Handle resource management and error recovery with Python fallback
- Provide intelligent progress reporting
- **Automatically use Python for file operations if permission issues occur**

## Output Format

### Default Text Format

```
=== Codex Conversation History Search Results ===
Query: "git merge conflicts"
Found: 3 matches in 2 conversations

📁 /workspace/worldarchitect.ai | 📅 2025-08-01 | 🌿 dev1754132318
💬 "I need help resolving git merge conflicts in the authentication module..."
   Context: Discussion about merge strategy and conflict resolution tools
   File: rollout-20250801-...-uuid.jsonl

📁 /workspace/worldarchitect.ai | 📅 2025-07-28 | 🌿 main  
💬 "The git merge created conflicts in package.json..."
   Context: Package dependency conflicts during feature merge
   File: rollout-20250728-...-uuid.jsonl
```

### JSON Format (`--format json`)

```json
{
  "query": "git merge conflicts",
  "total_matches": 3,
  "total_files": 2,
  "results": [
    {
      "conversation_id": "rollout-20250801-...-uuid",
      "cwd": "/workspace/worldarchitect.ai", 
      "date": "2025-08-01",
      "branch": "dev1754132318",
      "match": {
        "message": "I need help resolving git merge conflicts...",
        "context": ["Previous message...", "Matched message", "Next message..."],
        "timestamp": "2025-08-01T11:00:11.845Z"
      }
    }
  ]
}
```

### Table Format (`--format table`)

```
Query: git merge conflicts | Matches: 3 | Files: 2
┌───────────────────────────────┬─────────────────────────────┬──────────────┬───────────────┬──────────────────────────────────────┐
│ conversation_id               │ cwd                         │ date         │ branch        │ message_snippet                      │
├───────────────────────────────┼─────────────────────────────┼──────────────┼───────────────┼──────────────────────────────────────┤
│ rollout-20250801-...-uuid     │ /workspace/worldarchitect.ai│ 2025-08-01   │ dev1754132318 │ I need help resolving git merge ...  │
│ rollout-20250728-...-uuid     │ /workspace/worldarchitect.ai│ 2025-07-28   │ main          │ The git merge created conflicts ...  │
└───────────────────────────────┴─────────────────────────────┴──────────────┴───────────────┴──────────────────────────────────────┘
```

## Error Handling

- **Missing Directory**: Graceful handling if `~/.codex/sessions` doesn't exist - use Python to check with `os.path.exists()`
- **Corrupted JSONL**: Skip malformed lines with warning using Python json module
- **Permission Issues**: **AUTOMATIC PYTHON FALLBACK** - never fail due to permissions, always try Python subprocess
- **Large Result Sets**: Automatic pagination and memory management
- **No Matches**: Helpful suggestions for refining search query
- **File Access Failures**: Immediate switch to Python-based file operations with detailed error recovery

## Integration Features

### Secure File Access

The command uses secure direct access to `~/.codex/sessions/` directory, avoiding symlink vulnerabilities while maintaining search functionality within the project workspace.

### Memory MCP Integration

Search patterns and frequently accessed conversations can be cached in Memory MCP for faster repeated searches.

### Command Composition  

Can be combined with other commands:
```bash
/history "database issue" | /learn    # Learn from historical database solutions
/history "deployment error" | /debug  # Debug with historical context
```

## Advanced Features

### Semantic Search (Future Enhancement)

- Leverage embeddings for conceptual similarity matching
- Find conversations about similar topics even with different keywords

### Query Intelligence

- Auto-suggest corrections for common typos
- Recommend related search terms based on conversation patterns
- Historical query analysis for better search suggestions

## Examples of Complex Searches

```bash

# Find all tool usage in August 2025

/history --type "tool" --date "2025-08" --format table

# Search for authentication-related conversations in specific working directory

/history "auth login password" --cwd "/workspace/worldarchitect" --limit 5

# Find recent error discussions 

/history "error exception failed" --recent 14 --has-tools

# Fuzzy search with context

/history "database migration problem" --fuzzy --context 5

# Find conversations about specific files or commands

/history "main.py" --cwd "/workspace/worldarchitect" --format json
```

This command provides comprehensive conversation history search capabilities tailored to Codex session archives while leveraging `/execute` for intelligent performance optimization as requested.
