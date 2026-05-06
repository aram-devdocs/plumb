# crates/plumb-e2e — end-to-end harness (dev-only)

See `/AGENTS.md` for repo-wide rules. This file scopes to `plumb-e2e`.

## Purpose

Drives the locally built `plumb` binary against the framework
fixtures under `e2e-sites/`. Builds each fixture, serves it on a
loopback port via `tiny_http`, runs `plumb lint` three times, and
asserts the violation breakdown matches the fixture's `expected.json`
plus byte-equality across runs.

## Non-negotiable invariants

- `#![forbid(unsafe_code)]`.
- `publish = false` — dev-only, not a crates.io artifact.
- Excluded from `default-members` so `cargo build` does not pull it in.
- Library code (`src/lib.rs`, `src/runner.rs`, `src/server.rs`,
  `src/expected.rs`, `src/sites.rs`, `src/workspace.rs`) follows the
  workspace `unwrap_used` / `expect_used` deny. Tests (`tests/`,
  `#[cfg(test)] mod tests`) may use `expect`.
- The static server MUST refuse `..` path traversal and MUST NOT
  follow symlinks.
- Determinism: `run_site` asserts byte-identical output across
  `determinism_runs` runs (default 3, matching `just determinism-check`).

## Depends on

- External: `tiny_http`, `clap`, `serde`, `serde_json`, `indexmap`,
  `anyhow`, `thiserror`, `tracing`, `tracing-subscriber`.
- `plumb-cli` is exercised through its on-disk binary, never linked.

## Anti-patterns

- Linking `plumb-core` or `plumb-cli` as a library dep — the harness
  must mirror what end users run, which is the binary.
- Asserting on the full violations array. Only the target-rule subset
  declared by `expected.json` is asserted; non-target rules are
  tolerated.
