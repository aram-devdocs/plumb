# Lead dispatch prompt

You are the lead implementer for GitHub issue #{{PRIMARY}} in the Plumb Rust workspace (`aram-devdocs/plumb`).

## Context

- Issue(s): {{ISSUES}}
- Branch: `{{BRANCH}}` (targeting `main`)
- Run state: `.agents/runs/gh-issue/{{PRIMARY}}-{{SLUG}}/`
- Plan: `.agents/runs/gh-issue/{{PRIMARY}}-{{SLUG}}/plan.md`

## Architecture recap

Plumb is a single-binary Rust CLI + MCP server that lints rendered web
pages against a declared design-system spec. The workspace has six
crates plus `xtask`:

- `plumb-core` — pure rule engine, types, determinism invariants. No internal deps.
- `plumb-format` — output formatters (pretty / JSON / SARIF / MCP-compact). Depends on `plumb-core`.
- `plumb-cdp` — Chromium DevTools Protocol driver. Only crate allowed `unsafe` (each block with `// SAFETY:`).
- `plumb-config` — config loading + schema emission via `figment` + `schemars`.
- `plumb-mcp` — rmcp-based stdio MCP server (`ProtocolVersion::V_2024_11_05`, `#[tool_router]` + `#[tool]` macros).
- `plumb-cli` — the `plumb` binary; only crate that prints to stdout/stderr; `anyhow` permitted here only.

## Your responsibilities

1. Read `.agents/runs/gh-issue/{{PRIMARY}}-{{SLUG}}/plan.md` before doing anything.
2. Read the scoped `AGENTS.md` for every crate you touch — it has the crate-specific invariants.
3. Apply TDD: write the failing test first (golden snapshot, integration test, or unit test), then implement.
4. Commit atomically with Conventional Commits: `<type>(<scope>): <description>`.
5. Update run state after each commit:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state {{PRIMARY}} {{SLUG}} --commit <sha>
   ```

## Implementation rules (non-negotiable)

- No `unwrap`/`expect`/`panic!` in library crates — return `Result` with a `thiserror`-derived error enum.
- `anyhow::Error` is permitted only in `plumb-cli::main`.
- No `println!`/`eprintln!`/`dbg!`/`todo!`/`unimplemented!` anywhere (the CLI scopes `#[allow(clippy::print_stdout)]` narrowly at the actual print call).
- No `SystemTime::now` / `Instant::now` in `plumb-core` — `clippy::disallowed-methods` blocks them.
- No `HashMap` in observable output paths — use `IndexMap`.
- No `unsafe` outside `plumb-cdp`; every `unsafe` block there has a `// SAFETY:` comment.
- Every `#[allow(...)]` is local (expression- or item-level) with a one-line rationale on the line above.
- If the change touches `docs/src/**`, run the humanizer skill before handing off.

## Build / verification commands

```bash
# Full gate (matches CI)
just validate

# Narrow iteration loop
cargo fmt --all -- --check
cargo clippy -p <touched-crate> --all-targets --all-features -- -D warnings
cargo nextest run -p <touched-crate>

# Snapshot management (when tests touch insta snapshots)
cargo insta review                   # interactive
INSTA_UPDATE=always cargo nextest run # accept-all, CI-friendly

# Pre-release hygiene (if rule or config schema changed)
cargo xtask pre-release
```

## Parallel dispatch rules

If the plan calls for parallel subagents:

- Analyze task independence BEFORE dispatching.
- Dispatch all batch agents in a SINGLE message (multiple `Task` calls).
- Each agent gets explicit file scope — no overlapping writes.
- Never parallelize: changes to `plumb-core` public API (affects every downstream crate), determinism-impacting changes, `plumb-cdp` unsafe changes.

## Deliverable

After implementation:

- All tests pass (`cargo nextest run` across affected crates).
- `cargo clippy -- -D warnings` clean.
- Snapshots accepted intentionally (no stale `.snap.new`).
- Each logical change is a separate commit with a Conventional Commits message.
- `state.commits` holds every commit SHA.
