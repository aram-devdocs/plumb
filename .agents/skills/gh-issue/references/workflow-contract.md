# Workflow contract

Binding contract for `/gh-issue` skill execution in Plumb.

## Phase transitions

Each phase transition requires an explicit state update via `gh_issue_run.py update-state`.

| From | To | Trigger |
|------|----|---------|
| investigating | planning | Issue fetched, codebase explored, run initialized |
| planning | bootstrapped | Plan written, user approved, branch created |
| bootstrapped | implementing | Branch pushed, subagents dispatched |
| implementing | verifying | All subagents complete, commits recorded |
| verifying | reviewing | `just validate` (or narrow cargo-fmt/clippy/nextest loop) passes |
| reviewing | pr | All required review gates pass |
| pr | waiting-ci | PR created, number recorded |
| waiting-ci | cleanup | All CI checks pass |
| cleanup | done | Local cleanup complete |

## Invariants

1. **Branch invariant**: `state.branch` matches `codex/<primary>-<type>-<slug>` once set.
2. **Review order invariant**: spec → quality → architecture → test, in order. `security` may run in parallel with any of the above when triggered.
3. **Plan invariant**: `plan.md` exists once `phase > investigating`.
4. **Commit invariant**: every commit on the branch is recorded in `state.commits` before phase advances past `implementing`.
5. **PR invariant**: `state.pr` is set before phase advances past `pr`.

## Plumb-specific rules

### Crate layering (must not violate)

```
plumb-core ── plumb-format
            └─ plumb-cdp         (only crate permitted `unsafe`)
            └─ plumb-config
            └─ plumb-mcp         (depends on plumb-core + plumb-format)
            └─ plumb-cli         (top; only crate permitted stdout/stderr + anyhow)
```

Cross-cutting invariants enforced by lint / CI:

- `forbid(unsafe_code)` everywhere except `plumb-cdp`.
- `deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo, clippy::unimplemented, clippy::dbg_macro, clippy::print_stdout, clippy::print_stderr, missing_docs)` workspace-wide.
- `clippy::disallowed-methods` blocks `SystemTime::now`, `Instant::now`, `std::env::temp_dir` — strict in `plumb-core`.
- Observable output uses `IndexMap`, not `HashMap`.
- Violation sort key: `(rule_id, viewport, selector, dom_order)`.
- Error types: `thiserror` in library crates; `anyhow` only in `plumb-cli::main`.

### Build verification (must pass before PR)

```bash
just validate
```

Equivalent breakdown:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-features
just determinism-check
cargo deny check
```

### Branch naming

- Pattern: `codex/<primary>-<type>-<slug>`
- Example: `codex/17-feat-cdp-chromium-launch`
- Types: `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `build`, `ci`, `chore`, `style`, `revert`

### PR rules

- Target: `main` (Plumb has no `dev` branch).
- Title: Conventional Commits format — the `pr-title` workflow validates it.
- Body: `.agents/skills/gh-issue/assets/pr-body-template.md` (mirrors `.github/PULL_REQUEST_TEMPLATE.md`).
- `Fixes #<primary>` in body — the linked issue auto-closes on merge.

### Review gate order

1. `02-spec-reviewer` (always).
2. `03-code-quality-reviewer` (always, after spec passes).
3. `05-architecture-validator` (always).
4. `04-test-runner` (always, after architecture passes).
5. `06-security-auditor` — parallel with any of the above when the change touches `plumb-cdp`, `plumb-mcp`, URL/config parsing, or deps.
6. `/gh-review --local-diff main...HEAD` — local mirror of the GitHub Claude code-review workflow.

### TDD requirement

- Failing test is written first — golden snapshot for rules, integration test for CLI/MCP behavior, unit test for pure functions.
- Snapshots live under `crates/<crate>/tests/snapshots/`.
- `cargo insta review` accepts intentional snapshot changes (or `INSTA_UPDATE=always cargo nextest run` in CI).

### Subagent dispatch rules

- All implementation flows through a subagent (`delegation-guard` hook enforces this on the root orchestrator).
- Parallel dispatch: a single message with multiple `Task` calls.
- Independence analysis required before parallel dispatch.
- Hard-sequential: changes to `plumb-core` public API (affects every downstream crate), security-auditor runs, determinism-impacting changes.

## Non-negotiable blockers

- `cargo fmt --check`, `cargo clippy -- -D warnings`, or `cargo test` failures in final verification.
- `02-spec-reviewer` returning `BLOCK` (redesign required, not patch).
- `06-security-auditor` critical finding.
- Determinism regression (`just determinism-check` byte-diff fails).
- Binary size regression ≥ 25 MiB (the size-guard CI job fails).
- `cargo deny check` failure (new advisory or license issue).
- CI failure not fixed within the session.

## Definitions

- **primary**: the main GitHub issue number driving the PR.
- **slug**: short kebab-case identifier from the issue title.
- **run directory**: `.agents/runs/gh-issue/<primary>-<slug>/`.
- **review gate**: a formal reviewer subagent invocation that emits a `Verdict: APPROVE|REQUEST_CHANGES|BLOCK` line.
