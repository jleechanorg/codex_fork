//! Integration tests for the extension system
//!
//! These tests verify the complete extension workflow including:
//! - Loading settings, commands, and hooks together
//! - End-to-end slash command detection and execution
//! - Hook execution in realistic scenarios

use codex_extensions::{HookSystem, Settings, SlashCommandRegistry};
use std::io::Write;
use tempfile::TempDir;

#[test]
fn test_full_extension_system_integration() {
    // Create a temporary project directory
    let project = TempDir::new().unwrap();

    // Setup directory structure
    let claude_dir = project.path().join(".claude");
    std::fs::create_dir(&claude_dir).unwrap();
    std::fs::create_dir(claude_dir.join("commands")).unwrap();
    std::fs::create_dir(claude_dir.join("hooks")).unwrap();

    // Create a slash command
    let hello_cmd = claude_dir.join("commands/hello.md");
    std::fs::write(
        &hello_cmd,
        r#"---
name: hello
description: Greeting command
---

# Hello Command

Greet the user: $ARGUMENTS
"#,
    )
    .unwrap();

    // Create a hook script
    let hook_script = claude_dir.join("hooks/test_hook.sh");
    std::fs::write(
        &hook_script,
        r#"#!/bin/bash
cat | jq -c '.session_id'
"#,
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_script, perms).unwrap();
    }

    // Create settings
    let settings_file = claude_dir.join("settings.json");
    std::fs::write(
        &settings_file,
        r#"{
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "test_hook.sh",
                                "timeout": 5
                            }
                        ]
                    }
                ]
            }
        }"#,
    )
    .unwrap();

    // Test loading everything
    let settings = Settings::load(Some(project.path())).unwrap();
    let registry = SlashCommandRegistry::load(Some(project.path())).unwrap();
    let _hook_system = HookSystem::new(Some(project.path())).unwrap();

    // Verify settings loaded
    assert!(settings.hooks.contains_key("UserPromptSubmit"));

    // Verify command loaded
    let cmd = registry.get("hello").unwrap();
    assert_eq!(cmd.metadata.name, "hello");
    assert!(cmd.content.contains("Greet the user"));

    // Test command detection
    let (name, args) = SlashCommandRegistry::detect_command("/hello World").unwrap();
    assert_eq!(name, "hello");
    assert_eq!(args, "World");

    // Test argument substitution
    let substituted = cmd.substitute_arguments("World");
    assert!(substituted.contains("World"));
}

#[tokio::test]
async fn test_hook_execution_integration() {
    let project = TempDir::new().unwrap();
    let hooks_dir = project.path().join(".claude/hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();

    // Create a hook that outputs JSON
    let hook_script = hooks_dir.join("context_hook.sh");
    std::fs::write(
        &hook_script,
        r#"#!/bin/bash
# Read input and add context
input=$(cat)
echo '{"hookSpecificOutput": {"hookEventName": "UserPromptSubmit", "additionalContext": "Test context"}}'
exit 0
"#,
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_script, perms).unwrap();
    }

    let settings_file = project.path().join(".claude/settings.json");
    std::fs::write(
        &settings_file,
        r#"{
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "context_hook.sh",
                                "timeout": 5
                            }
                        ]
                    }
                ]
            }
        }"#,
    )
    .unwrap();

    let hook_system = HookSystem::new(Some(project.path())).unwrap();

    let input = codex_extensions::hooks::HookInput {
        session_id: "test-session".to_string(),
        transcript_path: "".to_string(),
        cwd: project.path().to_string_lossy().to_string(),
        hook_event_name: "UserPromptSubmit".to_string(),
        extra: Default::default(),
    };

    let results = hook_system
        .execute(codex_extensions::hooks::HookEvent::UserPromptSubmit, input)
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].parsed_output.is_some());
}

#[tokio::test]
async fn test_hook_blocking_integration() {
    let project = TempDir::new().unwrap();
    let hooks_dir = project.path().join(".claude/hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();

    // Create a hook that blocks with exit code 2
    let hook_script = hooks_dir.join("block_hook.sh");
    std::fs::write(
        &hook_script,
        r#"#!/bin/bash
echo "Access denied" >&2
exit 2
"#,
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_script, perms).unwrap();
    }

    let settings_file = project.path().join(".claude/settings.json");
    std::fs::write(
        &settings_file,
        r#"{
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "*",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "block_hook.sh",
                                "timeout": 5
                            }
                        ]
                    }
                ]
            }
        }"#,
    )
    .unwrap();

    let hook_system = HookSystem::new(Some(project.path())).unwrap();

    let input = codex_extensions::hooks::HookInput {
        session_id: "test-session".to_string(),
        transcript_path: "".to_string(),
        cwd: project.path().to_string_lossy().to_string(),
        hook_event_name: "PreToolUse".to_string(),
        extra: Default::default(),
    };

    let results = hook_system
        .execute(codex_extensions::hooks::HookEvent::PreToolUse, input)
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].is_blocking());
    assert_eq!(results[0].block_reason(), Some("Access denied".to_string()));
}

