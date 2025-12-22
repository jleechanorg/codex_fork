/*
Module: orchestrator

Central place for approvals + sandbox selection + retry semantics. Drives a
simple sequence for any ToolRuntime: approval → select sandbox → attempt →
retry without sandbox on denial (no re‑approval thanks to caching).
*/
use crate::error::CodexErr;
use crate::error::SandboxErr;
use crate::error::get_error_message_ui;
use crate::exec::ExecToolCallOutput;
use crate::protocol::BackgroundEventEvent;
use crate::protocol::EventMsg;
use crate::sandboxing::SandboxManager;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ProvidesSandboxRetryData;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use codex_extensions::HookEvent;
use codex_extensions::HookInput;
use codex_extensions::HookResult;
use codex_extensions::HookSystem;
use codex_extensions::Settings;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

pub(crate) struct ToolOrchestrator {
    sandbox: SandboxManager,
}

impl ToolOrchestrator {
    pub fn new() -> Self {
        Self {
            sandbox: SandboxManager::new(),
        }
    }

    pub async fn run<Rq, Out, T>(
        &mut self,
        tool: &mut T,
        req: &Rq,
        tool_ctx: &ToolCtx<'_>,
        turn_ctx: &crate::codex::TurnContext,
        approval_policy: AskForApproval,
    ) -> Result<Out, ToolError>
    where
        T: ToolRuntime<Rq, Out>,
        Rq: ProvidesSandboxRetryData,
    {
        let otel = turn_ctx.client.get_otel_event_manager();
        let otel_tn = &tool_ctx.tool_name;
        let otel_ci = &tool_ctx.call_id;
        let otel_user = codex_otel::otel_event_manager::ToolDecisionSource::User;
        let otel_cfg = codex_otel::otel_event_manager::ToolDecisionSource::Config;

        // 1) Approval
        let needs_initial_approval =
            tool.wants_initial_approval(req, approval_policy, &turn_ctx.sandbox_policy);
        let mut already_approved = false;

        if needs_initial_approval {
            let mut risk = None;

            if let Some(metadata) = req.sandbox_retry_data() {
                risk = tool_ctx
                    .session
                    .assess_sandbox_command(turn_ctx, &tool_ctx.call_id, &metadata.command, None)
                    .await;
            }

            let approval_ctx = ApprovalCtx {
                session: tool_ctx.session,
                turn: turn_ctx,
                call_id: &tool_ctx.call_id,
                retry_reason: None,
                risk,
            };
            let decision = tool.start_approval_async(req, approval_ctx).await;

            otel.tool_decision(otel_tn, otel_ci, decision, otel_user.clone());

            match decision {
                ReviewDecision::Denied | ReviewDecision::Abort => {
                    return Err(ToolError::Rejected("rejected by user".to_string()));
                }
                ReviewDecision::Approved | ReviewDecision::ApprovedForSession => {}
            }
            already_approved = true;
        } else {
            otel.tool_decision(otel_tn, otel_ci, ReviewDecision::Approved, otel_cfg);
        }

        // 2) First attempt under the selected sandbox.
        let mut initial_sandbox = self
            .sandbox
            .select_initial(&turn_ctx.sandbox_policy, tool.sandbox_preference());
        if tool.wants_escalated_first_attempt(req) {
            initial_sandbox = crate::exec::SandboxType::None;
        }
        // Platform-specific flag gating is handled by SandboxManager::select_initial
        // via crate::safety::get_platform_sandbox().
        let initial_attempt = SandboxAttempt {
            sandbox: initial_sandbox,
            policy: &turn_ctx.sandbox_policy,
            manager: &self.sandbox,
            sandbox_cwd: &turn_ctx.cwd,
            codex_linux_sandbox_exe: turn_ctx.codex_linux_sandbox_exe.as_ref(),
        };

        // Execute PreToolUse hooks (pass empty JSON since req isn't serializable)
        let tool_input_json = serde_json::Value::Object(serde_json::Map::new());
        execute_pre_tool_use_hooks(tool_ctx, turn_ctx, &tool_input_json).await?;

        match tool.run(req, &initial_attempt, tool_ctx).await {
            Ok(out) => {
                // Execute PostToolUse hooks
                let tool_response_json = serde_json::Value::Object(serde_json::Map::new());
                let _ = execute_post_tool_use_hooks(
                    tool_ctx,
                    turn_ctx,
                    &tool_input_json,
                    &tool_response_json,
                )
                .await; // Don't fail if post hooks error

                // We have a successful initial result
                Ok(out)
            }
            Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied { output }))) => {
                if !tool.escalate_on_failure() {
                    return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                        output,
                    })));
                }
                // Under `Never` or `OnRequest`, do not retry without sandbox; surface a concise
                // sandbox denial that preserves the original output.
                if !tool.wants_no_sandbox_approval(approval_policy) {
                    return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                        output,
                    })));
                }

                // Ask for approval before retrying without sandbox.
                if !tool.should_bypass_approval(approval_policy, already_approved) {
                    let mut risk = None;

                    if let Some(metadata) = req.sandbox_retry_data() {
                        let err = SandboxErr::Denied {
                            output: output.clone(),
                        };
                        let friendly = get_error_message_ui(&CodexErr::Sandbox(err));
                        let failure_summary = format!("failed in sandbox: {friendly}");

                        risk = tool_ctx
                            .session
                            .assess_sandbox_command(
                                turn_ctx,
                                &tool_ctx.call_id,
                                &metadata.command,
                                Some(failure_summary.as_str()),
                            )
                            .await;
                    }

                    let reason_msg = build_denial_reason_from_output(output.as_ref());
                    let approval_ctx = ApprovalCtx {
                        session: tool_ctx.session,
                        turn: turn_ctx,
                        call_id: &tool_ctx.call_id,
                        retry_reason: Some(reason_msg),
                        risk,
                    };

                    let decision = tool.start_approval_async(req, approval_ctx).await;
                    otel.tool_decision(otel_tn, otel_ci, decision, otel_user);

                    match decision {
                        ReviewDecision::Denied | ReviewDecision::Abort => {
                            return Err(ToolError::Rejected("rejected by user".to_string()));
                        }
                        ReviewDecision::Approved | ReviewDecision::ApprovedForSession => {}
                    }
                }

                let escalated_attempt = SandboxAttempt {
                    sandbox: crate::exec::SandboxType::None,
                    policy: &turn_ctx.sandbox_policy,
                    manager: &self.sandbox,
                    sandbox_cwd: &turn_ctx.cwd,
                    codex_linux_sandbox_exe: None,
                };

                // Execute PreToolUse hooks for retry
                execute_pre_tool_use_hooks(tool_ctx, turn_ctx, &tool_input_json).await?;

                // Second attempt.
                let result = (*tool).run(req, &escalated_attempt, tool_ctx).await;

                // Execute PostToolUse hooks if successful
                if result.is_ok() {
                    let tool_response_json = serde_json::Value::Object(serde_json::Map::new());
                    let _ = execute_post_tool_use_hooks(
                        tool_ctx,
                        turn_ctx,
                        &tool_input_json,
                        &tool_response_json,
                    )
                    .await;
                }

                result
            }
            other => other,
        }
    }
}

