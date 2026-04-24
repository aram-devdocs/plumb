# crates/plumb-config — config loading + schema emission

See `/AGENTS.md` for repo-wide rules. This file scopes to `plumb-config`.

## Purpose

Loads `plumb.toml` (or `.json`, `.yaml`) via `figment`; emits the
canonical JSON Schema via `schemars`. Public surface: `load`,
`emit_schema`, `ConfigError`.

## Non-negotiable invariants

- `#![forbid(unsafe_code)]`.
- `Config` (defined in `plumb-core`) uses `#[serde(deny_unknown_fields)]`
  at every boundary nested struct. A typo in `plumb.toml` is an error,
  not a silent drop.
- Schema committed at `schemas/plumb.toml.json`; `cargo xtask
  pre-release` enforces it matches the emitted schema.
- Error surface: `ConfigError` is `thiserror`-derived; no `unwrap`,
  no `panic!`.

## Depends on

- `plumb-core` (the `Config` type lives there).
- `figment` (toml + yaml + json + env features).
- `schemars`.

## Anti-patterns

- Adding a parser for a config format that isn't TOML / YAML / JSON.
  If a user wants a different format, they convert upstream.
- Reading env vars inside `load`. `figment` composes env → file layers
  at the call site; the loader here doesn't decide env precedence.
- Emitting a schema that drifts from `Config`. Always regenerate via
  `cargo xtask schema` after any `Config` change.
