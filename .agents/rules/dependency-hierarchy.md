# Rule: Dependency hierarchy

The workspace has a strict layering. Violating it breaks the determinism
invariants and the `unsafe`-isolation guarantee.

## Layers (bottom to top)

1. **`plumb-core`** — types, rule engine, `Config`, snapshot representation.
   Depends on: nothing internal. External deps are pure (serde, schemars,
   indexmap, palette). No I/O, no async, no wall-clock.
2. **`plumb-format`** — output formatters. Depends on: `plumb-core`.
3. **`plumb-cdp`** — CDP driver. Depends on: `plumb-core`. Owns every
   `unsafe` block in the workspace.
4. **`plumb-config`** — config loading and schema emission. Depends on:
   `plumb-core`.
5. **`plumb-mcp`** — MCP server. Depends on: `plumb-core`, `plumb-format`.
6. **`plumb-cli`** — the `plumb` binary. Depends on: every other crate.
   Only crate allowed to print to stdout/stderr and to use `anyhow`.

## What's banned

- `plumb-core` depending on anything else internal.
- `plumb-format` depending on `plumb-cdp` / `plumb-mcp` / `plumb-cli`.
- `plumb-cdp` depending on anything except `plumb-core`.
- Cyclic or diagonal dependencies.

## What's required

- Every library crate has `#![forbid(unsafe_code)]` except `plumb-cdp`
  (which has `#![deny(unsafe_op_in_unsafe_fn)]` and documents each
  `unsafe` block with `// SAFETY:`).
- `println!` / `eprintln!` live only in `plumb-cli`. Libraries use
  `tracing` macros.

## How it's enforced

- `cargo-deny bans` blocks cycles and unexpected deps.
- Workspace `[lints.rust].unsafe_code = "forbid"`; `plumb-cdp` overrides.
- Workspace `[lints.clippy].print_stdout/print_stderr = "deny"`; the CLI
  scopes `#[allow(clippy::print_stdout)]` to the exact `print!` / `println!`
  call.
