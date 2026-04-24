# crates/plumb-core — rule engine core

See `/AGENTS.md` for repo-wide rules. This file scopes to `plumb-core`.

## Purpose

The deterministic rule engine. Owns: `Rule` trait, `Violation` + `Fix`
+ `Severity` + `Confidence` + `Rect` + `ViewportKey` types, `Config`
schema, `PlumbSnapshot` + `SnapshotCtx`, `register_builtin`, `run`.

## Non-negotiable invariants

- `#![forbid(unsafe_code)]` — absolute. If you think you need `unsafe`,
  you don't; `plumb-cdp` is the only crate allowed it.
- No wall-clock: `SystemTime::now`, `Instant::now`, `std::env::temp_dir`
  are blocked by `clippy::disallowed-methods`.
- No `println!`/`eprintln!`/`dbg!`/`todo!`/`unimplemented!` anywhere.
- No `unwrap`/`expect`/`panic!` in non-test code — return `Result<_, E>`
  with a `thiserror::Error` variant.
- Observable output uses `IndexMap`, never `HashMap`.
- Sort key for violation output: `(rule_id, viewport, selector, dom_order)`.
  Never reorder.
- No internal deps. `plumb-core`'s `Cargo.toml` imports nothing from
  `crates/plumb-*/`.

## Rule authoring path

See `.agents/rules/rule-engine-patterns.md` for the full flow.
Summary:

1. `crates/plumb-core/src/rules/<category>/<id>.rs` — impl `Rule`.
2. `crates/plumb-core/tests/golden_<category>_<id>.rs` — insta snapshot.
3. `docs/src/rules/<category>-<id>.md` — rule docs (required, `cargo
   xtask sync-rules-index` enforces).
4. Register in `crates/plumb-core/src/rules/mod.rs::register_builtin`.

## Anti-patterns

- Adding an async fn. `plumb-core` is sync by contract; async belongs
  in `plumb-cdp` or `plumb-mcp`.
- Mutating shared state from a `Rule::check`. Rules are pure.
- Reading a config value during `Rule::check` from anywhere other than
  the `Config` argument.
