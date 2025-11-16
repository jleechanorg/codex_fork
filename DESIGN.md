# Codex Fork Design Document

## Executive Summary

This document describes the technical design for integrating OpenAI's Codex CLI with enhanced features from codex_plus, creating a unified fork that extends the base functionality while maintaining full compatibility with the upstream project.

## Architecture Overview

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Codex Fork                               │
│                                                              │
│  ┌────────────────────────────────────────────────────┐    │
│  │         OpenAI Codex Core (Base Layer)             │    │
│  │                                                     │    │
│  │  • Core CLI functionality                          │    │
│  │  • Agent runtime                                   │    │
│  │  • Tool execution                                  │    │
│  │  • Configuration system                            │    │
│  │  • Authentication                                  │    │
│  │  • MCP integration                                 │    │
│  └────────────────────────────────────────────────────┘    │
│                         ↑                                    │
│                         │ Extends                            │
│                         │                                    │
│  ┌────────────────────────────────────────────────────┐    │
│  │      codex_plus Extensions (Extension Layer)       │    │
│  │                                                     │    │
│  │  • Slash command system                            │    │
│  │  • Hook framework                                  │    │
│  │  • Additional utilities                            │    │
│  │  • Custom configurations                           │    │
│  └────────────────────────────────────────────────────┘    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Design Principles

1. **Non-Breaking Extension**: codex_plus features extend but never modify core OpenAI Codex behavior
2. **Plugin Architecture**: Extensions operate as plugins that can be enabled/disabled
3. **Clear Separation**: Maintain clear boundaries between core and extension code
4. **Upstream Compatibility**: Ability to pull updates from OpenAI Codex upstream
5. **Test Isolation**: Tests for core vs extensions are clearly separated

## Component Design

### 1. Slash Command System

#### Purpose
Provide a command-line interface extension allowing users to execute custom commands within the Codex CLI environment.

#### Architecture

```
┌──────────────────────────────────────────────────┐
│         Slash Command Registry                    │
│                                                   │
│  ┌─────────────────────────────────────────┐    │
│  │  Command Parser                         │    │
│  │  • Detects slash commands               │    │
│  │  • Parses arguments                     │    │
│  └─────────────────────────────────────────┘    │
│                    ↓                              │
│  ┌─────────────────────────────────────────┐    │
│  │  Command Router                         │    │
│  │  • Routes to appropriate handler        │    │
│  │  • Manages command lifecycle            │    │
│  └─────────────────────────────────────────┘    │
│                    ↓                              │
│  ┌─────────────────────────────────────────┐    │
│  │  Command Executors                      │    │
│  │  • Built-in commands                    │    │
│  │  • Custom commands from codex_plus      │    │
│  │  • User-defined commands                │    │
│  └─────────────────────────────────────────┘    │
└──────────────────────────────────────────────────┘
```

#### Implementation Strategy

**Location**: `codex-rs/extensions/slash-commands/` (new directory)

**Core Components**:
1. `command_registry.rs` - Central registry for all slash commands
2. `command_parser.rs` - Parses slash command syntax
3. `command_executor.rs` - Executes slash commands
4. `builtin_commands/` - Directory for built-in slash commands from codex_plus

**Integration Points**:
- Hook into Codex CLI input processing
- Integrate with existing command execution pipeline
- Leverage existing tool system where applicable

**Configuration**:
```toml
# ~/.codex/config.toml
[slash_commands]
enabled = true
custom_commands_dir = "~/.codex/commands"
```

#### Key Interfaces

```rust
/// Represents a slash command
pub trait SlashCommand {
    /// Command name (without the slash)
    fn name(&self) -> &str;

    /// Command description for help text
    fn description(&self) -> &str;

    /// Execute the command
    fn execute(&self, args: &[String], context: &Context) -> Result<CommandOutput>;

    /// Validate arguments
    fn validate_args(&self, args: &[String]) -> Result<()>;
}

/// Registry for managing slash commands
pub struct CommandRegistry {
    commands: HashMap<String, Box<dyn SlashCommand>>,
}

impl CommandRegistry {
    pub fn register(&mut self, command: Box<dyn SlashCommand>);
    pub fn execute(&self, command_line: &str, context: &Context) -> Result<CommandOutput>;
    pub fn list_commands(&self) -> Vec<&dyn SlashCommand>;
}
```

### 2. Hook System

#### Purpose
Provide event-driven extensibility allowing users to execute custom logic at specific points in the Codex CLI lifecycle.

#### Architecture