/// Execute PreToolUse hooks before tool execution
async fn execute_pre_tool_use_hooks(
    tool_ctx: &ToolCtx<'_>,
    turn_ctx: &crate::codex::TurnContext,
    tool_input: &serde_json::Value,
) -> Result<(), ToolError> {
    let Some(project_dir) = resolve_hook_settings_dir(Some(turn_ctx.cwd.as_path())) else {
        return Ok(());
    };
    // Load settings
    let settings = match Settings::load(Some(project_dir.as_path())) {
        Ok(s) => s,
        Err(_) => return Ok(()), // No hooks configured
    };

    // Check if PreToolUse hooks are configured
    if !settings.hooks.contains_key("PreToolUse") {
        return Ok(());
    }

    // Create hook system
    let hook_system = match HookSystem::new(Some(project_dir.as_path())) {
        Ok(h) => h,
        Err(_) => return Ok(()),
    };

    // Build hook input
    let mut extra = HashMap::new();
    extra.insert(
        "tool_name".to_string(),
        serde_json::Value::String(tool_ctx.tool_name.clone()),
    );
    extra.insert(
        "tool_use_id".to_string(),
        serde_json::Value::String(tool_ctx.call_id.clone()),
    );
    extra.insert("tool_input".to_string(), tool_input.clone());

    let input = HookInput {
        session_id: turn_ctx.sub_id.clone(),
        transcript_path: String::new(),
        cwd: turn_ctx.cwd.display().to_string(),
        hook_event_name: "PreToolUse".to_string(),
        extra,
    };

    // Execute hooks - if hooks fail to execute (e.g., missing scripts), log and continue
    let results = match hook_system.execute(HookEvent::PreToolUse, input).await {
        Ok(results) => results,
        Err(e) => {
            // Hook execution failed (missing script, spawn error, etc.)
            // Log the error but don't fail tool execution
            tracing::warn!("PreToolUse hook execution failed: {e}");
            return Ok(());
        }
    };

    record_hook_feedback(tool_ctx, turn_ctx, HookEvent::PreToolUse.as_str(), &results).await;

    // Check for blocking hooks
    for result in &results {
        if result.is_blocking() {
            let reason = result
                .block_reason()
                .unwrap_or_else(|| "Hook blocked tool execution".to_string());
            return Err(ToolError::Rejected(reason));
        }
    }

    Ok(())
}

