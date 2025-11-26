//! Settings management for extensions
//!
//! Parses and manages settings.json configuration files with precedence:
//! 1. .codexplus/settings.json (highest priority)
//! 2. .claude/settings.json (project level)
//! 3. ~/.claude/settings.json (user level)

use crate::error::ExtensionError;
use crate::error::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// Hook configuration from settings.json
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HookConfig {
    #[serde(rename = "type")]
    pub hook_type: String,
    pub command: String,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_timeout() -> u64 {
    5
}

/// Hook entry with optional matcher
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HookEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    pub hooks: Vec<HookConfig>,
}

/// Status line configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatusLineConfig {
    #[serde(rename = "type")]
    pub status_type: String,
    pub command: String,
    #[serde(default = "default_status_timeout")]
    pub timeout: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

fn default_status_timeout() -> u64 {
    2
}

/// Complete settings structure
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Settings {
    #[serde(default)]
    pub hooks: HashMap<String, Vec<HookEntry>>,
    #[serde(rename = "statusLine", skip_serializing_if = "Option::is_none")]
    pub status_line: Option<StatusLineConfig>,
}

impl Settings {
    /// Load settings with precedence from multiple locations
    pub fn load(project_dir: Option<&Path>) -> Result<Self> {
        let mut merged = Settings::default();

        // Determine base directory (project dir or current dir)
        let base_dir = project_dir.unwrap_or_else(|| Path::new("."));

        // Load in reverse precedence order (later overwrites earlier)
        let settings_paths = [
            dirs::home_dir().map(|h| h.join(".claude/settings.json")),
            Some(base_dir.join(".claude/settings.json")),
            Some(base_dir.join(".codexplus/settings.json")),
        ];

        for path_opt in settings_paths.iter().flatten() {
            if path_opt.exists() {
                let settings = Self::load_from_file(path_opt)?;
                merged.merge(settings);
            }
        }

        Ok(merged)
    }

    /// Load settings from a single file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| ExtensionError::SettingsError {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;

