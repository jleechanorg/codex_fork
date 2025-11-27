//! Integration tests for slash command detection and substitution in codex-exec.
//!
//! These tests verify that slash commands are:
//! - Detected in user prompts
//! - Substituted with their content before being sent to the model
//! - Properly handle arguments via $ARGUMENTS placeholder

#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;
use std::fs;

/// Verify that a slash command is detected and substituted with its content.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn slash_command_is_substituted() -> anyhow::Result<()> {
    let test = test_codex_exec();

    // Create .claude/commands directory with a test command
    let claude_dir = test.cwd_path().join(".claude");
    let commands_dir = claude_dir.join("commands");
    fs::create_dir_all(&commands_dir)?;

    // Create a simple slash command
    fs::write(
        commands_dir.join("greet.md"),
        r#"---
name: greet
description: Greeting command
---

# Greeting Command

Say hello to: $ARGUMENTS
"#,
    )?;

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("response_1", "Hello there!"),
        responses::ev_completed("response_1"),
    ]);
    let mock = responses::mount_sse_once(&server, body).await;

    // Isolate from user's ~/.claude by setting HOME to test home
    test.cmd_with_server(&server)
        .env("HOME", test.home_path())
        .arg("--skip-git-repo-check")
        .arg("/greet World")
        .assert()
        .code(0);

    // Verify the substituted content was sent to the model
    let req = mock.single_request();
    let user_texts = req.message_input_texts("user");
    assert!(!user_texts.is_empty(), "should have user message");

    // The substituted content should contain the command content
    let combined = user_texts.join(" ");
    assert!(
        combined.contains("Say hello to:") || combined.contains("Greeting Command"),
        "substituted content should be in request, got: {combined}"
    );

    Ok(())
}

/// Verify that a prompt without a slash command is passed through unchanged.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn non_slash_command_passes_through() -> anyhow::Result<()> {
    let test = test_codex_exec();

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("response_1", "Regular response"),
        responses::ev_completed("response_1"),
    ]);
    let mock = responses::mount_sse_once(&server, body).await;

    // Isolate from user's ~/.claude by setting HOME to test home
    test.cmd_with_server(&server)
        .env("HOME", test.home_path())
        .arg("--skip-git-repo-check")
        .arg("regular prompt without slash")
        .assert()
        .code(0);

    // Verify the original prompt was sent
    let req = mock.single_request();
    let user_texts = req.message_input_texts("user");
    let combined = user_texts.join(" ");
    assert!(
        combined.contains("regular prompt without slash"),
        "original prompt should be in request, got: {combined}"
    );

    Ok(())
}

/// Verify that an unknown slash command falls back to original prompt.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unknown_slash_command_falls_back() -> anyhow::Result<()> {
    let test = test_codex_exec();

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("response_1", "Handling unknown command"),
        responses::ev_completed("response_1"),
    ]);
    let mock = responses::mount_sse_once(&server, body).await;

    // Isolate from user's ~/.claude by setting HOME to test home
    test.cmd_with_server(&server)
        .env("HOME", test.home_path())
        .arg("--skip-git-repo-check")
        .arg("/nonexistent_command test")
        .assert()
        .code(0);

    // The original prompt should be sent since the command doesn't exist
    let req = mock.single_request();
    let user_texts = req.message_input_texts("user");
    let combined = user_texts.join(" ");
    // Either the original command is sent or it was handled gracefully
    assert!(
        combined.contains("nonexistent_command") || combined.contains("/nonexistent_command"),
        "should handle unknown command gracefully, got: {combined}"
    );

    Ok(())
}

