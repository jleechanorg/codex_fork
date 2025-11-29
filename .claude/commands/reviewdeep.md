# /reviewdeep Command

**Purpose**
Deliver a high-signal, security-aware code review entirely inside the CLI harness - no MCP agents or external automation required. Mirrors the manual process we just ran.

## Usage
```bash
/reviewdeep [<path>|<PR>]      # review current checkout or specific target
```

## Review Flow
1. **Context & Diff Intake**
   - `git status -sb` for scope
   - `git show HEAD` (or staged diff) for patch view

2. **Analysis Tracks**
   - **Correctness & Design**: inspect key files, invariants, regression risk
   - **Security & Error Surfaces**: auth flows, trust boundaries, error propagation
   - **Testing & Coverage**: confirm existing or missing automated tests

3. **CLI Helpers (optional)**
   - `rg`, local scripts for targeted search
   - Project test suite (`npm run test`, etc.) if quick enough

4. **Synthesis**
   - Summarize findings ordered by severity (High/Med/Low/Info)
   - Each finding = title, file:line when possible, impact, remediation guidance
   - Record verification steps (tests run, manual checks)

## Output Template
```
Findings
- High - <title> (<file:line>): <issue + fix>
- Medium - ...

Verification
- Tests: <commands> (pass/fail)
- Notes: <manual review, scripts run>
```

## Notes
- Built for solo maintainers; emphasize actionable steps over exhaustive prose
- No MCP requirements; runs fully in sandboxed CLI
- Keep responses concise and evidence-backed