/// Execute PostToolUse hooks after tool execution
async fn execute_post_tool_use_hooks(
    tool_ctx: &ToolCtx<'_>,
    turn_ctx: &crate::codex::TurnContext,
    tool_input: &serde_json::Value,
    tool_response: &serde_json::Value,
) -> Result<(), ToolError> {
    let Some(project_dir) = resolve_hook_settings_dir(Some(turn_ctx.cwd.as_path())) else {
        return Ok(());
    };
    // Load settings
    let settings = match Settings::load(Some(project_dir.as_path())) {
        Ok(s) => s,
        Err(_) => return Ok(()), // No hooks configured
    };

    // Check if PostToolUse hooks are configured
    if !settings.hooks.contains_key("PostToolUse") {
        return Ok(());
    }

    // Create hook system
    let hook_system = match HookSystem::new(Some(project_dir.as_path())) {
        Ok(h) => h,
        Err(_) => return Ok(()),
    };

    // Build hook input
    let mut extra = HashMap::new();
    extra.insert(
        "tool_name".to_string(),
        serde_json::Value::String(tool_ctx.tool_name.clone()),
    );
    extra.insert(
        "tool_use_id".to_string(),
        serde_json::Value::String(tool_ctx.call_id.clone()),
    );
    extra.insert("tool_input".to_string(), tool_input.clone());
    extra.insert("tool_response".to_string(), tool_response.clone());

    let input = HookInput {
        session_id: turn_ctx.sub_id.clone(),
        transcript_path: String::new(),
        cwd: turn_ctx.cwd.display().to_string(),
        hook_event_name: "PostToolUse".to_string(),
        extra,
    };

    // Execute hooks - if hooks fail to execute, log and continue
    let results = match hook_system.execute(HookEvent::PostToolUse, input).await {
        Ok(results) => results,
        Err(e) => {
            // Hook execution failed (missing script, spawn error, etc.)
            // Log the error but don't fail since tool already executed
            tracing::warn!("PostToolUse hook execution failed: {e}");
            return Ok(());
        }
    };

    record_hook_feedback(
        tool_ctx,
        turn_ctx,
        HookEvent::PostToolUse.as_str(),
        &results,
    )
    .await;

    // Check for blocking hooks (informational for PostToolUse since tool already ran)
    for result in &results {
        if result.is_blocking() {
            tracing::warn!("PostToolUse hook blocked, but tool already executed");
        }
    }

    Ok(())
}

fn build_denial_reason_from_output(_output: &ExecToolCallOutput) -> String {
    // Keep approval reason terse and stable for UX/tests, but accept the
    // output so we can evolve heuristics later without touching call sites.
    "command failed; retry without sandbox?".to_string()
}

async fn record_hook_feedback(
    tool_ctx: &ToolCtx<'_>,
    turn_ctx: &crate::codex::TurnContext,
    event_name: &str,
    results: &[HookResult],
) {
    if results.is_empty() {
        return;
    }
    let tool_name = &tool_ctx.tool_name;
    let prefix = match event_name {
        "PreToolUse" | "PostToolUse" => format!("{event_name} hook ({tool_name})"),
        _ => format!("{event_name} hook"),
    };

    for result in results {
        if let Some(feedback) = result.feedback() {
            let message = format!("{prefix}: {feedback}");
            tool_ctx
                .session
                .send_event(
                    turn_ctx,
                    EventMsg::BackgroundEvent(BackgroundEventEvent { message }),
                )
                .await;
        }
    }
}

fn resolve_hook_settings_dir(project_dir: Option<&Path>) -> Option<PathBuf> {
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

    candidates.into_iter().find(|dir| {
        dir.join(".claude/settings.json").exists() || dir.join(".codexplus/settings.json").exists()
    })
}
