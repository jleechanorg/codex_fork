//! Hook system for lifecycle events
//!
//! Executes hook scripts at specific points in the Codex CLI lifecycle.
//! Hooks communicate via JSON stdin/stdout protocol.

use crate::error::{ExtensionError, Result};
use crate::settings::Settings;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::Duration;

/// Hook events supported by the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookEvent {
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    Notification,
    Stop,
    PreCompact,
    SessionStart,
    SessionEnd,
}

impl HookEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            HookEvent::UserPromptSubmit => "UserPromptSubmit",
            HookEvent::PreToolUse => "PreToolUse",
            HookEvent::PostToolUse => "PostToolUse",
            HookEvent::Notification => "Notification",
            HookEvent::Stop => "Stop",
            HookEvent::PreCompact => "PreCompact",
            HookEvent::SessionStart => "SessionStart",
            HookEvent::SessionEnd => "SessionEnd",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "UserPromptSubmit" => Some(HookEvent::UserPromptSubmit),
            "PreToolUse" => Some(HookEvent::PreToolUse),
            "PostToolUse" => Some(HookEvent::PostToolUse),
            "Notification" => Some(HookEvent::Notification),
            "Stop" => Some(HookEvent::Stop),
            "PreCompact" => Some(HookEvent::PreCompact),
            "SessionStart" => Some(HookEvent::SessionStart),
            "SessionEnd" => Some(HookEvent::SessionEnd),
            _ => None,
        }
    }
}

/// Input payload sent to hook scripts
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookInput {
    pub session_id: String,
    pub transcript_path: String,
    pub cwd: String,
    pub hook_event_name: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Output from hook scripts
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook_specific_output: Option<serde_json::Value>,
}

/// Result of hook execution
#[derive(Debug, Clone)]
pub struct HookResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub parsed_output: Option<HookOutput>,
}

impl HookResult {
    /// Check if the hook requested to block the action
    pub fn is_blocking(&self) -> bool {
        self.exit_code == 2
            || self
                .parsed_output
                .as_ref()
                .and_then(|o| o.decision.as_deref())
                .map(|d| d == "block" || d == "deny")
                .unwrap_or(false)
    }

    /// Get the block reason if blocking
    pub fn block_reason(&self) -> Option<String> {
        if self.exit_code == 2 {
            let trimmed = self.stderr.trim();
            Some(
                trimmed
                    .split_once('\n')
                    .map(|(first, _)| first.to_string())
                    .unwrap_or_else(|| trimmed.to_string()),
            )
        } else {
            self.parsed_output.as_ref().and_then(|o| o.reason.clone())
        }
    }
}

/// Hook system manager
#[derive(Debug)]
pub struct HookSystem {
    settings: Settings,
    session_id: String,
    project_dir: PathBuf,
}

impl HookSystem {
    /// Create a new hook system
    pub fn new(project_dir: Option<&Path>) -> Result<Self> {
        let project_dir = project_dir
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let settings = Settings::load(Some(&project_dir))?;

        let session_id = std::env::var("CODEX_SESSION_ID")
            .unwrap_or_else(|_| format!("session-{}", std::process::id()));

        Ok(Self {
            settings,
            session_id,
            project_dir,
        })
    }

    /// Execute hooks for a specific event
    pub async fn execute(&self, event: HookEvent, input: HookInput) -> Result<Vec<HookResult>> {
        let hooks = match self.settings.get_hooks(event.as_str()) {
            Some(h) => h,
            None => return Ok(Vec::new()),
        };

        let mut results = Vec::new();
        let mut blocked = false;

        for entry in hooks {
            if blocked {
                break;
            }

            for hook_config in &entry.hooks {
                if hook_config.hook_type != "command" {
                    continue;
                }

                let result = self
                    .execute_command_hook(&hook_config.command, &input, hook_config.timeout)
                    .await?;

                results.push(result.clone());

                // Stop on first blocking hook
                if result.is_blocking() {
                    blocked = true;
                    break;
                }
            }
        }

        Ok(results)
    }