```
┌──────────────────────────────────────────────────┐
│              Hook System                          │
│                                                   │
│  ┌─────────────────────────────────────────┐    │
│  │  Hook Points (Events)                   │    │
│  │  • pre_execution                        │    │
│  │  • post_execution                       │    │
│  │  • on_error                             │    │
│  │  • on_tool_use                          │    │
│  │  • session_start / session_end          │    │
│  └─────────────────────────────────────────┘    │
│                    ↓                              │
│  ┌─────────────────────────────────────────┐    │
│  │  Hook Manager                           │    │
│  │  • Registers hooks                      │    │
│  │  • Triggers hooks at appropriate times  │    │
│  │  • Manages hook execution order         │    │
│  └─────────────────────────────────────────┘    │
│                    ↓                              │
│  ┌─────────────────────────────────────────┐    │
│  │  Hook Executors                         │    │
│  │  • Shell script hooks                   │    │
│  │  • Compiled binary hooks                │    │
│  │  • Plugin hooks                         │    │
│  └─────────────────────────────────────────┘    │
└──────────────────────────────────────────────────┘
```

#### Implementation Strategy

**Location**: `codex-rs/extensions/hooks/` (new directory)

**Core Components**:
1. `hook_manager.rs` - Central manager for all hooks
2. `hook_executor.rs` - Executes hooks safely
3. `hook_points.rs` - Defines all available hook points
4. `builtin_hooks/` - Directory for built-in hooks from codex_plus

**Integration Points**:
- Instrument key points in Codex CLI lifecycle
- Integrate with existing event system (if any)
- Provide context information to hooks

**Configuration**:
```toml
# ~/.codex/config.toml
[hooks]
enabled = true
hooks_dir = "~/.codex/hooks"
timeout_ms = 5000

[hooks.session_start]
enabled = true
scripts = ["init_workspace.sh", "check_dependencies.sh"]

[hooks.pre_execution]
enabled = true
scripts = ["validate_context.sh"]

[hooks.post_execution]
enabled = true
scripts = ["cleanup.sh"]
```

#### Key Interfaces

```rust
/// Hook execution point in the lifecycle
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HookPoint {
    SessionStart,
    SessionEnd,
    PreExecution,
    PostExecution,
    OnError,
    OnToolUse,
}

/// Context provided to hooks
pub struct HookContext {
    pub hook_point: HookPoint,
    pub working_directory: PathBuf,
    pub environment: HashMap<String, String>,
    pub metadata: serde_json::Value,
}

/// Represents a hook
pub trait Hook {
    fn name(&self) -> &str;
    fn hook_point(&self) -> HookPoint;
    fn execute(&self, context: &HookContext) -> Result<HookOutput>;
}

/// Manager for hooks
pub struct HookManager {
    hooks: HashMap<HookPoint, Vec<Box<dyn Hook>>>,
}

impl HookManager {
    pub fn register(&mut self, hook: Box<dyn Hook>);
    pub fn trigger(&self, point: HookPoint, context: &HookContext) -> Result<Vec<HookOutput>>;
    pub fn list_hooks(&self, point: Option<HookPoint>) -> Vec<&dyn Hook>;
}
```

### 3. Extension Integration Layer

#### Purpose
Provide seamless integration between OpenAI Codex core and codex_plus extensions without modifying core code.

#### Architecture

```
┌──────────────────────────────────────────────────┐
│         Extension Manager                         │
│                                                   │
│  ┌─────────────────────────────────────────┐    │
│  │  Extension Loader                       │    │
│  │  • Discovers extensions                 │    │
│  │  • Validates compatibility              │    │
│  │  • Initializes extensions               │    │
│  └─────────────────────────────────────────┘    │
│                    ↓                              │
│  ┌─────────────────────────────────────────┐    │
│  │  Extension Registry                     │    │
│  │  • Slash Commands                       │    │
│  │  • Hooks                                │    │
│  │  • Utilities                            │    │
│  └─────────────────────────────────────────┘    │
│                    ↓                              │
│  ┌─────────────────────────────────────────┐    │
│  │  Integration Points                     │    │
│  │  • CLI input interception               │    │
│  │  • Event emission                       │    │
│  │  • Configuration extension              │    │
│  └─────────────────────────────────────────┘    │
└──────────────────────────────────────────────────┘
```

#### Implementation Strategy

**Location**: `codex-rs/extensions/` (new directory)

**Core Components**:
1. `mod.rs` - Extension module entry point
2. `manager.rs` - Extension manager
3. `loader.rs` - Dynamic extension loading
4. `slash-commands/` - Slash command system
5. `hooks/` - Hook system

