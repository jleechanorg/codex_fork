---
name: consensus
description: "/consensus Command - Multi-Agent Consensus Review"
type: llm-orchestration
execution_mode: immediate
argument-hint: "[scope]"
---

# /consensus Command - Multi-Agent Consensus Review

High-speed consensus command tailored for the Codex Plus proxy. Run it when you need a decisive multi-agent-style review of code, documentation, or launch decisions. The workflow is optimized for solo MVP projects with GitHub rollback safety.

> ⚠️ **Codex Plus compatibility:**
> - Execute every step directly in this CLI environment; do not rely on unavailable tools.
> - If the Task tool is not exposed, simulate the 5 agents by running sequential sub-analyses and labeling each clearly.
> - Use regular follow-up questions instead of TodoWrite; only track tasks in plain markdown checklists when helpful.
> - Keep protected proxy files untouched (see `CLAUDE.md`).

## Usage
```bash
/consensus [<scope>]
/cons [<scope>]           # alias
```
- **Scope**: Defaults to the active PR plus any local modifications. Provide file paths or PR numbers to narrow the review.

## Phase 0 – Mode Selection
1. Determine whether the request concerns **Code Review**, **Documentation & Spec**, or **Operational Decision** scope.
2. If ambiguous, pause and ask the user which mode they prefer before collecting context.
3. Record the selected mode for later reporting.

## Phase 1 – Context Acquisition
Run the following (adjust when information is unavailable):
- `gh pr status` or `git config branch.<branch>.merge` → detect active PR.
- `git log -1 --stat` → capture latest commit info.
- `git status --short` → list local changes.
- `git diff --stat` and targeted `git diff <file>` snippets → inspect modifications.
- `gh pr view <pr> --json files,headRefName,baseRefName` → confirm synchronization when PR exists.
- Redact obvious secrets before sharing context.
- Bundle the PR description, commit summary, diff notes, and local-only edits into a concise briefing for the agents.

## Phase 2 – Agent Roster & Prompts
Pick the 5-agent set that matches the mode:
- **Code Review Mode**: `code-review`, `codex-consultant`, `gemini-consultant`, `cursor-consultant`, `code-centralization-consultant`
- **Documentation & Spec Mode**: `accuracy-reviewer`, `evidence-verifier`, `product-strategist`, `delivery-ops`, `clarity-editor`
- **Operational Decision Mode**: `risk-analyst`, `product-strategist`, `delivery-ops`, `customer-advocate`, `exec-synthesizer`

Launch all five agents in parallel using the Task tool when available.
> **Fallback for unsupported parallel execution:** If the Task tool cannot run agents concurrently, execute the same five prompts sequentially. Expect each agent run to take several minutes, so a full three-round consensus may require noticeable extra time. Watch for CLI timeouts or stalled executions; if delays occur, narrow the scope, trim optional prompts, or pause after each agent to confirm progress.

### Prompt Template
For each agent, provide:
- Solo MVP context (pre-launch, rollback friendly).
- Scope summary (files/docs/decision statement).
- Explanation of the agent's specialization and responsibilities.
- Checklist tailored to the mode (bugs & architecture for code, evidence & clarity for docs, risk & mitigation for decisions).
- Output requirements: **PASS** or **REWORK**, confidence (1-10), evidence references, 2-3 bullet summary, and next actions if REWORK.

Stop the round immediately if any agent uncovers a severity 9-10 blocker.

## Phase 3 – Consensus Loop (max 3 rounds)
1. Collect PASS/REWORK verdicts and confidence scores.
2. Compute:
   - **CONSENSUS_PASS** when ≥3 agents PASS *and* average confidence ≥6.
   - **CONSENSUS_REWORK** when ≥3 agents request REWORK *or* average confidence ≤5.
   - Otherwise note **MIXED_SIGNALS**, document disagreements, and continue with majority direction.
3. Apply quick fixes for REWORK items before starting another round (up to three total).
4. Perform validation appropriate to the mode:
   - **Code Review**: Prefer `./run_tests.sh`; fall back to targeted `pytest`, `npm test`, or linting checks.
   - **Documentation & Spec**: Cross-check cited evidence, ensure metrics and references align.
   - **Operational Decision**: Validate risk mitigation steps, owner assignments, and timeline realism.
5. Abort immediately on failed tests, validation gaps, or critical blockers.

## Output Format
Summarize using the following structure:
```
# Consensus Review Report

## Summary
- Round count: <1-3>
- Final status: PASS | REWORK_LIMIT_REACHED | VALIDATION_ABORT | MIXED_SIGNALS
- Mode: Code Review | Documentation & Spec | Operational Decision
- Key validated areas

## Major Findings
| Round | Source Agent | Reference (file/section) | Severity | Resolution |
| ----- | ------------- | ------------------------ | -------- | ---------- |

## Implemented Fixes / Actions
- <list of updates or decisions>

## Evidence & Validation Log
- Tests run / evidence cross-checked / stakeholders consulted

## Round-by-Round Summaries
- Round <n>: <highlights>
  - <agent>: <key takeaways>

## Remaining Follow-Ups
- <nitpicks, deferred work, open questions>
```

Always mention executed commands (e.g., `pytest`, `gh pr view`), evidence sources, and unresolved risks. Confirm the working tree status before finishing.
