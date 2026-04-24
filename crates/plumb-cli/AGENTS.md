# crates/plumb-cli — the `plumb` binary

See `/AGENTS.md` for repo-wide rules. This file scopes to `plumb-cli`.

## Purpose

The only binary in the workspace. Public subcommands: `lint`, `init`,
`explain`, `schema`, `mcp`.

## Unique permissions

This is the ONLY crate allowed to:

- `println!` / `eprintln!` — scoped via `#[allow(clippy::print_stdout)]`
  on the exact `print!` / `println!` sites, not file-wide.
- `anyhow::Error` — `plumb-cli::main` is the only place `anyhow` is
  acceptable. Everywhere else uses `thiserror`-derived enums.
- Wall-clock reads in startup/telemetry (`#![allow(clippy::disallowed_methods)]`
  at the crate root, since `plumb-core` forbids them).

## Exit codes (PRD §13.3)

| Code | Meaning |
|------|---------|
| 0 | No violations. |
| 1 | One or more `error`-severity violations. |
| 2 | CLI / infrastructure failure (bad URL, missing config, driver error). |
| 3 | Only `warning`-severity violations. |

## Non-negotiable invariants

- `#![forbid(unsafe_code)]`.
- `#![allow(unreachable_pub)]` — the binary crate's public items are
  never imported externally, and `redundant_pub_crate` (clippy) would
  otherwise conflict.
- `miette::set_panic_hook()` is called before the runtime starts, so
  errors print with spans when they bubble out of `main`.

## Subcommand contract

- `lint <url>` — drives the pipeline; exit-code logic in `commands::lint::exit_code_for`.
- `init [--force]` — writes `examples/plumb.toml` to CWD; refuses to overwrite without `--force`.
- `explain <rule>` — reads `docs/src/rules/<slug>.md`.
- `schema` — delegates to `plumb_config::emit_schema`.
- `mcp` — blocks in `plumb_mcp::run_stdio` until EOF.

## Depends on

Every library crate + `clap`, `tokio`, `anyhow`, `miette`, `tracing`, `tracing-subscriber`, `serde_json`.

## Anti-patterns

- Business logic in `main.rs` — subcommand bodies live under `src/commands/<name>.rs`.
- Hand-rolled config reads (use `plumb_config::load`) or formatter output (use `plumb_format::*`).
