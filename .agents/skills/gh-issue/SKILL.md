---
name: gh-issue
description: Strict issue-to-PR lifecycle for GitHub issues in the Plumb Rust workspace. Investigates, plans, implements (TDD), verifies, reviews, creates PR, waits for CI, and cleans up. Supports worktree isolation and durable run state for session resumption.
user_invocable: true
---

# /gh-issue — GitHub issue delivery lifecycle for Plumb

Strict delivery lifecycle that drives one or more GitHub issues from
investigation through merged PR against `aram-devdocs/plumb`.

Use `/gh-runbook` first when the source is a runbook spec that still
needs to be fanned out into grouped issues.

## Usage

```
/gh-issue 17                     # Single issue
/gh-issue 17 18 19               # Multiple related issues (single PR)
/gh-issue 17 --worktree          # Isolated worktree
/gh-issue 17 --resume            # Resume after compaction
```

## State management

```bash
# Initialize a new run
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py init-run <primary> <slug> [--issues 17 18 19] [--worktree]

# Update phase or field
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state <primary> <slug> --phase implementing
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state <primary> <slug> --pr 42
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state <primary> <slug> --review spec pass
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state <primary> <slug> --commit abc1234

# Validate a run can be resumed
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py validate-resume <primary> <slug>

# Poll PR CI status
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py poll-pr <primary> <slug>

# Clean up worktree after done
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py cleanup-worktree <primary> <slug>
```

## Phases

```
investigating > planning > bootstrapped > implementing > verifying > reviewing > pr > waiting-ci > cleanup > done
```

## Lifecycle

### Phase 1 — investigating

1. Fetch issue(s):
   ```bash
   gh issue view <N> --repo aram-devdocs/plumb --json number,title,body,labels,milestone,assignees,comments
   ```
