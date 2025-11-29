// - In the default output mode, it is paramount that the only thing written to
//   stdout is the final message (if any).
// - In --json mode, stdout must be valid JSONL, one event per line.
// For both modes, any other output must be written to stderr.
#![deny(clippy::print_stdout)]

mod cli;
mod event_processor;
mod event_processor_with_human_output;
pub mod event_processor_with_jsonl_output;
pub mod exec_events;

pub use cli::Cli;
use codex_core::AuthManager;
use codex_core::BUILT_IN_OSS_MODEL_PROVIDER_ID;
use codex_core::ConversationManager;
use codex_core::NewConversation;
use codex_core::auth::enforce_login_restrictions;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::git_info::get_git_repo_root;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SessionSource;
use codex_ollama::DEFAULT_OSS_MODEL;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::user_input::UserInput;
use event_processor_with_human_output::EventProcessorWithHumanOutput;
use event_processor_with_jsonl_output::EventProcessorWithJsonOutput;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use serde_json::Value;
use std::io::IsTerminal;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use supports_color::Stream;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

use crate::cli::Command as ExecCommand;
use crate::event_processor::CodexStatus;
use crate::event_processor::EventProcessor;
use codex_core::default_client::set_default_originator;
use codex_core::find_conversation_path_by_id_str;
use codex_extensions::StatusLineResult;
use codex_extensions::execute_status_line;