**Integration Approach**:
- Minimal changes to core Codex files
- Use dependency injection where possible
- Leverage existing plugin patterns in Rust codebase
- Feature flags for enabling/disabling extensions

## Directory Structure

```
codex_fork/
├── codex-rs/
│   ├── extensions/              # NEW: Extension layer
│   │   ├── mod.rs               # Extension module entry
│   │   ├── manager.rs           # Extension manager
│   │   ├── loader.rs            # Extension loader
│   │   ├── slash-commands/      # Slash command system
│   │   │   ├── mod.rs
│   │   │   ├── registry.rs
│   │   │   ├── parser.rs
│   │   │   ├── executor.rs
│   │   │   ├── builtin/         # Built-in commands from codex_plus
│   │   │   │   ├── mod.rs
│   │   │   │   ├── help.rs
│   │   │   │   └── ...
│   │   │   └── tests/
│   │   ├── hooks/               # Hook system
│   │   │   ├── mod.rs
│   │   │   ├── manager.rs
│   │   │   ├── executor.rs
│   │   │   ├── hook_points.rs
│   │   │   ├── builtin/         # Built-in hooks from codex_plus
│   │   │   │   ├── mod.rs
│   │   │   │   └── ...
│   │   │   └── tests/
│   │   └── utils/               # Shared utilities from codex_plus
│   │       ├── mod.rs
│   │       └── ...
│   ├── cli/                     # MODIFIED: Add extension initialization
│   ├── core/                    # UNCHANGED: OpenAI Codex core
│   └── ...                      # UNCHANGED: Other OpenAI Codex components
├── docs/
│   ├── extensions/              # NEW: Extension documentation
│   │   ├── slash-commands.md
│   │   ├── hooks.md
│   │   └── creating-extensions.md
│   └── ...                      # EXISTING: OpenAI Codex docs
├── examples/
│   ├── slash-commands/          # NEW: Example slash commands
│   └── hooks/                   # NEW: Example hooks
├── SPEC.md                      # NEW: This specification
├── DESIGN.md                    # NEW: This design document
├── IMPLEMENTATION-PLAN.md       # NEW: Implementation plan
└── README.md                    # MODIFIED: Updated with fork info
```

## Data Flow

### Slash Command Execution Flow

```
User Input → CLI Parser → Slash Command Detector → Command Registry
                                                          ↓
                                                    Command Executor
                                                          ↓
                                                    Execute Logic
                                                          ↓
                                                    Return Output
                                                          ↓
                                                    Display to User
```

### Hook Execution Flow

```
Codex Event → Hook Manager → Find Registered Hooks → Execute Hooks
                                                           ↓
                                                      Shell/Binary
                                                           ↓
                                                    Collect Outputs
                                                           ↓
                                                    Process Results
                                                           ↓
                                                  Continue Execution
```

## Configuration Management

### Merged Configuration Strategy

1. **Base Configuration**: OpenAI Codex `config.toml` structure preserved
2. **Extension Configuration**: Add new sections for extensions
3. **Precedence**: OpenAI Codex settings take precedence for any conflicts

### Example Merged Configuration

```toml
# ~/.codex/config.toml

# OpenAI Codex Core Settings (unchanged)
[general]
model = "gpt-4"
# ... other core settings ...

[mcp_servers]
# ... MCP server configs ...

# codex_plus Extensions (new)
[extensions]
enabled = true

[extensions.slash_commands]
enabled = true
custom_commands_dir = "~/.codex/commands"
prefix = "/"

[extensions.hooks]
enabled = true
hooks_dir = "~/.codex/hooks"
timeout_ms = 5000

[extensions.hooks.session_start]
enabled = true
scripts = ["init.sh"]

[extensions.hooks.pre_execution]
enabled = true
scripts = []

[extensions.hooks.post_execution]
enabled = true
scripts = ["cleanup.sh"]
```

## Dependency Management

### Rust Dependencies (Cargo.toml)

```toml
[workspace]
members = [
    # ... existing OpenAI Codex members ...
    "codex-rs/extensions",
    "codex-rs/extensions/slash-commands",
    "codex-rs/extensions/hooks",
]

[workspace.dependencies]
# ... existing OpenAI Codex dependencies ...

# New dependencies for extensions
shellexpand = "3.1"
```

### JavaScript Dependencies (package.json)