        serde_json::from_str(&content).map_err(|e| ExtensionError::SettingsError {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })
    }

    /// Merge another settings instance into this one.
    ///
    /// # Merge Semantics
    ///
    /// Settings are loaded with precedence: `.codexplus/` > `.claude/` > `~/.claude/`.
    /// The merge behavior differs by field type:
    ///
    /// - **Hooks**: Concatenated from all sources, executing in precedence order.
    ///   This allows higher-priority directories to add hooks without replacing
    ///   hooks from lower-priority directories. All hooks from all sources will execute.
    ///
    /// - **Status line**: Replaced by higher-priority sources. Only the highest-priority
    ///   status line configuration is used.
    ///
    /// # Example
    ///
    /// If `~/.claude/settings.json` defines a `SessionStart` hook and
    /// `.claude/settings.json` also defines a `SessionStart` hook, both hooks
    /// will execute (`.claude/` first, then `~/.claude/`).
    fn merge(&mut self, other: Settings) {
        // Merge hooks - concatenate arrays for same event
        // This means hooks from ALL sources execute, in precedence order
        for (event, entries) in other.hooks {
            self.hooks.entry(event).or_default().extend(entries);
        }

        // Status line: replace if present (higher precedence wins)
        if other.status_line.is_some() {
            self.status_line = other.status_line;
        }
    }

    /// Get hooks for a specific event
    pub fn get_hooks(&self, event: &str) -> Option<&Vec<HookEntry>> {
        self.hooks.get(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_simple_settings() {
        let json = r#"{
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
            }
        }"#;

        let settings: Settings = serde_json::from_str(json).unwrap();
        assert!(settings.hooks.contains_key("UserPromptSubmit"));
        let entries = &settings.hooks["UserPromptSubmit"];
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].matcher, Some("*".to_string()));
        assert_eq!(entries[0].hooks.len(), 1);
        assert_eq!(entries[0].hooks[0].command, "add_context.py");
        assert_eq!(entries[0].hooks[0].timeout, 5);
    }

    #[test]
    fn test_parse_status_line() {
        let json = r#"{
            "statusLine": {
                "type": "command",
                "command": "git_status.sh",
                "timeout": 2,
                "mode": "prepend"
            }
        }"#;

        let settings: Settings = serde_json::from_str(json).unwrap();
        assert!(settings.status_line.is_some());
        let status_line = settings.status_line.unwrap();
        assert_eq!(status_line.command, "git_status.sh");
        assert_eq!(status_line.timeout, 2);
        assert_eq!(status_line.mode, Some("prepend".to_string()));
    }

    #[test]
    fn test_default_timeout() {
        let json = r#"{
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "test.sh"
                            }
                        ]
                    }
                ]
            }
        }"#;

        let settings: Settings = serde_json::from_str(json).unwrap();
        let entries = &settings.hooks["UserPromptSubmit"];
        assert_eq!(entries[0].hooks[0].timeout, 5); // Default timeout
    }

    #[test]
    fn test_load_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let settings_path = temp_dir.path().join("settings.json");

        let content = r#"{
            "hooks": {
                "SessionStart": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "init.sh",
                                "timeout": 3
                            }
                        ]
                    }
                ]
            }
        }"#;

        std::fs::write(&settings_path, content).unwrap();

        let settings = Settings::load_from_file(&settings_path).unwrap();
        assert!(settings.hooks.contains_key("SessionStart"));
    }

    #[test]
    fn test_merge_settings() {
        let mut base = Settings::default();
        base.hooks.insert(
            "UserPromptSubmit".to_string(),
            vec![HookEntry {
                matcher: Some("*".to_string()),
                hooks: vec![HookConfig {
                    hook_type: "command".to_string(),
                    command: "hook1.sh".to_string(),
                    timeout: 5,
                }],
            }],
        );

        let override_settings = Settings {
            hooks: HashMap::from([(
                "SessionStart".to_string(),
                vec![HookEntry {
                    matcher: None,
                    hooks: vec![HookConfig {
                        hook_type: "command".to_string(),
                        command: "hook2.sh".to_string(),
                        timeout: 3,
                    }],
                }],
            )]),
            status_line: Some(StatusLineConfig {
                status_type: "command".to_string(),
                command: "status.sh".to_string(),
                timeout: 2,
                mode: None,
            }),
        };

        base.merge(override_settings);

        assert_eq!(base.hooks.len(), 2);
        assert!(base.hooks.contains_key("UserPromptSubmit"));
        assert!(base.hooks.contains_key("SessionStart"));
        assert!(base.status_line.is_some());
    }

    #[test]
    fn test_load_with_precedence() {
        let temp_dir = TempDir::new().unwrap();

        // Create .claude directory
        let claude_dir = temp_dir.path().join(".claude");
        std::fs::create_dir(&claude_dir).unwrap();

        // Create .claude/settings.json
        let claude_settings = claude_dir.join("settings.json");
        let content1 = r#"{
            "hooks": {
                "UserPromptSubmit": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "claude_hook.sh",
                                "timeout": 5
                            }
                        ]
                    }
                ]
            }
        }"#;
        std::fs::write(&claude_settings, content1).unwrap();

        // Create .codexplus directory
        let codexplus_dir = temp_dir.path().join(".codexplus");
        std::fs::create_dir(&codexplus_dir).unwrap();

        // Create .codexplus/settings.json (higher precedence)
        let codexplus_settings = codexplus_dir.join("settings.json");
        let content2 = r#"{
            "statusLine": {
                "type": "command",
                "command": "codexplus_status.sh",
                "timeout": 2
            }
        }"#;
        std::fs::write(&codexplus_settings, content2).unwrap();

        let settings = Settings::load(Some(temp_dir.path())).unwrap();

        // Should have hooks from .claude and statusLine from .codexplus
        assert!(settings.hooks.contains_key("UserPromptSubmit"));
        assert!(settings.status_line.is_some());
        assert_eq!(settings.status_line.unwrap().command, "codexplus_status.sh");
    }

    #[test]
    fn test_get_hooks() {
        let mut settings = Settings::default();
        settings.hooks.insert(
            "PreToolUse".to_string(),
            vec![HookEntry {
                matcher: Some("Bash".to_string()),
                hooks: vec![HookConfig {
                    hook_type: "command".to_string(),
                    command: "validate.sh".to_string(),
                    timeout: 5,
                }],
            }],
        );

        let hooks = settings.get_hooks("PreToolUse");
        assert!(hooks.is_some());
        assert_eq!(hooks.unwrap().len(), 1);

        let no_hooks = settings.get_hooks("NonExistent");
        assert!(no_hooks.is_none());
    }
}
