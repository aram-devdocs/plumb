# ADR 0001 — Bootstrap conventions

**Status:** Accepted
**Date:** 2026-04-23
**Deciders:** Aram Hammoudeh

## Context

Plumb's first commit establishes every convention the project will be
maintained under. This ADR captures the non-obvious choices so future
contributors (human or agent) can understand the why before changing the
what.

The PRD (`docs/local/prd.md`) specifies functional requirements. This
ADR covers the infrastructure decisions the PRD doesn't: workspace
layout, lint policy, hook strategy, AI tooling layout, release pipeline.

## Decisions

### 1. Cargo workspace with six crates

`plumb-core`, `plumb-format`, `plumb-cdp`, `plumb-config`, `plumb-mcp`,
`plumb-cli`. Enforced hierarchy in `.agents/rules/dependency-hierarchy.md`.

**Rationale.** Sharp crate boundaries force discipline. A rule that
accidentally takes a dependency on Chromium fails to compile. A
formatter that reaches for the MCP transport fails to compile. The cost
(a handful of extra `Cargo.toml` files) is far lower than the cost of
discovering a layering violation six months in.

### 2. Strict lints with no bypass

Workspace `[lints]` table denies `unwrap_used`, `expect_used`,
`print_stdout`, `print_stderr`, `dbg_macro`, `todo`, `unimplemented`,
`panic`, `missing_docs`. `unsafe_code` is forbidden outside `plumb-cdp`.
`clippy::disallowed_methods` blocks wall-clock sources in `plumb-core`.

Hooks (`lefthook`) have no skip-flag support. There is no
`SKIP_VALIDATION` env var. `just validate` mirrors CI exactly.

**Rationale.** Strictness only works if the escape hatch doesn't exist.
The day we add one is the day it starts getting used, and from there
it's a ratchet the wrong direction.

### 3. `.agents/` portable library, `.claude/` runtime, `.claude/rules` ⇄ `.agents/rules` symlinks

Shared rules and skills live in `.agents/` (tool-agnostic,
AGENTS.md-compatible). Claude Code's runtime (settings, hooks, subagent
specs) lives in `.claude/`. `.claude/rules` and `.claude/skills` are
symlinks into `.agents/`.

**Rationale.** Matches every other repo in the fleet. Lets future AI
tools (Codex, Cursor, Windsurf) point at `.agents/` without caring what
Claude Code does. Windows users need `git config --global core.symlinks
true` plus Developer Mode — documented in CONTRIBUTING.md.

### 4. justfile over cargo aliases

Plumb's task surface lives in `/justfile`. `.cargo/config.toml` has no
`[alias]` entries.

**Rationale.** Keeping two task runners in sync is churn. Picking one
and enforcing it eliminates the question.

### 5. release-please + cargo-dist (config only, no workflow yet)

`release-please-config.json` and `dist-workspace.toml` ship in the
founding commit. The actual `release-please.yml` / `release.yml`
workflows land later, once crate names are reserved on crates.io and
the Homebrew tap / npm org exist.

**Rationale.** Landing release plumbing before the infrastructure is
ready produces a broken CI badge and a "why doesn't this work" thread
on day one.

### 6. Conventional Commits enforced by shell, not Node

`scripts/validate_conventional_commit.sh` is a plain bash script. No
`commitlint`, no Husky, no Node dependency in a Rust repo.

**Rationale.** Plumb has no Node.js in its toolchain. Adding one to
validate commit messages is disproportionate.

### 7. Placeholder rule with `#[deprecated]` attribute

The walking-skeleton rule `placeholder/hello-world` is marked
`#[deprecated]` so the compiler warns at its registration site until a
real rule replaces it. It's also tagged `#[doc(hidden)]` so it doesn't
appear in rustdoc.

**Rationale.** A TODO in a comment is easy to forget. A deprecation
warning is not.

### 8. rmcp 0.2.x from day one

`plumb-mcp` uses the official [`rmcp`](https://crates.io/crates/rmcp)
crate with the `#[tool_router]` / `#[tool]` macros. Protocol tests spawn
the real `plumb mcp` subprocess and speak JSON-RPC 2.0 over stdio.

**Rationale.** rmcp is Anthropic's canonical Rust MCP SDK. Rolling a
hand-written JSON-RPC loop trades a dependency for a protocol-drift
risk — the SDK will track new MCP spec revisions, a fork would not.
Tools live under `impl PlumbServer` and get their description directly
in the attribute; the pattern is documented in
`.agents/rules/mcp-tool-patterns.md`.

## Consequences

- Strictness creates short-term friction (more lints to satisfy) in
  exchange for long-term velocity (fewer regressions to debug).
- The `.agents/` / `.claude/` layout requires Windows contributors to
  enable symlinks before cloning.
- The hand-rolled MCP server is temporary. The migration to rmcp needs
  its own tracking issue and ADR when the crate's API stabilizes.

## References

- `docs/local/prd.md` — functional spec.
- `.agents/rules/` — every rule enforced day one.
- `CONTRIBUTING.md` — the developer-facing version of most of this.
