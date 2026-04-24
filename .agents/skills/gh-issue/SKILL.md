---
name: gh-issue
description: Strict issue-to-PR lifecycle for GitHub issues. Investigates, plans, implements (TDD), verifies, reviews, creates PR, waits for CI, and cleans up. Supports worktree isolation and durable run state for session resumption.
user_invocable: true
---

# /gh-issue - GitHub Issue Delivery Lifecycle

Strict delivery lifecycle that takes one or more GitHub issues from investigation through merged PR.
Use `/gh-runbook` first when the source material is still an audit, research report, or release review that needs grouped implementation issues.

## Usage

```
/gh-issue 375                    # Single issue
/gh-issue 375 376 377            # Multiple related issues (single PR)
/gh-issue 375 --worktree         # Isolated worktree
/gh-issue 375 --resume           # Resume after compaction
```

## State Management

All run state is managed via `python3 .agents/skills/gh-issue/scripts/gh_issue_run.py`:

```bash
# Initialize a new run
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py init-run <primary> <slug> [--issues 375 376 377] [--worktree]

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

### Phase 1: Investigating

1. Fetch issue(s) from GitHub:
   ```bash
   gh issue view <N> --repo aram-devdocs/omnifol --json number,title,body,labels,milestone,assignees,comments
   ```
2. Identify affected packages, layers, and files via codebase exploration
3. Check for blocking dependencies (issues referenced in body)
4. Read relevant `AGENTS.md`, scoped `CLAUDE.md`, and package rules
5. If the issue originated from an audit or report, verify its grouping and dependency model against `/gh-runbook`
5. Initialize run state:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py init-run <primary> <slug> --issues <N> [<M>...]
   ```

### Phase 2: Planning

1. Determine issue type from labels: `feat`, `fix`, `refactor`, `test`, `chore`, `docs`
2. Create branch name: `codex/<primary>-<type>-<slug>` (e.g., `codex/375-feat-idor-fix`)
3. Identify:
   - Target packages and files
   - Required subagents (implementer, database-migration, trpc-procedure, etc.)
   - Domain expert consultation needed (trading-domain-expert, omniscript-domain-expert)
   - Review gates needed (always spec + quality + architecture; security if auth/exchange/financial)
   - Whether a local `/gh-review --local-diff dev...HEAD` pass is required before opening the PR
   - Whether user-facing text needs a humanizer pass before review or PR creation
4. Write plan to `.agents/runs/gh-issue/<primary>-<slug>/plan.md` using `.agents/skills/gh-issue/assets/plan-template.md`
5. Present plan to user for approval before proceeding

### Phase 3: Bootstrapped

1. Create branch from `dev`:
   ```bash
   git checkout dev && git pull origin dev
   git checkout -b <branch-name>
   ```
2. If `--worktree` flag: use `EnterWorktree` instead
3. Update run state:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state <primary> <slug> --phase bootstrapped --branch <branch-name>
   ```

### Phase 4: Implementing

1. Consult domain expert if required (trading or omniscript work)
2. Dispatch appropriate subagent(s) per the plan:
   - Single agent for focused work
   - Parallel batch for independent changes (per parallel-orchestration rules)
3. Subagent follows TDD: scaffold failing tests first, then implement
4. Respect Omnifol constraints:
   - L1 -> L6 import direction only
   - stateless UI pattern: route wrapper -> page hook -> stateless page -> UI primitives
   - typed feature flags instead of hardcoded config
   - humanizer pass for user-facing docs, comments, and interface copy
5. Subagent commits atomically with conventional commit messages
5. Update run state after each commit:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state <primary> <slug> --commit <sha>
   ```

### Phase 5: Verifying

1. Run full verification:
   ```bash
   pnpm typecheck && pnpm lint && pnpm --filter @omnifol/<package> test
   ```
