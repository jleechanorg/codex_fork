//! Slash command system
//!
//! Parses and manages slash commands from markdown files with YAML frontmatter.
//! Commands are stored in .claude/commands/ or .codexplus/commands/ directories.

use crate::error::{ExtensionError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Command metadata from YAML frontmatter
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandMetadata {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// A parsed slash command
#[derive(Debug, Clone, PartialEq)]
pub struct SlashCommand {
    pub metadata: CommandMetadata,
    pub content: String,
    pub file_path: PathBuf,
}

impl SlashCommand {
    /// Parse a command from a markdown file with YAML frontmatter
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_string(&content, path)
    }

    /// Parse a command from a string
    fn from_string(content: &str, path: &Path) -> Result<Self> {
        // Split frontmatter and body
        let (frontmatter, body) = Self::extract_frontmatter(content).ok_or_else(|| {
            ExtensionError::InvalidCommandFormat {
                path: path.to_path_buf(),
                reason: "Missing or invalid YAML frontmatter".to_string(),
            }
        })?;

        // Parse frontmatter
        let metadata: CommandMetadata =
            serde_yaml::from_str(&frontmatter).map_err(|e| ExtensionError::InvalidCommandFormat {
                path: path.to_path_buf(),
                reason: format!("Invalid YAML frontmatter: {}", e),
            })?;

        Ok(SlashCommand {
            metadata,
            content: body.to_string(),
            file_path: path.to_path_buf(),
        })
    }

    /// Extract YAML frontmatter from markdown content
    /// Returns (frontmatter, remaining_content)
    fn extract_frontmatter(content: &str) -> Option<(String, String)> {
        let lines: Vec<&str> = content.lines().collect();

        if lines.is_empty() || lines[0].trim() != "---" {
            return None;
        }

        // Find the closing ---
        let end_index = lines.iter().skip(1).position(|line| line.trim() == "---")?;

        let frontmatter = lines[1..=end_index].join("\n");
        let body = lines[end_index + 2..].join("\n");

        Some((frontmatter, body))
    }

    /// Substitute arguments into the command content
    pub fn substitute_arguments(&self, args: &str) -> String {
        self.content.replace("$ARGUMENTS", args)
    }
}

/// Registry for managing slash commands
#[derive(Debug, Default)]
pub struct SlashCommandRegistry {
    commands: HashMap<String, SlashCommand>,
}

impl SlashCommandRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Load commands from standard directories
    /// Searches .codexplus/commands/ then .claude/commands/
    pub fn load(project_dir: Option<&Path>) -> Result<Self> {
        let mut registry = Self::new();
        let base_dir = project_dir.unwrap_or_else(|| Path::new("."));

        let command_dirs = [
            base_dir.join(".codexplus/commands"),
            base_dir.join(".claude/commands"),
        ];

        for dir in &command_dirs {
            if dir.exists() && dir.is_dir() {
                registry.load_from_directory(dir)?;
            }
        }

        Ok(registry)
    }

    /// Load all commands from a directory
    pub fn load_from_directory(&mut self, dir: &Path) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                match SlashCommand::from_file(&path) {
                    Ok(cmd) => {
                        // Only add if not already present (precedence)
                        self.commands
                            .entry(cmd.metadata.name.clone())
                            .or_insert(cmd);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load command from {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Register a command
    pub fn register(&mut self, command: SlashCommand) {
        self.commands
            .insert(command.metadata.name.clone(), command);
    }

    /// Get a command by name
    pub fn get(&self, name: &str) -> Option<&SlashCommand> {
        self.commands.get(name)
    }

    /// List all available commands
    pub fn list(&self) -> Vec<&SlashCommand> {
        self.commands.values().collect()
    }

    /// Detect if input contains a slash command
    /// Returns (command_name, arguments) if found
    pub fn detect_command(input: &str) -> Option<(String, String)> {
        let trimmed = input.trim();

        if !trimmed.starts_with('/') {
            return None;
        }

        // Split on first whitespace
        let parts: Vec<&str> = trimmed[1..].splitn(2, char::is_whitespace).collect();

        let command_name = parts[0].to_string();
        let args = parts.get(1).unwrap_or(&"").trim().to_string();

        Some((command_name, args))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_extract_frontmatter() {
        let content = r#"---
name: hello
description: A hello command
---

# Hello Command

This is the command body.
"#;

        let (frontmatter, body) = SlashCommand::extract_frontmatter(content).unwrap();
        assert!(frontmatter.contains("name: hello"));
        assert!(body.contains("# Hello Command"));
    }

    #[test]
    fn test_parse_command_from_string() {
        let content = r#"---
name: hello
description: Simple test command
---

# Hello Command

When this command is executed:

1. Greet the user
2. Show the current date

Arguments: $ARGUMENTS
"#;

        let cmd = SlashCommand::from_string(content, Path::new("test.md")).unwrap();
        assert_eq!(cmd.metadata.name, "hello");
        assert_eq!(cmd.metadata.description, "Simple test command");
        assert!(cmd.content.contains("Greet the user"));
        assert!(cmd.content.contains("$ARGUMENTS"));
    }

    #[test]
    fn test_substitute_arguments() {
        let content = r#"---
name: greet
description: Greet someone
---

Hello $ARGUMENTS!
"#;

        let cmd = SlashCommand::from_string(content, Path::new("test.md")).unwrap();
        let substituted = cmd.substitute_arguments("World");
        assert_eq!(substituted.trim(), "Hello World!");
    }

    #[test]
    fn test_detect_command() {
        // Basic command
        let (name, args) = SlashCommandRegistry::detect_command("/hello").unwrap();
        assert_eq!(name, "hello");
        assert_eq!(args, "");

        // Command with args
        let (name, args) = SlashCommandRegistry::detect_command("/greet World").unwrap();
        assert_eq!(name, "greet");
        assert_eq!(args, "World");

        // Command with multiple args
        let (name, args) = SlashCommandRegistry::detect_command("/greet Hello World").unwrap();
        assert_eq!(name, "greet");
        assert_eq!(args, "Hello World");

        // Not a command
        assert!(SlashCommandRegistry::detect_command("hello").is_none());
        assert!(SlashCommandRegistry::detect_command("regular text").is_none());
    }

    #[test]
    fn test_load_from_directory() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        std::fs::create_dir(&commands_dir).unwrap();

        // Create a command file
        let cmd_file = commands_dir.join("hello.md");
        let content = r#"---
name: hello
description: Hello command
---

# Hello

Greet the user.
"#;
        std::fs::write(&cmd_file, content).unwrap();

        let mut registry = SlashCommandRegistry::new();
        registry.load_from_directory(&commands_dir).unwrap();

        assert_eq!(registry.commands.len(), 1);
        assert!(registry.get("hello").is_some());
    }

    #[test]
    fn test_load_with_precedence() {
        let temp_dir = TempDir::new().unwrap();

        // Create .claude/commands directory
        let claude_dir = temp_dir.path().join(".claude/commands");
        std::fs::create_dir_all(&claude_dir).unwrap();

        let claude_hello = claude_dir.join("hello.md");
        std::fs::write(
            &claude_hello,
            r#"---
name: hello
description: Claude hello
---

Claude version
"#,
        )
        .unwrap();

        // Create .codexplus/commands directory
        let codexplus_dir = temp_dir.path().join(".codexplus/commands");
        std::fs::create_dir_all(&codexplus_dir).unwrap();

        let codexplus_hello = codexplus_dir.join("hello.md");
        std::fs::write(
            &codexplus_hello,
            r#"---
name: hello
description: Codexplus hello
---

Codexplus version
"#,
        )
        .unwrap();

        let registry = SlashCommandRegistry::load(Some(temp_dir.path())).unwrap();

        // Should have codexplus version (higher precedence)
        let cmd = registry.get("hello").unwrap();
        assert!(cmd.content.contains("Codexplus version"));
    }

    #[test]
    fn test_registry_operations() {
        let mut registry = SlashCommandRegistry::new();

        let cmd = SlashCommand {
            metadata: CommandMetadata {
                name: "test".to_string(),
                description: "Test command".to_string(),
            },
            content: "Test content".to_string(),
            file_path: PathBuf::from("test.md"),
        };

        registry.register(cmd);

        assert_eq!(registry.list().len(), 1);
        assert!(registry.get("test").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_invalid_frontmatter() {
        let content = r#"---
invalid yaml: [unclosed
---

Body
"#;

        let result = SlashCommand::from_string(content, Path::new("test.md"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ExtensionError::InvalidCommandFormat { .. }
        ));
    }

    #[test]
    fn test_missing_frontmatter() {
        let content = "# No frontmatter\n\nJust body";

        let result = SlashCommand::from_string(content, Path::new("test.md"));
        assert!(result.is_err());
    }
}