/// Verify that slash command arguments are properly substituted.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn slash_command_arguments_substituted() -> anyhow::Result<()> {
    let test = test_codex_exec();

    // Create .claude/commands directory with a test command
    let claude_dir = test.cwd_path().join(".claude");
    let commands_dir = claude_dir.join("commands");
    fs::create_dir_all(&commands_dir)?;

    // Create a command that uses $ARGUMENTS
    fs::write(
        commands_dir.join("echo.md"),
        r#"---
name: echo
description: Echo command
---

Echo back: $ARGUMENTS
"#,
    )?;

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("response_1", "Echoing"),
        responses::ev_completed("response_1"),
    ]);
    let mock = responses::mount_sse_once(&server, body).await;

    // Isolate from user's ~/.claude by setting HOME to test home
    test.cmd_with_server(&server)
        .env("HOME", test.home_path())
        .arg("--skip-git-repo-check")
        .arg("/echo hello world from test")
        .assert()
        .code(0);

    // Verify the arguments were substituted
    let req = mock.single_request();
    let user_texts = req.message_input_texts("user");
    let combined = user_texts.join(" ");
    assert!(
        combined.contains("hello world from test"),
        "arguments should be substituted, got: {combined}"
    );

    Ok(())
}

/// Verify that positional arguments ($1, $2, etc.) are substituted.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn slash_command_positional_arguments() -> anyhow::Result<()> {
    let test = test_codex_exec();

    // Create .claude/commands directory with a test command
    let claude_dir = test.cwd_path().join(".claude");
    let commands_dir = claude_dir.join("commands");
    fs::create_dir_all(&commands_dir)?;

    // Create a command that uses positional arguments
    fs::write(
        commands_dir.join("greet.md"),
        r#"---
name: greet
description: Greeting with positional args
---

Hello $1! Your favorite color is $2.
All args: $ARGUMENTS
"#,
    )?;

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("response_1", "Greeted"),
        responses::ev_completed("response_1"),
    ]);
    let mock = responses::mount_sse_once(&server, body).await;

    // Isolate from user's ~/.claude by setting HOME to test home
    test.cmd_with_server(&server)
        .env("HOME", test.home_path())
        .arg("--skip-git-repo-check")
        .arg("/greet Alice blue")
        .assert()
        .code(0);

    // Verify positional arguments were substituted
    let req = mock.single_request();
    let user_texts = req.message_input_texts("user");
    let combined = user_texts.join(" ");
    assert!(
        combined.contains("Hello Alice"),
        "$1 should be substituted with 'Alice', got: {}",
        combined
    );
    assert!(
        combined.contains("blue"),
        "$2 should be substituted with 'blue', got: {}",
        combined
    );

    Ok(())
}

/// Verify that .codexplus commands take precedence over .claude commands.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn codexplus_takes_precedence() -> anyhow::Result<()> {
    let test = test_codex_exec();

    // Create both .claude and .codexplus directories with same command name
    let claude_dir = test.cwd_path().join(".claude");
    let claude_commands = claude_dir.join("commands");
    fs::create_dir_all(&claude_commands)?;

    let codexplus_dir = test.cwd_path().join(".codexplus");
    let codexplus_commands = codexplus_dir.join("commands");
    fs::create_dir_all(&codexplus_commands)?;

    // Create competing commands
    fs::write(
        claude_commands.join("test.md"),
        r#"---
name: test
description: Claude version
---

This is the CLAUDE version
"#,
    )?;

    fs::write(
        codexplus_commands.join("test.md"),
        r#"---
name: test
description: Codexplus version
---

This is the CODEXPLUS version
"#,
    )?;

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("response_1", "Test response"),
        responses::ev_completed("response_1"),
    ]);
    let mock = responses::mount_sse_once(&server, body).await;

    // Note: For codexplus precedence test, we DON'T set HOME to test.home_path()
    // because we want both .codexplus and .claude in the CWD to be found
    test.cmd_with_server(&server)
        .env("HOME", test.home_path())
        .arg("--skip-git-repo-check")
        .arg("/test")
        .assert()
        .code(0);

    // Verify .codexplus version was used
    let req = mock.single_request();
    let user_texts = req.message_input_texts("user");
    let combined = user_texts.join(" ");
    assert!(
        combined.contains("CODEXPLUS"),
        ".codexplus should take precedence, got: {combined}"
    );

    Ok(())
}