```json
{
  "dependencies": {
    // ... existing OpenAI Codex dependencies ...
    // Additional codex_plus dependencies (if any)
  },
  "devDependencies": {
    // ... merged dev dependencies ...
  }
}
```

**Conflict Resolution**: OpenAI Codex versions take precedence

## Testing Strategy

### Test Organization

```
codex-rs/
├── extensions/
│   ├── slash-commands/
│   │   └── tests/
│   │       ├── integration_tests.rs
│   │       ├── command_parser_tests.rs
│   │       └── command_executor_tests.rs
│   └── hooks/
│       └── tests/
│           ├── integration_tests.rs
│           ├── hook_manager_tests.rs
│           └── hook_executor_tests.rs
└── ... (existing OpenAI Codex tests)
```

### Test Categories

1. **Core Tests**: Existing OpenAI Codex tests (must all pass)
2. **Extension Unit Tests**: Test individual extension components
3. **Extension Integration Tests**: Test extension integration with core
4. **End-to-End Tests**: Test complete workflows with extensions enabled

### Test Execution

```bash
# Run all tests
cargo test

# Run only core tests (exclude extensions)
cargo test --workspace --exclude codex-extensions

# Run only extension tests
cargo test -p codex-extensions

# Run specific extension tests
cargo test -p codex-slash-commands
cargo test -p codex-hooks
```

## Security Considerations

### Slash Commands
- Validate all user input
- Sanitize command arguments
- Restrict file system access
- Implement command allowlists/denylists

### Hooks
- Execute hooks in sandboxed environment
- Implement timeout mechanisms
- Validate hook scripts before execution
- Provide security audit logs

### General
- No sensitive data in logs
- Secure configuration file permissions
- Validate all extension code
- Regular security audits

## Performance Considerations

### Lazy Loading
- Load extensions only when needed
- Cache compiled extensions
- Minimize startup time impact

### Async Execution
- Execute hooks asynchronously where possible
- Use parallel execution for independent hooks
- Implement proper timeout handling

### Resource Management
- Limit hook execution time
- Monitor memory usage
- Implement proper cleanup

## Backward Compatibility

### With OpenAI Codex
- All existing functionality must work unchanged
- Existing configurations must be compatible
- API surface remains stable
- Users can disable extensions entirely

### With codex_plus
- Adapt features to fit new architecture
- May require API changes from original codex_plus
- Focus on functionality preservation over API preservation

## Migration Path

### For OpenAI Codex Users
1. Install codex_fork (same as OpenAI Codex)
2. Optionally enable extensions in config
3. No breaking changes to existing workflows

### For codex_plus Users
1. Install codex_fork
2. Configure extensions (slash commands, hooks)
3. Migrate custom commands/hooks to new format
4. Review and update configurations

## Future Extensibility

### Plugin System
- Design allows for third-party plugins
- Clear plugin API and interfaces
- Plugin marketplace potential

### Additional Extensions
- Easy to add new extension types
- Follows established patterns
- Well-documented extension API

## Maintenance Strategy

### Upstream Sync
- Regular pulls from OpenAI Codex upstream
- Automated testing on upstream changes
- Clear separation allows easier merges

### Version Management
- Track OpenAI Codex version
- Track codex_plus features version
- Semantic versioning for fork

## Documentation Requirements

### User Documentation
- Installation guide
- Slash command reference
- Hook system guide
- Configuration reference
- Migration guides

### Developer Documentation
- Architecture overview
- Extension development guide
- API reference
- Contributing guidelines
- Testing guide

## Success Metrics

1. **Functionality**: All OpenAI Codex features work + all codex_plus features adapted
2. **Tests**: 100% of existing tests pass + new tests for extensions pass
3. **Performance**: No significant performance degradation (<5% overhead)
4. **Code Quality**: Passes all linters and quality checks
5. **Documentation**: Comprehensive docs for all new features

## Appendices

### Appendix A: Technology Stack
- **Language**: Rust (codex-rs), TypeScript (codex-cli)
- **Build Tools**: Cargo, pnpm
- **Testing**: Rust testing framework, Jest/Vitest
- **Configuration**: TOML format

### Appendix B: Glossary
- **OpenAI Codex**: Upstream open-source project
- **codex_plus**: Private repository with enhancements
- **codex_fork**: This integrated fork repository
- **Slash Commands**: User commands prefixed with `/`
- **Hooks**: Event-driven extension points

### Appendix C: References
- OpenAI Codex: https://github.com/openai/codex
- Apache-2.0 License: https://www.apache.org/licenses/LICENSE-2.0
