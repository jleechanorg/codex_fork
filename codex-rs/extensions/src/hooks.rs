//! Hook system for lifecycle events
//!
//! Executes hook scripts at specific points in the Codex CLI lifecycle.
//! Hooks communicate via JSON stdin/stdout protocol.

use crate::error::ExtensionError;
use crate::error::Result;
use crate::settings::Settings;
use crate::settings::StatusLineConfig;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
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

    pub fn from_string(s: &str) -> Option<Self> {
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
    pub feedback: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook_specific_output: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

/// Result of hook execution
#[derive(Debug, Clone)]
pub struct HookResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub parsed_output: Option<HookOutput>,
}

#[derive(Debug, Clone)]
pub struct StatusLineResult {
    pub text: String,
    pub mode: Option<String>,
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

    /// Extract a human-readable feedback message if provided by the hook.
    pub fn feedback(&self) -> Option<&str> {
        self.parsed_output
            .as_ref()
            .and_then(|o| o.feedback.as_deref())
            .map(str::trim)
            .filter(|msg| !msg.is_empty())
    }
}

/// Hook system manager
#[derive(Debug)]
pub struct HookSystem {
    settings: Settings,
    #[allow(dead_code)]
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
        let mut current_input = input;

        for entry in hooks {
            if blocked {
                break;
            }

            for hook_config in &entry.hooks {
                if hook_config.hook_type != "command" {
                    continue;
                }

                let result = self
                    .execute_command_hook(&hook_config.command, &current_input, hook_config.timeout)
                    .await?;

                if let Some(prompt) = result
                    .parsed_output
                    .as_ref()
                    .and_then(|o| o.prompt.as_deref())
                    .map(ToString::to_string)
                {
                    current_input
                        .extra
                        .insert("prompt".to_string(), serde_json::Value::String(prompt));
                }

                let is_blocking = result.is_blocking();
                results.push(result);

                // Stop on first blocking hook
                if is_blocking {
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
        let cmd_display = cmd_path.display().to_string();

        // Determine how to execute (interpreter vs direct)
        let (executable, args) = self.determine_executable(&cmd_path);

        // Prepare input JSON
        let input_json = serde_json::to_string(input)?;

        if !cmd_path.exists() {
            tracing::warn!("Hook command missing: {cmd_display}");
            return Ok(HookResult {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                parsed_output: None,
            });
        }

        // Spawn process
        let mut child = Command::new(&executable)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(&self.project_dir)
            .env("CLAUDE_PROJECT_DIR", &self.project_dir)
            .spawn()
            .map_err(|e| {
                ExtensionError::HookExecutionFailed(format!("Failed to spawn {cmd_display}: {e}"))
            })?;

        // Write input to stdin
        // Note: Some hooks may exit immediately without reading stdin, causing a broken pipe.
        // This is normal behavior for hooks that don't need input, so we log but don't fail.
        if let Some(mut stdin) = child.stdin.take() {
            if let Err(e) = stdin.write_all(input_json.as_bytes()).await {
                // Log the error but continue - hook may have exited early
                eprintln!("Warning: Failed to write to hook stdin: {e}");
            }
            stdin.flush().await.ok();
            drop(stdin);
        }

        // Read stdout/stderr concurrently while waiting for completion
        let mut stdout_handle = None;
        if let Some(mut stdout_pipe) = child.stdout.take() {
            stdout_handle = Some(tokio::spawn(async move {
                let mut buf = Vec::new();
                let _ = stdout_pipe.read_to_end(&mut buf).await;
                String::from_utf8_lossy(&buf).to_string()
            }));
        }

        let mut stderr_handle = None;
        if let Some(mut stderr_pipe) = child.stderr.take() {
            stderr_handle = Some(tokio::spawn(async move {
                let mut buf = Vec::new();
                let _ = stderr_pipe.read_to_end(&mut buf).await;
                String::from_utf8_lossy(&buf).to_string()
            }));
        }

        // Wait with timeout and clean up on expiry to avoid leaking child processes
        let timeout = Duration::from_secs(timeout_secs);
        let wait_fut = child.wait();
        let status = match tokio::time::timeout(timeout, wait_fut).await {
            Ok(res) => res.map_err(|e| {
                ExtensionError::HookExecutionFailed(format!("Failed to wait for process: {e}"))
            })?,
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(ExtensionError::HookTimeout {
                    timeout_ms: timeout_secs * 1000,
                });
            }
        };

        let stdout = match stdout_handle {
            Some(handle) => handle.await.unwrap_or_default(),
            None => String::new(),
        };

        let stderr = match stderr_handle {
            Some(handle) => handle.await.unwrap_or_default(),
            None => String::new(),
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

    /// Execute a status line command using the existing hook machinery.
    pub async fn execute_status_line(&self, config: &StatusLineConfig) -> Result<HookResult> {
        let input = HookInput {
            session_id: String::new(),
            transcript_path: String::new(),
            cwd: self.project_dir.display().to_string(),
            hook_event_name: "statusLine".to_string(),
            extra: HashMap::new(),
        };

        self.execute_command_hook(&config.command, &input, config.timeout)
            .await
    }

    /// Resolve command path from hook configuration
    fn resolve_command_path(&self, command: &str) -> Result<PathBuf> {
        // If it's an absolute path, use it directly
        if Path::new(command).is_absolute() {
            return Ok(PathBuf::from(command));
        }

        // If it contains path separators, resolve relative to project dir
        if command.contains('/') || command.contains('\\') {
            return Ok(self.project_dir.join(command));
        }

        // Otherwise, search in hooks directories
        let hook_dirs = [
            self.project_dir.join(".claude/hooks"),
            self.project_dir.join(".codexplus/hooks"),
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
            if path.exists()
                && let Ok(metadata) = std::fs::metadata(path)
            {
                let permissions = metadata.permissions();
                if permissions.mode() & 0o111 != 0 {
                    // Is executable
                    return (path_str, vec![]);
                }
            }
        }

        // Check extension
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            let ext_lc = ext.to_ascii_lowercase();
            match ext_lc.as_str() {
                "py" => return ("python3".to_string(), vec![path_str]),
                "sh" => return ("bash".to_string(), vec![path_str]),
                "js" => return ("node".to_string(), vec![path_str]),
                _ => {}
            }

            #[cfg(windows)]
            match ext_lc.as_str() {
                "bat" | "cmd" => return ("cmd.exe".to_string(), vec!["/C".into(), path_str]),
                "ps1" => {
                    return (
                        "powershell.exe".to_string(),
                        vec![
                            "-NoProfile".into(),
                            "-ExecutionPolicy".into(),
                            "Bypass".into(),
                            "-File".into(),
                            path_str,
                        ],
                    );
                }
                _ => {}
            }
        }

        // Default: try to execute directly
        (path_str, vec![])
    }
}

#[derive(Debug, Clone)]
pub struct UserPromptHookOutcome {
    pub prompt: String,
    pub results: Vec<HookResult>,
}

impl UserPromptHookOutcome {
    fn unchanged(prompt: &str) -> Self {
        Self {
            prompt: prompt.to_string(),
            results: Vec::new(),
        }
    }
}

fn resolve_hook_project_dir(project_dir: Option<&Path>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = project_dir {
        candidates.push(dir.to_path_buf());
        if let Some(parent) = dir.parent() {
            candidates.push(parent.to_path_buf());
        }
    } else if let Ok(dir) = std::env::current_dir() {
        candidates.push(dir.clone());
        if let Some(parent) = dir.parent() {
            candidates.push(parent.to_path_buf());
        }
    }

    for dir in candidates {
        if dir.join(".claude/settings.json").exists()
            || dir.join(".codexplus/settings.json").exists()
        {
            return Some(dir);
        }
    }
    None
}

/// Execute UserPromptSubmit hooks for a prompt, returning the possibly modified prompt and hook results.
pub async fn execute_user_prompt_submit_hooks(
    prompt: &str,
    cwd: Option<&Path>,
    session_id: Option<&str>,
) -> Result<UserPromptHookOutcome> {
    let debug_hooks = std::env::var("CODEX_DEBUG_HOOKS").is_ok();
    let resolved_project_dir = resolve_hook_project_dir(cwd);
    if resolved_project_dir.is_none() {
        if debug_hooks {
            eprintln!(
                "UserPromptSubmit hooks skipped (no settings) for cwd: {}",
                cwd.map(|p| p.display().to_string())
                    .unwrap_or_else(|| "<unset>".to_string())
            );
        }
        tracing::debug!("UserPromptSubmit hooks skipped: no local settings found");
        return Ok(UserPromptHookOutcome::unchanged(prompt));
    }
    let resolved_project_dir = resolved_project_dir.unwrap();

    let settings = match Settings::load(Some(resolved_project_dir.as_path())) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("Hook settings failed to load, skipping hooks: {e}");
            return Ok(UserPromptHookOutcome::unchanged(prompt));
        }
    };