2. Identify affected crates, layers, and files via codebase exploration.
3. Read the root `AGENTS.md`, the scoped `AGENTS.md` for any crate touched (`crates/plumb-*/AGENTS.md`), and `.agents/rules/` — the Plumb invariants live there.
4. Check for blocking dependencies (issues referenced in body).
5. If the issue came from a runbook spec, re-read the spec in `docs/runbooks/` to confirm batch and gate context.
6. Initialize run state:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py init-run <primary> <slug> --issues <N> [<M>...]
   ```

### Phase 2 — planning

1. Determine issue type from labels: `feat`, `fix`, `refactor`, `test`, `chore`, `docs`, `perf`, `ci`, `build`, `style`, `revert`.
2. Branch name: `codex/<primary>-<type>-<slug>` (e.g., `codex/17-feat-cdp-chromium-launch`).
3. Identify:
   - Target crate(s) and files.
   - Required subagent(s) from Plumb's set: `01-implementer`, `08-rule-author` (for new rules), `09-mcp-tool-author` (for MCP tools), `10-quick-fix` (for trivial fixes).
   - Review gates — always `02-spec-reviewer` → `03-code-quality-reviewer` → `05-architecture-validator` → `04-test-runner`. Add `06-security-auditor` in parallel when the change touches `plumb-cdp`, `plumb-mcp`, URL handling, or dependency-graph changes.
   - Whether a local `/gh-review --local-diff main...HEAD` pass is required before opening the PR.
   - Whether user-facing docs need a humanizer pass (any PR touching `docs/src/**`).
4. Write the plan to `.agents/runs/gh-issue/<primary>-<slug>/plan.md` using `.agents/skills/gh-issue/assets/plan-template.md`.
5. Present the plan to the user for approval before proceeding.

### Phase 3 — bootstrapped

1. Create the branch from `main`:
   ```bash
   git checkout main && git pull origin main
   git checkout -b <branch-name>
   ```
2. If `--worktree` flag: use `EnterWorktree` instead.
3. Update run state:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state <primary> <slug> --phase bootstrapped --branch <branch-name>
   ```

### Phase 4 — implementing

1. Dispatch the right subagent per the plan:
   - `01-implementer` for general work.
   - `08-rule-author` for new rule + golden test + doc.
   - `09-mcp-tool-author` for new MCP tool + protocol test.
   - `10-quick-fix` only for trivial, single-commit work — otherwise use `01-implementer`.
2. TDD: write the failing test first (golden snapshot, integration test, or unit test), then the minimum code to pass.
3. Honor Plumb invariants (`.agents/rules/`):
   - Layer discipline — `plumb-core` depends on nothing internal; `unsafe` only in `plumb-cdp` with `// SAFETY:`; `println!`/`eprintln!` only in `plumb-cli`.
   - Determinism — no `SystemTime::now`/`Instant::now` in `plumb-core`; `IndexMap` for observable output; sort key `(rule_id, viewport, selector, dom_order)`.
   - No `unwrap`/`expect`/`panic!` in library crates; `thiserror`-derived enums for errors; `anyhow` only in `plumb-cli::main`.
   - No `todo!`/`unimplemented!`/`dbg!`.
4. Commit atomically with Conventional Commits: `<type>(<scope>): <description>`.
5. Update run state after each commit:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state <primary> <slug> --commit <sha>
   ```

### Phase 5 — verifying

1. Run the full gate:
   ```bash
   just validate
   ```
   Or the narrow equivalent while iterating:
   ```bash
   cargo fmt --all -- --check && \
     cargo clippy --workspace --all-targets --all-features -- -D warnings && \
     cargo nextest run --workspace --all-features
   ```
2. If failures: dispatch `07-debugger` to diagnose, then `01-implementer` to fix, then loop back to verifying.
3. If docs under `docs/src/**` changed: run the humanizer skill before review.
4. Update run state:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state <primary> <slug> --phase verifying
   ```

### Phase 6 — reviewing

Sequential gates (review-gate-guard hook enforces the order):

1. **`02-spec-reviewer`** — does the change satisfy the spec exactly, nothing more?
   - Record: `--review spec pass` (or `fail`).
2. **`03-code-quality-reviewer`** — idiomatic Rust, error shapes, lint suppression justified? Only runs after spec passes.
   - Record: `--review quality pass`.
3. **`05-architecture-validator`** — layering, unsafe boundary, deny-lints respected?
   - Record: `--review architecture pass`.
4. **`04-test-runner`** — runs `just validate` + `just determinism-check` + `cargo deny check`.
   - Record: `--review test pass`.
5. **`06-security-auditor`** (parallel with any of the above when triggered) — required when the change touches `plumb-cdp`, `plumb-mcp`, URL/config parsing, or deps.
   - Record: `--review security pass` or `not_required`.
6. **`/gh-review` local dry-run** — mirror of `.github/workflows/claude-code-review.yml`:
   ```bash
   python3 .agents/skills/gh-review/scripts/gh_review.py --local-diff main...HEAD
   ```
   If the PR already exists, prefer `--pr <number>`. Any blocker finding sends you back to `implementing`.

### Phase 7 — pr

1. Push the branch:
   ```bash
   git push -u origin <branch-name>
   ```
2. Render the PR body from `.agents/skills/gh-issue/assets/pr-body-template.md` — it mirrors `.github/PULL_REQUEST_TEMPLATE.md` section-for-section.
3. Create the PR targeting `main`:
   ```bash
   gh pr create --repo aram-devdocs/plumb --base main \
     --title "<type>(<scope>): <description>" \
     --body-file /tmp/<primary>-pr-body.md
   ```
4. Update run state:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state <primary> <slug> --phase pr --pr <number>
   ```

### Phase 8 — waiting-ci

1. Poll:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py poll-pr <primary> <slug>
   ```
2. On CI failure: `gh pr checks <PR> --repo aram-devdocs/plumb --fail-fast`, dispatch `07-debugger` → `01-implementer`, push fix, loop.
3. On pass: advance to cleanup.

### Phase 9 — cleanup

1. Update run state to `cleanup`.
2. If `--worktree`: `cleanup-worktree`.
3. Return to `main`:
   ```bash
   git checkout main && git pull origin main
   ```

### Phase 10 — done

1. `--phase done`.
2. Report: PR URL, issues addressed, review verdicts, CI outcome.

## Durable run state

Per invocation: `.agents/runs/gh-issue/<primary>-<slug>/`.

### state.json

```json
{
  "primary": 17,
  "issues": [17],
  "slug": "cdp-chromium-launch",
  "phase": "implementing",
  "branch": "codex/17-feat-cdp-chromium-launch",
  "pr": null,
  "commits": ["abc1234"],
  "reviews": {
    "spec": "pass",
    "quality": null,
    "architecture": null,
    "test": null,
    "security": "not_required"
  },
  "worktree": false,
  "created": "2026-04-23T22:00:00Z",
  "updated": "2026-04-23T22:30:00Z"
}
```

### plan.md

Written during planning from `assets/plan-template.md`. Covers: issue summary, acceptance criteria, affected crates, implementation approach, subagent dispatch plan, review gates, adjacent skill usage (`/gh-runbook`, `/gh-review`, humanizer).

## Resuming after compaction

```bash
ls .agents/runs/gh-issue/
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py validate-resume <primary> <slug>
```

Then read `state.json` and `plan.md` and resume from the recorded phase. The `save-session` Stop hook writes a per-session summary so context survives compaction.

## Rules

- Branch MUST target `main`. Plumb has no `dev` branch.
- Branch pattern: `codex/<primary>-<type>-<slug>`.
- Commits use Conventional Commits — the `commit-msg` lefthook validator enforces it.
- TDD is mandatory: test first, implementation second.
- Review gates are mandatory and sequential: spec → quality → architecture → test; security-auditor in parallel when triggered.
- Never bypass pre-commit or pre-push hooks — no `--no-verify`.
- All implementation goes through subagents; the root orchestrator never edits `crates/*/src/*.rs` directly (`delegation-guard` hook enforces this).
- Every state transition goes through `gh_issue_run.py`.

See also: `AGENTS.md`, `.agents/rules/`, `/gh-review`, `/gh-runbook`, humanizer skill.