    /// Execute a single command hook
    async fn execute_command_hook(
        &self,
        command: &str,
        input: &HookInput,
        timeout_secs: u64,
    ) -> Result<HookResult> {
        // Resolve command path
        let cmd_path = self.resolve_command_path(command)?;

        // Determine how to execute (interpreter vs direct)
        let (executable, args) = self.determine_executable(&cmd_path);

        // Prepare input JSON
        let input_json = serde_json::to_string(input)?;

        // Spawn process
        let mut child = Command::new(&executable)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(&self.project_dir)
            .env("CLAUDE_PROJECT_DIR", &self.project_dir)
            .spawn()
            .map_err(|e| ExtensionError::HookExecutionFailed(format!("Failed to spawn: {}", e)))?;

        // Write input to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input_json.as_bytes()).await.map_err(|e| {
                ExtensionError::HookExecutionFailed(format!("Failed to write stdin: {}", e))
            })?;
            stdin.flush().await.ok();
            drop(stdin);
        }

        // Take stdout and stderr before waiting so we can kill the child if needed
        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();

        // Wait with timeout and kill the process if it times out
        let timeout = Duration::from_secs(timeout_secs);
        let status = tokio::select! {
            result = child.wait() => {
                result.map_err(|e| ExtensionError::HookExecutionFailed(format!(
                    "Failed to wait for process: {}",
                    e
                )))?
            }
            _ = tokio::time::sleep(timeout) => {
                // Timeout elapsed; kill the hook process to prevent resource leaks
                let _ = child.kill().await;
                return Err(ExtensionError::HookTimeout {
                    timeout_ms: timeout_secs * 1000,
                });
            }
        };

        // Read stdout and stderr
        let stdout = if let Some(mut handle) = stdout_handle {
            let mut buf = Vec::new();
            tokio::io::AsyncReadExt::read_to_end(&mut handle, &mut buf).await.ok();
            String::from_utf8_lossy(&buf).to_string()
        } else {
            String::new()
        };

        let stderr = if let Some(mut handle) = stderr_handle {
            let mut buf = Vec::new();
            tokio::io::AsyncReadExt::read_to_end(&mut handle, &mut buf).await.ok();
            String::from_utf8_lossy(&buf).to_string()
        } else {
            String::new()
        };

        let exit_code = status.code().unwrap_or(1);

        // Try to parse JSON output
        let parsed_output = if !stdout.trim().is_empty() {
            serde_json::from_str(&stdout).ok()
        } else {
            None
        };

        Ok(HookResult {
            exit_code,
            stdout,
            stderr,
            parsed_output,
        })
    }

    /// Resolve command path from hook configuration
    fn resolve_command_path(&self, command: &str) -> Result<PathBuf> {
        // If it's an absolute path, use it directly
        if Path::new(command).is_absolute() {
            return Ok(PathBuf::from(command));
        }

        // If it contains path separators, resolve relative to project dir
        if command.contains(std::path::MAIN_SEPARATOR) {
            return Ok(self.project_dir.join(command));
        }

        // Otherwise, search in hooks directories
        let hook_dirs = [
            self.project_dir.join(".codexplus/hooks"),
            self.project_dir.join(".claude/hooks"),
        ];

        for dir in &hook_dirs {
            let candidate = dir.join(command);
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        // Fall back to command as-is (might be in PATH)
        Ok(PathBuf::from(command))
    }

    /// Determine executable and arguments based on file type
    fn determine_executable(&self, path: &Path) -> (String, Vec<String>) {
        let path_str = path.to_string_lossy().to_string();

        // Check if executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if path.exists() {
                if let Ok(metadata) = std::fs::metadata(path) {
                    let permissions = metadata.permissions();
                    if permissions.mode() & 0o111 != 0 {
                        // Is executable
                        return (path_str, vec![]);
                    }
                }
            }
        }

        // Check extension
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext {
                "py" => return ("python3".to_string(), vec![path_str]),
                "sh" => return ("bash".to_string(), vec![path_str]),
                "js" => return ("node".to_string(), vec![path_str]),
                _ => {}
            }
        }

        // Default: try to execute directly
        (path_str, vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_hook_event_conversion() {
        assert_eq!(HookEvent::UserPromptSubmit.as_str(), "UserPromptSubmit");
        assert_eq!(
            HookEvent::from_str("UserPromptSubmit"),
            Some(HookEvent::UserPromptSubmit)
        );
        assert_eq!(HookEvent::from_str("Invalid"), None);
    }

    #[test]
    fn test_hook_result_blocking() {
        let result = HookResult {
            exit_code: 2,
            stdout: "".to_string(),
            stderr: "Blocked".to_string(),
            parsed_output: None,
        };
        assert!(result.is_blocking());
        assert_eq!(result.block_reason(), Some("Blocked".to_string()));

        let result2 = HookResult {
            exit_code: 0,
            stdout: r#"{"decision": "block", "reason": "Test block"}"#.to_string(),
            stderr: "".to_string(),
            parsed_output: Some(HookOutput {
                decision: Some("block".to_string()),
                reason: Some("Test block".to_string()),
                hook_specific_output: None,
            }),
        };
        assert!(result2.is_blocking());
        assert_eq!(result2.block_reason(), Some("Test block".to_string()));
    }

    #[tokio::test]
    async fn test_execute_simple_hook() {
        let temp_dir = TempDir::new().unwrap();

        // Create a simple echo hook script
        let hooks_dir = temp_dir.path().join(".claude/hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();

        let hook_script = hooks_dir.join("echo.sh");
        let script_content = r#"#!/bin/bash
cat  # Echo stdin to stdout
"#;
        std::fs::write(&hook_script, script_content).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&hook_script).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&hook_script, perms).unwrap();
        }

        // Create settings
        let settings_file = temp_dir.path().join(".claude/settings.json");
        let settings_content = r#"{
            "hooks": {
                "SessionStart": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "echo.sh",
                                "timeout": 5
                            }
                        ]
                    }
                ]
            }
        }"#;
        std::fs::write(&settings_file, settings_content).unwrap();

        let hook_system = HookSystem::new(Some(temp_dir.path())).unwrap();

        let input = HookInput {
            session_id: "test-session".to_string(),
            transcript_path: "".to_string(),
            cwd: temp_dir.path().to_string_lossy().to_string(),
            hook_event_name: "SessionStart".to_string(),
            extra: HashMap::new(),
        };

        let results = hook_system
            .execute(HookEvent::SessionStart, input)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].stdout.contains("test-session"));
    }

    #[tokio::test]
    async fn test_hook_timeout() {
        let temp_dir = TempDir::new().unwrap();

        let hooks_dir = temp_dir.path().join(".claude/hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();

        // Create a hook that sleeps forever
        let hook_script = hooks_dir.join("slow.sh");
        let script_content = r#"#!/bin/bash
sleep 100
"#;
        std::fs::write(&hook_script, script_content).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&hook_script).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&hook_script, perms).unwrap();
        }

        let settings_file = temp_dir.path().join(".claude/settings.json");
        let settings_content = r#"{
            "hooks": {
                "SessionStart": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "slow.sh",
                                "timeout": 1
                            }
                        ]
                    }
                ]
            }
        }"#;
        std::fs::write(&settings_file, settings_content).unwrap();

        let hook_system = HookSystem::new(Some(temp_dir.path())).unwrap();

        let input = HookInput {
            session_id: "test-session".to_string(),
            transcript_path: "".to_string(),
            cwd: temp_dir.path().to_string_lossy().to_string(),
            hook_event_name: "SessionStart".to_string(),
            extra: HashMap::new(),
        };

        let result = hook_system.execute(HookEvent::SessionStart, input).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ExtensionError::HookTimeout { .. }
        ));
    }

    #[test]
    fn test_resolve_command_path() {
        let temp_dir = TempDir::new().unwrap();
        let hooks_dir = temp_dir.path().join(".claude/hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();

        let hook_script = hooks_dir.join("test.sh");
        std::fs::write(&hook_script, "#!/bin/bash\necho test").unwrap();

        let hook_system = HookSystem::new(Some(temp_dir.path())).unwrap();

        // Test bare filename
        let resolved = hook_system.resolve_command_path("test.sh").unwrap();
        assert!(resolved.ends_with("test.sh"));
        assert!(resolved.exists());

        // Test absolute path
        let abs_path = hook_script.to_string_lossy().to_string();
        let resolved = hook_system.resolve_command_path(&abs_path).unwrap();
        assert_eq!(resolved, hook_script);
    }

    #[test]
    fn test_determine_executable() {
        let temp_dir = TempDir::new().unwrap();
        let hook_system = HookSystem::new(Some(temp_dir.path())).unwrap();

        // Python file
        let (exec, args) = hook_system.determine_executable(Path::new("test.py"));
        assert_eq!(exec, "python3");
        assert_eq!(args, vec!["test.py"]);

        // Shell script
        let (exec, args) = hook_system.determine_executable(Path::new("test.sh"));
        assert_eq!(exec, "bash");
        assert_eq!(args, vec!["test.sh"]);

        // JavaScript file
        let (exec, args) = hook_system.determine_executable(Path::new("test.js"));
        assert_eq!(exec, "node");
        assert_eq!(args, vec!["test.js"]);
    }
}