    if !settings.hooks.contains_key("UserPromptSubmit") {
        if debug_hooks {
            eprintln!(
                "UserPromptSubmit hooks skipped: event not configured in {}",
                resolved_project_dir.display()
            );
        }
        tracing::debug!(
            "UserPromptSubmit hooks skipped: no event configured in {}",
            resolved_project_dir.display()
        );
        return Ok(UserPromptHookOutcome::unchanged(prompt));
    }

    tracing::debug!(
        "Executing UserPromptSubmit hooks from {}",
        resolved_project_dir.display()
    );
    let hook_system = HookSystem::new(Some(resolved_project_dir.as_path()))?;

    let mut extra = HashMap::new();
    extra.insert(
        "prompt".to_string(),
        serde_json::Value::String(prompt.to_string()),
    );

    let resolved_session_id = session_id
        .map(ToOwned::to_owned)
        .or_else(|| std::env::var("CODEX_SESSION_ID").ok())
        .unwrap_or_else(|| format!("session-{}", std::process::id()));

    let input = HookInput {
        session_id: resolved_session_id,
        transcript_path: String::new(),
        cwd: cwd
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| resolved_project_dir.display().to_string()),
        hook_event_name: HookEvent::UserPromptSubmit.as_str().to_string(),
        extra,
    };

    let results = match hook_system
        .execute(HookEvent::UserPromptSubmit, input)
        .await
    {
        Ok(results) => results,
        Err(e) => {
            tracing::warn!("UserPromptSubmit hook execution failed: {e}");
            return Ok(UserPromptHookOutcome::unchanged(prompt));
        }
    };

    if debug_hooks {
        eprintln!(
            "UserPromptSubmit hooks executed: {} result(s)",
            results.len()
        );
    }
    let mut updated_prompt = prompt.to_string();
    for result in &results {
        if let Some(new_prompt) = result
            .parsed_output
            .as_ref()
            .and_then(|o| o.prompt.as_deref())
        {
            updated_prompt = new_prompt.to_string();
        }
    }

    Ok(UserPromptHookOutcome {
        prompt: updated_prompt,
        results,
    })
}

