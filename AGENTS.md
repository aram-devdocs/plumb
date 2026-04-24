# AGENTS.md

This file is the tool-agnostic entry point for every AI coding agent working in this repository — Claude Code, Codex, Cursor, Windsurf, and any future assistant that honors the [`AGENTS.md` convention](https://agents.md).

The same content is surfaced to Claude Code via the `CLAUDE.md` symlink at the repo root.

## What Plumb is

Plumb is a deterministic design-system linter for rendered websites. It ships as a single Rust binary with two entry points:

- `plumb lint <url>` — CLI for developers and CI.
- `plumb mcp` — Model Context Protocol server for AI coding agents.

The complete product specification lives at `docs/local/prd.md`. **Read the PRD before making non-trivial changes.** It is the single source of truth for scope, architecture, determinism invariants, output formats, and release targets.

## Repository layout

- `crates/plumb-core/` — rule engine, types, determinism guarantees. No I/O, no async, no wall-clock.
- `crates/plumb-cli/` — clap-based binary. Only crate allowed to print to stdout/stderr.
- `crates/plumb-mcp/` — rmcp stdio server.
- `crates/plumb-cdp/` — Chromium DevTools Protocol driver. Only crate permitted to use `unsafe`.
- `crates/plumb-config/` — config file loading + JSON Schema emission.
- `crates/plumb-format/` — output formatters (pretty, JSON, SARIF, MCP-compact).
- `docs/src/` — mdBook source for <https://plumb.aramhammoudeh.com>.
- `docs/adr/` — architecture decision records.
- `docs/local/` — gitignored scratch space for local-only docs (PRD lives here until extracted).
- `.agents/` — tool-agnostic AI library (rules, skills, role specs).
- `.claude/` — Claude Code runtime (settings, hooks, agents). `.claude/rules` and `.claude/skills` symlink into `.agents/`.

## Read order

1. This file.
2. `docs/local/prd.md` — product requirements document.
3. `.agents/rules/` — project-specific rules (determinism, dependency hierarchy, rule engine patterns, MCP tool patterns, testing, documentation).
4. `.agents/skills/` — reusable skills (humanizer, code review, etc.).
5. `CONTRIBUTING.md` — human contributor guide; covers commit conventions and the dev loop.

## Hard rules

These are non-negotiable and enforced by CI. Violating them is never acceptable.

- **No bypasses.** There is no `SKIP_VALIDATION`, no `--no-verify`. If a check fails, fix the root cause.
- **Determinism first.** `plumb-core` must produce byte-identical output across runs. Never introduce wall-clock time, HashMap iteration order, or any nondeterministic source.
- **Layer discipline.** `plumb-core` depends on nothing project-internal. `plumb-format` depends only on `plumb-core`. `plumb-cdp` owns all `unsafe`. `plumb-cli` is the only crate that may call `println!`/`eprintln!`.
- **No `unwrap`/`expect` in libraries.** Return `Result` with `thiserror`-derived errors. `anyhow` is allowed only in `plumb-cli::main`.
- **No `todo!`/`unimplemented!`/`dbg!` anywhere.** If something is unfinished, open a tracking issue and return a typed error.
- **No legacy code.** `#[deprecated]` items carry a tracking issue and a concrete removal milestone. No commented-out code, no orphan shims, no open-ended `TODO: remove later`. Unused imports, functions, and fields fail CI. Full policy: `.agents/rules/no-legacy-code.md`.
- **Docs must be human.** The `humanizer` skill runs on every docs PR. Avoid AI-tell phrasing ("dive in", "comprehensive", "leverage", "seamless", etc.).

## Workflow expectations

- **TDD for rules.** Write the golden snapshot test first, then implement.
- **Atomic commits.** One logical change per commit. Conventional Commits format — validated by the `commit-msg` hook.
- **Run `just validate` before pushing.** It mirrors CI exactly; if it passes, CI passes.
- **Update the CHANGELOG under `## [Unreleased]`** only for user-visible changes — release-please takes over from PR #3 onward.

## When in doubt

- For architecture questions → `docs/local/prd.md` + `docs/adr/`.
- For commit style → `.agents/rules/` + `CONTRIBUTING.md`.
- For rule authoring → `.agents/rules/rule-engine-patterns.md`.
- For MCP tool authoring → `.agents/rules/mcp-tool-patterns.md`.

If none of those answer your question, ask. Never guess on determinism, layer discipline, or output format.