pub async fn run_main(cli: Cli, codex_linux_sandbox_exe: Option<PathBuf>) -> anyhow::Result<()> {
    if let Err(err) = set_default_originator("codex_exec".to_string()) {
        tracing::warn!(?err, "Failed to set codex exec originator override {err:?}");
    }

    let Cli {
        command,
        images,
        model: model_cli_arg,
        oss,
        config_profile,
        full_auto,
        dangerously_bypass_approvals_and_sandbox,
        cwd,
        skip_git_repo_check,
        add_dir,
        color,
        last_message_file,
        json: json_mode,
        sandbox_mode: sandbox_mode_cli_arg,
        prompt,
        output_schema: output_schema_path,
        config_overrides,
    } = cli;

    // Determine the prompt source (parent or subcommand) and read from stdin if needed.
    let prompt_arg = match &command {
        // Allow prompt before the subcommand by falling back to the parent-level prompt
        // when the Resume subcommand did not provide its own prompt.
        Some(ExecCommand::Resume(args)) => args.prompt.clone().or(prompt),
        None => prompt,
    };

    let prompt = match prompt_arg {
        Some(p) if p != "-" => p,
        // Either `-` was passed or no positional arg.
        maybe_dash => {
            // When no arg (None) **and** stdin is a TTY, bail out early – unless the
            // user explicitly forced reading via `-`.
            let force_stdin = matches!(maybe_dash.as_deref(), Some("-"));

            if std::io::stdin().is_terminal() && !force_stdin {
                eprintln!(
                    "No prompt provided. Either specify one as an argument or pipe the prompt into stdin."
                );
                std::process::exit(1);
            }

            // Ensure the user knows we are waiting on stdin, as they may
            // have gotten into this state by mistake. If so, and they are not
            // writing to stdin, Codex will hang indefinitely, so this should
            // help them debug in that case.
            if !force_stdin {
                eprintln!("Reading prompt from stdin...");
            }
            let mut buffer = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut buffer) {
                eprintln!("Failed to read prompt from stdin: {e}");
                std::process::exit(1);
            } else if buffer.trim().is_empty() {
                eprintln!("No prompt provided via stdin.");
                std::process::exit(1);
            }
            buffer
        }
    };

    // Execute UserPromptSubmit hooks on the original prompt
    let prompt = match execute_user_prompt_submit_hook(&prompt, cwd.as_deref()).await {
        Ok(updated) => updated,
        Err(e) => {
            tracing::warn!("UserPromptSubmit hook failed, continuing: {e}");
            prompt
        }
    };

    // Slash command detection and substitution on the (possibly) modified prompt
    let prompt = match detect_and_substitute_slash_command(&prompt, cwd.as_deref()).await {
        Ok(substituted) => substituted,
        Err(e) => {
            tracing::warn!(
                "Slash command detection failed; using original prompt: {}",
                e
            );
            prompt // Fall back to original prompt if detection fails
        }
    };

    if prompt.is_empty() {
        // Slash command handled (e.g., /statusline); nothing further to do.
        return Ok(());
    }

    let output_schema = load_output_schema(output_schema_path);

    let (stdout_with_ansi, stderr_with_ansi) = match color {
        cli::Color::Always => (true, true),
        cli::Color::Never => (false, false),
        cli::Color::Auto => (
            supports_color::on_cached(Stream::Stdout).is_some(),
            supports_color::on_cached(Stream::Stderr).is_some(),
        ),
    };

    // Build fmt layer (existing logging) to compose with OTEL layer.
    let default_level = "error";

    // Build env_filter separately and attach via with_filter.
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(default_level))
        .unwrap_or_else(|_| EnvFilter::new(default_level));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(stderr_with_ansi)
        .with_writer(std::io::stderr)
        .with_filter(env_filter);

    let sandbox_mode = if full_auto {
        Some(SandboxMode::WorkspaceWrite)
    } else if dangerously_bypass_approvals_and_sandbox {
        Some(SandboxMode::DangerFullAccess)
    } else {
        sandbox_mode_cli_arg.map(Into::<SandboxMode>::into)
    };

    // When using `--oss`, let the bootstrapper pick the model (defaulting to
    // gpt-oss:20b) and ensure it is present locally. Also, force the built‑in
    // `oss` model provider.
    let model = if let Some(model) = model_cli_arg {
        Some(model)
    } else if oss {
        Some(DEFAULT_OSS_MODEL.to_owned())
    } else {
        None // No model specified, will use the default.
    };

    let model_provider = if oss {
        Some(BUILT_IN_OSS_MODEL_PROVIDER_ID.to_string())
    } else {
        None // No specific model provider override.
    };

    if let Some(line) = maybe_status_line(cwd.as_deref()).await {
        eprintln!("Status line: {line}");
    }

    // Load configuration and determine approval policy
    let overrides = ConfigOverrides {
        model,
        review_model: None,
        config_profile,
        // Default to never ask for approvals in headless mode. Feature flags can override.
        approval_policy: Some(AskForApproval::Never),
        sandbox_mode,
        cwd: cwd.map(|p| p.canonicalize().unwrap_or(p)),
        model_provider,
        codex_linux_sandbox_exe,
        base_instructions: None,
        developer_instructions: None,
        compact_prompt: None,
        include_apply_patch_tool: None,
        show_raw_agent_reasoning: oss.then_some(true),
        tools_web_search_request: None,
        experimental_sandbox_command_assessment: None,
        additional_writable_roots: add_dir,
    };
    // Parse `-c` overrides.
    let cli_kv_overrides = match config_overrides.parse_overrides() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error parsing -c overrides: {e}");
            std::process::exit(1);
        }
    };

    let config = Config::load_with_cli_overrides(cli_kv_overrides, overrides).await?;

    if let Err(err) = enforce_login_restrictions(&config).await {
        eprintln!("{err}");
        std::process::exit(1);
    }

    let otel = codex_core::otel_init::build_provider(&config, env!("CARGO_PKG_VERSION"));

    #[allow(clippy::print_stderr)]
    let otel = match otel {
        Ok(otel) => otel,
        Err(e) => {
            eprintln!("Could not create otel exporter: {e}");
            std::process::exit(1);
        }
    };

    if let Some(provider) = otel.as_ref() {
        let otel_layer = OpenTelemetryTracingBridge::new(&provider.logger).with_filter(
            tracing_subscriber::filter::filter_fn(codex_core::otel_init::codex_export_filter),
        );

        let _ = tracing_subscriber::registry()
            .with(fmt_layer)
            .with(otel_layer)
            .try_init();
    } else {
        let _ = tracing_subscriber::registry().with(fmt_layer).try_init();
    }

    let mut event_processor: Box<dyn EventProcessor> = match json_mode {
        true => Box::new(EventProcessorWithJsonOutput::new(last_message_file.clone())),
        _ => Box::new(EventProcessorWithHumanOutput::create_with_ansi(
            stdout_with_ansi,
            &config,
            last_message_file.clone(),
        )),
    };

    if oss {
        codex_ollama::ensure_oss_ready(&config)
            .await
            .map_err(|e| anyhow::anyhow!("OSS setup failed: {e}"))?;
    }

    let default_cwd = config.cwd.to_path_buf();
    let default_approval_policy = config.approval_policy;
    let default_sandbox_policy = config.sandbox_policy.clone();
    let default_model = config.model.clone();
    let default_effort = config.model_reasoning_effort;
    let default_summary = config.model_reasoning_summary;

    if !skip_git_repo_check && get_git_repo_root(&default_cwd).is_none() {
        eprintln!("Not inside a trusted directory and --skip-git-repo-check was not specified.");
        std::process::exit(1);
    }

    let auth_manager = AuthManager::shared(
        config.codex_home.clone(),
        true,
        config.cli_auth_credentials_store_mode,
    );
    let conversation_manager = ConversationManager::new(auth_manager.clone(), SessionSource::Exec);

    // Handle resume subcommand by resolving a rollout path and using explicit resume API.
    let NewConversation {
        conversation_id: _,
        conversation,
        session_configured,
    } = if let Some(ExecCommand::Resume(args)) = command {
        let resume_path = resolve_resume_path(&config, &args).await?;

        if let Some(path) = resume_path {
            conversation_manager
                .resume_conversation_from_rollout(config.clone(), path, auth_manager.clone())
                .await?
        } else {
            conversation_manager
                .new_conversation(config.clone())
                .await?
        }
    } else {
        conversation_manager
            .new_conversation(config.clone())
            .await?
    };
    // Print the effective configuration and prompt so users can see what Codex
    // is using.
    event_processor.print_config_summary(&config, &prompt, &session_configured);

    info!("Codex initialized with event: {session_configured:?}");

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    {
        let conversation = conversation.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        tracing::debug!("Keyboard interrupt");
                        // Immediately notify Codex to abort any in‑flight task.
                        conversation.submit(Op::Interrupt).await.ok();

                        // Exit the inner loop and return to the main input prompt. The codex
                        // will emit a `TurnInterrupted` (Error) event which is drained later.
                        break;
                    }
                    res = conversation.next_event() => match res {
                        Ok(event) => {
                            debug!("Received event: {event:?}");

                            let is_shutdown_complete = matches!(event.msg, EventMsg::ShutdownComplete);
                            if let Err(e) = tx.send(event) {
                                error!("Error sending event: {e:?}");
                                break;
                            }
                            if is_shutdown_complete {
                                info!("Received shutdown event, exiting event loop.");
                                break;
                            }
                        },
                        Err(e) => {
                            error!("Error receiving event: {e:?}");
                            break;
                        }
                    }
                }
            }
        });
    }

    // Package images and prompt into a single user input turn.
    let mut items: Vec<UserInput> = images
        .into_iter()
        .map(|path| UserInput::LocalImage { path })
        .collect();
    items.push(UserInput::Text { text: prompt });
    let initial_prompt_task_id = conversation
        .submit(Op::UserTurn {
            items,
            cwd: default_cwd,
            approval_policy: default_approval_policy,
            sandbox_policy: default_sandbox_policy,
            model: default_model,
            effort: default_effort,
            summary: default_summary,
            final_output_json_schema: output_schema,
        })
        .await?;
    info!("Sent prompt with event ID: {initial_prompt_task_id}");

    // Run the loop until the task is complete.
    // Track whether a fatal error was reported by the server so we can
    // exit with a non-zero status for automation-friendly signaling.
    let mut error_seen = false;
    while let Some(event) = rx.recv().await {
        if matches!(event.msg, EventMsg::Error(_)) {
            error_seen = true;
        }
        let shutdown: CodexStatus = event_processor.process_event(event);
        match shutdown {
            CodexStatus::Running => continue,
            CodexStatus::InitiateShutdown => {
                conversation.submit(Op::Shutdown).await?;
            }
            CodexStatus::Shutdown => {
                break;
            }
        }
    }
    event_processor.print_final_output();
    if error_seen {
        std::process::exit(1);
    }

    Ok(())
}