/// Execute the configured status line command and return a displayable string, if any.
pub async fn execute_status_line(cwd: Option<&Path>) -> Result<Option<StatusLineResult>> {
    let resolved_project_dir = resolve_hook_project_dir(cwd);
    if resolved_project_dir.is_none() {
        return Ok(None);
    }
    let resolved_project_dir = resolved_project_dir.unwrap();

    let settings = match Settings::load(Some(resolved_project_dir.as_path())) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("Status line settings failed to load, skipping: {e}");
            return Ok(None);
        }
    };

    let Some(status_cfg) = settings.status_line else {
        return Ok(None);
    };

    let hook_system = HookSystem::new(Some(resolved_project_dir.as_path()))?;
    let result = match hook_system.execute_status_line(&status_cfg).await {
        Ok(res) => res,
        Err(e) => {
            tracing::warn!("Status line command failed: {e}");
            return Ok(None);
        }
    };

    if result.exit_code != 0 {
        tracing::warn!(
            "Status line command exited with {} (stderr: {})",
            result.exit_code,
            result.stderr.trim()
        );
        return Ok(None);
    }

    let text = result
        .parsed_output
        .and_then(|o| o.feedback)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| result.stdout.trim().to_string());

    if text.is_empty() {
        return Ok(None);
    }

    Ok(Some(StatusLineResult {
        text,
        mode: status_cfg.mode.clone(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_hook_event_conversion() {
        assert_eq!(HookEvent::UserPromptSubmit.as_str(), "UserPromptSubmit");
        assert_eq!(
            HookEvent::from_string("UserPromptSubmit"),
            Some(HookEvent::UserPromptSubmit)
        );
        assert_eq!(HookEvent::from_string("Invalid"), None);
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
                feedback: None,
                hook_specific_output: None,
                prompt: None,
            }),
        };
        assert!(result2.is_blocking());
        assert_eq!(result2.block_reason(), Some("Test block".to_string()));
    }

    #[test]
    fn test_hook_result_feedback_trims() {
        let result = HookResult {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            parsed_output: Some(HookOutput {
                decision: None,
                reason: None,
                feedback: Some("  hello world  ".to_string()),
                hook_specific_output: None,
                prompt: None,
            }),
        };

        assert_eq!(result.feedback(), Some("hello world"));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_execute_simple_hook() {
        use std::env;

        let temp_dir = TempDir::new().unwrap();

        // Make test hermetic to user HOME
        let temp_home = temp_dir.path().join("fake_home");
        std::fs::create_dir_all(&temp_home).unwrap();
        unsafe {
            env::set_var("HOME", &temp_home);
        }

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

        unsafe {
            env::remove_var("HOME");
        }

        assert_eq!(results.len(), 1);
        assert!(results[0].stdout.contains("test-session"));
    }

    #[tokio::test]
    #[cfg(unix)]
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
