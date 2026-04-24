# Feedback triage prompt

You are triaging reviewer feedback for GitHub issue #{{PRIMARY}}.

## Reviewer: {{REVIEWER}}
## Verdict: {{VERDICT}}

## Feedback

{{FEEDBACK}}

## Your task

Categorize each piece of feedback:

1. **Must fix** — spec violation, bug, security issue, layering / determinism violation, missing test for a public change.
2. **Should fix** — code quality, error-shape drift, pattern inconsistency, missing `# Errors` doc.
3. **Consider** — style preference, minor optimization, non-blocking suggestion.
4. **Disagree** — explain why the feedback does not apply.

## Dispatch decision

For each "Must fix" and "Should fix" item, identify:

- Which file(s) need changing (with `file:line` when cited).
- Which subagent to dispatch:
  - `01-implementer` for general code changes.
  - `08-rule-author` when a rule definition or its doc/test needs to change.
  - `09-mcp-tool-author` when an MCP tool definition or its test needs to change.
  - `10-quick-fix` only when the change is a one-commit trivial fix that needs no new test.
  - `07-debugger` if the root cause is unclear before fixing.
- Whether the fixes are independent (parallel batch) or sequential.

## After fixes

Re-run the gate:

```bash
just validate
```

Or the narrow loop:

```bash
cargo fmt --all -- --check && \
  cargo clippy -p <crate> --all-targets --all-features -- -D warnings && \
  cargo nextest run -p <crate>
```

Then re-dispatch the same reviewer gate.

Update run state only after the reviewer emits `Verdict: APPROVE`:

```bash
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state {{PRIMARY}} {{SLUG}} --review {{GATE}} pass
```

## Rules

- Never mark a gate as passed without an explicit `Verdict: APPROVE`.
- Sequence holds: `02-spec-reviewer` must APPROVE before dispatching `03-code-quality-reviewer`; quality before architecture; architecture before test. `06-security-auditor` may run in parallel when triggered.
- Never skip a required gate.
- Never modify test assertions to force green — fix the code or update the snapshot intentionally via `cargo insta review`.