async fn resolve_resume_path(
    config: &Config,
    args: &crate::cli::ResumeArgs,
) -> anyhow::Result<Option<PathBuf>> {
    if args.last {
        let default_provider_filter = vec![config.model_provider_id.clone()];
        match codex_core::RolloutRecorder::list_conversations(
            &config.codex_home,
            1,
            None,
            &[],
            Some(default_provider_filter.as_slice()),
            &config.model_provider_id,
        )
        .await
        {
            Ok(page) => Ok(page.items.first().map(|it| it.path.clone())),
            Err(e) => {
                error!("Error listing conversations: {e}");
                Ok(None)
            }
        }
    } else if let Some(id_str) = args.session_id.as_deref() {
        let path = find_conversation_path_by_id_str(&config.codex_home, id_str).await?;
        Ok(path)
    } else {
        Ok(None)
    }
}

/// Executes UserPromptSubmit hooks and applies any modifications to the prompt.
async fn execute_user_prompt_submit_hook(
    prompt: &str,
    cwd: Option<&Path>,
) -> anyhow::Result<String> {
    let debug_hooks = std::env::var("CODEX_DEBUG_HOOKS").is_ok();
    if debug_hooks {
        eprintln!(
            "exec: invoking UserPromptSubmit hooks (cwd={})",
            cwd.map(|p| p.display().to_string())
                .unwrap_or_else(|| "<unset>".to_string())
        );
    }
    let session_id = std::env::var("CODEX_SESSION_ID")
        .unwrap_or_else(|_| format!("exec-{}", std::process::id()));

    let outcome =
        match codex_extensions::execute_user_prompt_submit_hooks(prompt, cwd, Some(&session_id))
            .await
        {
            Ok(outcome) => outcome,
            Err(e) => {
                tracing::warn!("UserPromptSubmit hook execution failed: {e}");
                return Ok(prompt.to_string());
            }
        };

    if debug_hooks {
        eprintln!(
            "exec: UserPromptSubmit hook returned {} result(s)",
            outcome.results.len()
        );
    }
    for result in &outcome.results {
        if let Some(feedback) = result.feedback() {
            eprintln!("UserPromptSubmit hook: {feedback}");
        }
        if result.is_blocking() {
            let reason = result
                .block_reason()
                .unwrap_or_else(|| "Hook blocked execution".to_string());
            eprintln!("Hook blocked execution: {reason}");
            std::process::exit(1);
        }
    }

    Ok(outcome.prompt)
}

