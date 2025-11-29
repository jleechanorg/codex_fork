# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Codex CLI is OpenAI's official local coding agent. This fork adds an extension system with slash commands and lifecycle hooks. The codebase has two implementations:

- **codex-rs/** - Rust implementation (active, maintained) - the primary CLI
- **codex-cli/** - Legacy TypeScript implementation (deprecated)

## Build & Test Commands

### Rust (Primary - in codex-rs/)

```bash
# Build
cargo build

# Run the CLI
cargo run --bin codex -- "your prompt"

# Run with just (requires: cargo install just)
just c "your prompt"           # alias for codex
just exec "your prompt"        # headless mode
just tui "your prompt"         # TUI mode

# Test single package (preferred)
cargo test -p codex-tui
cargo test -p codex-core

# Full test suite (use sparingly)
cargo test --all-features

# Fast tests with nextest (requires: cargo install cargo-nextest)
just test

# Lint and format
just fmt                       # format code (auto-run after changes)
just fix -p codex-tui         # fix clippy issues for specific crate
just clippy                    # check all clippy issues
```

### Monorepo (root)

```bash
pnpm install                   # install dependencies
pnpm format                    # check formatting
pnpm format:fix               # fix formatting
```

## Architecture

### Rust Crates (codex-rs/)

Crate names are prefixed with `codex-` (e.g., `core/` is `codex-core`).

Key crates:

- **core/** - Business logic, library for building Codex applications
- **tui/** - Terminal UI built with Ratatui
- **exec/** - Headless CLI for automation
- **cli/** - Multi-tool CLI providing subcommands
- **extensions/** - Slash commands and hooks system (this fork's addition)
- **protocol/** - Message types and serialization
- **mcp-server/** - MCP server implementation
- **common/** - Shared utilities

### Extension System (this fork)

Configuration locations (in order of precedence):

1. `.codexplus/settings.json` - Highest priority
2. `.claude/settings.json` - Project
3. `~/.claude/settings.json` - User global

- **Slash commands**: Markdown files in `.claude/commands/` with YAML frontmatter
- **Hooks**: Executable scripts in `.claude/hooks/` triggered on lifecycle events (UserPromptSubmit, PreToolUse, PostToolUse, SessionStart, etc.)
- Reference docs: [Claude Code slash commands](https://code.claude.com/docs/en/slash-commands) ; [Statusline](https://code.claude.com/docs/en/statusline)

## Code Style

### Rust Conventions

- Always inline format args: `format!("{x}")` not `format!("{}", x)`
- Collapse if statements per clippy::collapsible_if
- Use method references over closures: `.map(String::as_str)` not `.map(|s| s.as_str())`
- Do not use unsigned integers even if values cannot be negative
- Compare entire objects in tests, not field-by-field

### TUI Styling (Ratatui)

- Use Stylize helpers: `"text".dim()`, `.bold()`, `.cyan()`, not manual `Style`
- Simple conversions: `"text".into()` for spans, `vec![...].into()` for lines
- Avoid hardcoded `.white()` - use default foreground
- Colors: cyan for UI hints, green for success, red for errors, magenta for Codex branding
- Avoid blue, yellow, black, white as foreground colors

### Text Wrapping

- Use `textwrap::wrap` for plain strings
- Use helpers in `tui/src/wrapping.rs` for Ratatui Lines

## Testing

### Snapshot Tests (Insta)

```bash
cargo test -p codex-tui              # generate snapshots
cargo insta pending-snapshots -p codex-tui  # check pending
cargo insta accept -p codex-tui      # accept all (if intentional)
```

### Integration Tests (Core)

Use utilities in `core_test_support::responses`:

- `mount_sse_once` for mock SSE responses
- `ResponseMock::single_request()` for assertions
- Use `ev_*` constructors for SSE payloads
- Prefer `wait_for_event` over `wait_for_event_with_timeout`

## Sandbox Environment

Code runs sandboxed with these environment variables set:

- `CODEX_SANDBOX_NETWORK_DISABLED=1` - Network is disabled
- `CODEX_SANDBOX=seatbelt` - Running under macOS Seatbelt

Never modify code related to these variables - tests using them are designed for sandbox limitations.

## Documentation

Update files in `docs/` when changing APIs. Key docs:

- `docs/config.md` - Configuration reference
- `docs/slash_commands.md` - Slash command documentation
- `examples/claude/README.md` - Extension system examples
