#![allow(clippy::unwrap_used, clippy::expect_used)]
use core_test_support::responses::{ev_completed, mount_sse_once, sse, start_mock_server};
use core_test_support::test_codex_exec::test_codex_exec;
use std::fs;
use tempfile::TempDir;

/// Test that blocking hooks cause exec to fail gracefully with an error
/// instead of calling process::exit(1)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blocking_hook_returns_error_instead_of_exit() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let hooks_dir = temp_dir.path().join(".claude/hooks");
    fs::create_dir_all(&hooks_dir)?;

    // Create a blocking hook that exits with code 2
    let blocking_hook = hooks_dir.join("blocking.sh");
    fs::write(
        &blocking_hook,
        r#"#!/bin/bash
echo "Hook is blocking this action" >&2
exit 2
"#,
    )?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&blocking_hook)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&blocking_hook, perms)?;
    }

    // Create settings.json to configure the hook
    let settings_file = temp_dir.path().join(".claude/settings.json");
    fs::write(
        &settings_file,
        r#"{
  "hooks": {
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "blocking.sh",
            "timeout": 5
          }
        ]
      }
    ]
  }
}"#,
    )?;

    let test = test_codex_exec();
    let server = start_mock_server().await;

    mount_sse_once(&server, sse(vec![ev_completed("request_0")])).await;

    // Execute codex with the blocking hook - should fail with error message
    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(temp_dir.path())
        .arg("test prompt")
        .assert()
        .failure() // Should fail gracefully
        .stderr(predicates::str::contains("Hook blocked execution"));

    Ok(())
}

/// Test that blocking hooks with decision="block" in JSON output also return errors
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blocking_hook_with_json_decision_returns_error() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let hooks_dir = temp_dir.path().join(".claude/hooks");
    fs::create_dir_all(&hooks_dir)?;

    // Create a hook that returns JSON with decision: "block"
    let blocking_hook = hooks_dir.join("json_block.sh");
    fs::write(
        &blocking_hook,
        r#"#!/bin/bash
# Read input from stdin (required for hook protocol)
cat > /dev/null
# Return JSON with blocking decision
echo '{"decision": "block", "reason": "Policy violation detected"}'
exit 0
"#,
    )?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&blocking_hook)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&blocking_hook, perms)?;
    }

    let settings_file = temp_dir.path().join(".claude/settings.json");
    fs::write(
        &settings_file,
        r#"{
  "hooks": {
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "json_block.sh",
            "timeout": 5
          }
        ]
      }
    ]
  }
}"#,
    )?;

    let test = test_codex_exec();
    let server = start_mock_server().await;

    mount_sse_once(&server, sse(vec![ev_completed("request_0")])).await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(temp_dir.path())
        .arg("test prompt")
        .assert()
        .failure()
        .stderr(predicates::str::contains("Hook blocked execution"));

    Ok(())
}

/// Test that non-blocking hooks execute successfully
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_blocking_hook_succeeds() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let hooks_dir = temp_dir.path().join(".claude/hooks");
    fs::create_dir_all(&hooks_dir)?;

    // Create a non-blocking hook that modifies the prompt
    let hook_script = hooks_dir.join("prefix.sh");
    fs::write(
        &hook_script,
        r#"#!/bin/bash
# Read input from stdin
input=$(cat)
# Extract the prompt and add a prefix
echo "$input" | jq -r '.prompt' | sed 's/^/PREFIXED: /'
exit 0
"#,
    )?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&hook_script)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook_script, perms)?;
    }

    let settings_file = temp_dir.path().join(".claude/settings.json");
    fs::write(
        &settings_file,
        r#"{
  "hooks": {
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "prefix.sh",
            "timeout": 5
          }
        ]
      }
    ]
  }
}"#,
    )?;

    let test = test_codex_exec();
    let server = start_mock_server().await;

    mount_sse_once(&server, sse(vec![ev_completed("request_0")])).await;

    // Should succeed - non-blocking hook
    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(temp_dir.path())
        .arg("test prompt")
        .assert()
        .success();

    Ok(())
}