/// Detects slash commands in the prompt and substitutes them with their content.
async fn detect_and_substitute_slash_command(
    prompt: &str,
    cwd: Option<&Path>,
) -> anyhow::Result<String> {
    use codex_extensions::SlashCommandRegistry;

    // Check if this looks like a slash command
    if let Some((cmd_name, args)) = SlashCommandRegistry::detect_command(prompt) {
        tracing::info!("Detected slash command: /{cmd_name} with args: '{args}'");

        // Determine project directory (cwd or current dir)
        let current_dir_fallback = std::env::current_dir().ok();
        let project_dir = cwd.or(current_dir_fallback.as_deref());

        // Load slash commands from .claude/.codexplus directories
        let registry = SlashCommandRegistry::load(project_dir)?;

        // Try to get the command
        if let Some(command) = registry.get(&cmd_name) {
            let substituted = command.substitute_arguments(&args);
            tracing::info!(
                "Substituted slash command /{cmd_name} (from {})",
                command.file_path.display()
            );
            Ok(substituted)
        } else if cmd_name == "statusline" {
            // Claude Code exposes /statusline as a built-in; honor it even without a .md file.
            if let Some(line) = maybe_status_line(cwd).await {
                eprintln!("Status line: {line}");
                return Ok(String::new());
            }
            tracing::warn!("/statusline requested but no status line configuration found");
            Ok(String::new())
        } else {
            tracing::warn!("Slash command /{cmd_name} not found in registry");
            Ok(prompt.to_string()) // Return original if command not found
        }
    } else {
        // Not a slash command, return original
        Ok(prompt.to_string())
    }
}

async fn maybe_status_line(cwd: Option<&Path>) -> Option<String> {
    match execute_status_line(cwd).await {
        Ok(Some(StatusLineResult { text, .. })) if !text.trim().is_empty() => Some(text),
        Ok(_) => None,
        Err(err) => {
            tracing::warn!("Status line execution failed: {err}");
            None
        }
    }
}

fn load_output_schema(path: Option<PathBuf>) -> Option<Value> {
    let path = path?;

    let schema_str = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!(
                "Failed to read output schema file {}: {err}",
                path.display()
            );
            std::process::exit(1);
        }
    };

    match serde_json::from_str::<Value>(&schema_str) {
        Ok(value) => Some(value),
        Err(err) => {
            eprintln!(
                "Output schema file {} is not valid JSON: {err}",
                path.display()
            );
            std::process::exit(1);
        }
    }
}
