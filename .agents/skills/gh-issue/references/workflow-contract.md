# Workflow Contract

This document defines the binding contract for /gh-issue skill execution in Omnifol.

## Phase Transitions

Each phase transition requires explicit state update via `gh_issue_run.py update-state`.

| From | To | Trigger |
|------|----|---------|
| investigating | planning | Issue fetched, codebase explored, run initialized |
| planning | bootstrapped | Plan written, user approved, branch created |
| bootstrapped | implementing | Branch pushed, subagents dispatched |
| implementing | verifying | All subagents complete, commits recorded |
| verifying | reviewing | `pnpm typecheck && pnpm lint && pnpm test` pass |
| reviewing | pr | All required review gates pass |
| pr | waiting-ci | PR created, number recorded |
| waiting-ci | cleanup | All CI checks pass |
| cleanup | done | Local cleanup complete |

## Invariants

These must hold at all times:

1. **Branch invariant**: `state.branch` matches `codex/<primary>-<type>-<slug>` pattern once set
2. **Review order invariant**: `quality != pass` while `spec != pass`
3. **Plan invariant**: `plan.md` exists once phase > `investigating`
4. **Commit invariant**: all commits are recorded in `state.commits` before phase advances
5. **PR invariant**: `state.pr` is set before phase advances past `pr`

## Omnifol-Specific Rules

### Layer Architecture (must not violate)
```
L1 Core -> L2 Data -> L3 Infra -> L4 Business -> L5 Integration -> L6 Apps
```

### Build Verification (must pass before PR)
```bash
pnpm typecheck && pnpm lint && pnpm --filter @omnifol/<pkg> test
```

### Branch Naming
- Pattern: `codex/<primary>-<type>-<slug>`
- Example: `codex/375-feat-idor-fix`
- Type from issue labels: feat, fix, refactor, test, chore, docs

### PR Rules
- Target: `dev` (NEVER `main`)
- Title: conventional commit format
- Body: references `.github/pull_request_template.md` structure
- Fixes: `Fixes #<primary>`

### Review Gate Order
1. spec-reviewer (always)
2. code-quality-reviewer (always, after spec passes)
3. architecture-validator (always)
4. security-auditor (if auth/exchange/financial touched)

### TDD Requirement
- Tests MUST be written before implementation
- Test files co-located with source files
- Minimum 80% coverage for business logic

### Subagent Dispatch Rules
- ALL implementation through subagents, never direct orchestrator edits
- Parallel dispatch: single message with multiple Task calls
- Independence analysis required before parallel dispatch
- Hard sequential: database migrations, compiler changes, security reviews

## Non-Negotiable Blockers

These stop the workflow entirely:

- Pre-commit hook failures (fix the issue, never use `--no-verify`)
- TypeScript errors in final verification
- spec-reviewer REJECTED verdict (requires redesign, not patch)
- security-auditor critical finding (block merge)
- CI failure not fixed within reasonable time

## Definitions

- **primary**: the main GitHub issue number driving the PR
- **slug**: short kebab-case identifier from the issue title
- **run directory**: `.agents/runs/gh-issue/<primary>-<slug>/`
- **review gate**: a formal agent review with explicit APPROVED/REJECTED verdict