#[test]
fn test_command_and_settings_precedence() {
    let project = TempDir::new().unwrap();

    // Create .claude directory with command
    let claude_dir = project.path().join(".claude");
    std::fs::create_dir_all(claude_dir.join("commands")).unwrap();
    std::fs::write(
        claude_dir.join("commands/test.md"),
        r#"---
name: test
description: Claude version
---
Claude version content
"#,
    )
    .unwrap();

    // Create .codexplus directory with higher precedence
    let codexplus_dir = project.path().join(".codexplus");
    std::fs::create_dir_all(codexplus_dir.join("commands")).unwrap();
    std::fs::write(
        codexplus_dir.join("commands/test.md"),
        r#"---
name: test
description: Codexplus version
---
Codexplus version content
"#,
    )
    .unwrap();

    let registry = SlashCommandRegistry::load(Some(project.path())).unwrap();
    let cmd = registry.get("test").unwrap();

    // Should get .codexplus version (higher precedence)
    assert!(cmd.content.contains("Codexplus version"));
}

#[test]
fn test_multiple_commands_loading() {
    let project = TempDir::new().unwrap();
    let commands_dir = project.path().join(".claude/commands");
    std::fs::create_dir_all(&commands_dir).unwrap();

    // Create multiple command files
    let commands = vec![
        ("hello", "Greeting"),
        ("echo", "Echo test"),
        ("help", "Show help"),
    ];

    for (name, desc) in commands {
        std::fs::write(
            commands_dir.join(format!("{}.md", name)),
            format!(
                r#"---
name: {}
description: {}
---
Command content for {}
"#,
                name, desc, name
            ),
        )
        .unwrap();
    }

    let registry = SlashCommandRegistry::load(Some(project.path())).unwrap();

    // Verify all commands loaded
    assert!(registry.get("hello").is_some());
    assert!(registry.get("echo").is_some());
    assert!(registry.get("help").is_some());

    // Verify count
    assert_eq!(registry.list().len(), 3);
}

#[tokio::test]
async fn test_multiple_hooks_execution() {
    let project = TempDir::new().unwrap();
    let hooks_dir = project.path().join(".claude/hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();

    // Create first hook
    let hook1 = hooks_dir.join("hook1.sh");
    std::fs::write(
        &hook1,
        r#"#!/bin/bash
echo '{"decision": "allow"}'
"#,
    )
    .unwrap();

    // Create second hook
    let hook2 = hooks_dir.join("hook2.sh");
    std::fs::write(
        &hook2,
        r#"#!/bin/bash
echo '{"decision": "allow"}'
"#,
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for script in [&hook1, &hook2] {
            let mut perms = std::fs::metadata(script).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(script, perms).unwrap();
        }
    }

    let settings_file = project.path().join(".claude/settings.json");
    std::fs::write(
        &settings_file,
        r#"{
            "hooks": {
                "SessionStart": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "hook1.sh",
                                "timeout": 5
                            },
                            {
                                "type": "command",
                                "command": "hook2.sh",
                                "timeout": 5
                            }
                        ]
                    }
                ]
            }
        }"#,
    )
    .unwrap();

    let hook_system = HookSystem::new(Some(project.path())).unwrap();

    let input = codex_extensions::hooks::HookInput {
        session_id: "test-session".to_string(),
        transcript_path: "".to_string(),
        cwd: project.path().to_string_lossy().to_string(),
        hook_event_name: "SessionStart".to_string(),
        extra: Default::default(),
    };

    let results = hook_system
        .execute(codex_extensions::hooks::HookEvent::SessionStart, input)
        .await
        .unwrap();

    // Both hooks should execute
    assert_eq!(results.len(), 2);
}

#[test]
fn test_invalid_command_handling() {
    let project = TempDir::new().unwrap();
    let commands_dir = project.path().join(".claude/commands");
    std::fs::create_dir_all(&commands_dir).unwrap();

    // Create invalid command (no name in frontmatter)
    std::fs::write(
        commands_dir.join("invalid.md"),
        r#"---
description: Missing name field
---
Content
"#,
    )
    .unwrap();

    // Create valid command
    std::fs::write(
        commands_dir.join("valid.md"),
        r#"---
name: valid
description: Valid command
---
Content
"#,
    )
    .unwrap();

    let registry = SlashCommandRegistry::load(Some(project.path())).unwrap();

    // Should load valid command and skip invalid
    assert!(registry.get("valid").is_some());
    assert_eq!(registry.list().len(), 1);
}
