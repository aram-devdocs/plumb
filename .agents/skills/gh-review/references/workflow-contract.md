# gh-review workflow contract

This skill mirrors `.github/workflows/claude-code-review.yml`.

## Inputs

- PR number (`--pr <N>`), or
- local diff range (`--local-diff main...HEAD`).
- Optional reviewer instructions (`--instructions "..."`).

## Required phases

1. Context gathering — fetch PR metadata, diff, file list.
2. Crate / area classification.
3. Blocker scan on added lines.
4. Warning scan (TODOs, allow-without-rationale, AI-flavored docs prose).
5. Rule / MCP tool / config-schema contract checks.
6. Quality assessment summary.
7. Scope verification.
8. Structured output.

## File buckets

- `plumb-core` — rule engine, types, determinism.
- `plumb-format` — formatters (pretty, JSON, SARIF, MCP-compact).
- `plumb-cdp` — only crate permitted `unsafe`.
- `plumb-config` — figment loader + schemars schema emission.
- `plumb-mcp` — rmcp stdio server.
- `plumb-cli` — binary entry; only crate permitted stdout/stderr.
- `xtask` — dev tooling (schema emission, pre-release checks, runbook validation).
- `docs` — mdBook source + ADRs + runbook specs.
- `ci` — `.github/workflows/*`, `lefthook.yml`, `justfile`.
- `deps` — `Cargo.toml`, `Cargo.lock`, `deny.toml`.

## Blockers (Rust)

- New `unsafe` outside `plumb-cdp`.
- New `unwrap` / `expect` / `panic!` in a library crate (`plumb-core`, `plumb-format`, `plumb-cdp`, `plumb-config`, `plumb-mcp`). `anyhow`/`expect` remain permitted in `plumb-cli::main` and in tests.
- New `println!` / `eprintln!` outside `plumb-cli` (use `tracing`).
- New `SystemTime::now` / `Instant::now` / `std::env::temp_dir` in `plumb-core`.
- New `todo!` / `unimplemented!` / `dbg!` anywhere.
- New `HashMap` / `HashSet` in an observable-output path (use `IndexMap` / `IndexSet`).
- New rule added without a sibling golden test + `docs/src/rules/<slug>.md` + entry in `register_builtin`.
- New MCP tool added without a protocol test in `crates/plumb-cli/tests/mcp_stdio.rs`.
- Config shape changed without regenerating `schemas/plumb.toml.json` (`cargo xtask schema`).
- New direct dep with a forbidden license (GPL / AGPL / LGPL — `cargo-deny` will catch).
- Binary-size regression ≥ 25 MiB (CI `size-guard` job fails).

## Warnings

- Missing `# Errors` section on a public fallible fn.
- `#[allow(...)]` without a one-line rationale comment above it.
- `TODO` comment without a `#<issue>` reference.
- Missing rustdoc on a new public item.
- AI-flavored prose in `docs/src/**` (`delve`, `tapestry`, `leverage`, `seamless`, etc.).
- Stilted phrasing (`In conclusion`, `It's important to note`, `Dive in`).
- New dep with multi-major-version duplication in the tree.

## Required output sections

1. `### Code review summary`
2. `#### Blockers`
3. `#### Warnings`
4. `#### Architecture compliance`
5. `#### Anti-pattern scan`
6. `#### Quality assessment`
7. `#### Scope check`
8. Final `**Verdict:** APPROVE|REQUEST_CHANGES|BLOCK`

The verdict format matches the review-gate-validator hook expectation.