2. If failures: dispatch fix agent, loop back to implementing
3. If user-facing copy changed: run a humanizer pass before review
4. Update run state:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state <primary> <slug> --phase verifying
   ```

### Phase 6: Reviewing

Sequential review gates (per subagent-workflow rules):

1. **spec-reviewer** (sonnet): Verify implementation matches issue spec exactly
   - Dispatch with prompt from `.agents/skills/gh-issue/assets/prompts/review-dispatch.md`
   - If issues found: dispatch implementer fixes, re-verify, re-review
   - Record result: `python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state <primary> <slug> --review spec pass`
2. **code-quality-reviewer** (sonnet): Quality, patterns, maintainability
   - Only runs AFTER spec-reviewer passes
   - Record result: `--review quality pass`
3. **architecture-validator** (haiku): Run validation scripts
   - Record result: `--review architecture pass`
4. **security-auditor** (opus): Only if auth/exchange/financial code touched
   - NEVER parallel with other agents
   - Block on critical findings
   - Record result: `--review security pass`
5. **gh-review** (local dry run): Mirror `.github/workflows/claude-code-review.yml`
   - Run:
     ```bash
     python3 .agents/skills/gh-review/scripts/gh_review.py --local-diff dev...HEAD
     ```
   - If the PR already exists, prefer:
     ```bash
     python3 .agents/skills/gh-review/scripts/gh_review.py --pr <number>
     ```
   - Treat blocker findings as a return to implementation

### Phase 7: PR

1. Push branch:
   ```bash
   git push -u origin <branch-name>
   ```
2. Create the PR body from `.agents/skills/gh-issue/assets/pr-body-template.md`, which mirrors `.github/pull_request_template.md` section-for-section.
3. Create PR targeting `dev`:
   ```bash
   gh pr create --base dev --title "<type>: <description>" --body-file /tmp/<primary>-pr-body.md
   ```
4. Update run state:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state <primary> <slug> --phase pr --pr <number>
   ```

### Phase 8: Waiting CI

1. Poll CI:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py poll-pr <primary> <slug>
   ```
2. If CI fails:
   - Read failure logs: `gh pr checks <PR-number> --repo aram-devdocs/omnifol`
   - Dispatch debugger or implementer to fix
   - Push fix, loop back to waiting-ci
3. If CI passes: proceed to cleanup

### Phase 9: Cleanup

1. Update run state:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state <primary> <slug> --phase cleanup
   ```
2. If worktree:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py cleanup-worktree <primary> <slug>
   ```
3. Clean up local branch if merged:
   ```bash
   git checkout dev && git pull origin dev
   ```

### Phase 10: Done

1. Update run state:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state <primary> <slug> --phase done
   ```
2. Report summary: PR URL, issues addressed, review findings resolved

## Durable Run State

Each invocation creates a run directory at `.agents/runs/gh-issue/<primary>-<slug>/`.

### state.json

```json
{
  "primary": 375,
  "issues": [375, 376, 377],
  "slug": "idor-fix",
  "phase": "implementing",
  "branch": "codex/375-feat-idor-fix",
  "pr": null,
  "commits": ["abc1234", "def5678"],
  "reviews": {
    "spec": "pass",
    "quality": null,
    "architecture": null,
    "security": "not_required"
  },
  "worktree": false,
  "created": "2026-03-16T22:00:00Z",
  "updated": "2026-03-16T22:30:00Z"
}
```

### plan.md

Written during planning phase using `.agents/skills/gh-issue/assets/plan-template.md`. Contains:
- Issue summary and acceptance criteria
- Affected packages and files
- Implementation approach
- Subagent dispatch plan
- Review gates needed
- Adjacent skill usage (`/gh-runbook`, `/gh-review`, humanizer) when applicable

## Resuming After Compaction

When resuming a `/gh-issue` run after session compaction:

1. Check for active runs:
   ```bash
   ls .agents/runs/gh-issue/
   ```
2. Validate the run:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py validate-resume <primary> <slug>
   ```
3. Read the run's `state.json` to determine current phase
4. Read `plan.md` for full context
5. Resume from the current phase

The `save-session` PreCompact hook automatically surfaces active run state.

## Rules

- Branch MUST target `dev`, never `main`
- Branch pattern: `codex/<primary>-<type>-<slug>` (e.g., `codex/375-feat-idor-fix`)
- Commits MUST use conventional format: `<type>: <description>`
- TDD is mandatory: tests before implementation
- Review gates are mandatory and sequential: spec -> quality -> architecture -> security
- Domain expert consultation is mandatory for trading/omniscript work
- Never bypass pre-commit hooks
- All implementation goes through subagents, never direct orchestrator edits
- Use `python3 .agents/skills/gh-issue/scripts/gh_issue_run.py` for all state transitions
