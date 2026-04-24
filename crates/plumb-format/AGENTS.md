# crates/plumb-format — output formatters

See `/AGENTS.md` for repo-wide rules. This file scopes to `plumb-format`.

## Purpose

Pure output formatters. Public surface: `pretty`, `json`, `sarif`,
`mcp_compact`. Consumed by `plumb-cli` and `plumb-mcp`.

## Non-negotiable invariants

- `#![forbid(unsafe_code)]`.
- Every formatter is a pure fn: `&[Violation]` in, `String` (or
  `(String, serde_json::Value)` for mcp_compact) out. No env, no
  clock, no filesystem, no network.
- Deterministic across runs given the same inputs. No `HashMap`
  iteration in output; use `indexmap` when ordering isn't already
  guaranteed by the input slice.
- No `unwrap`/`expect` — serialization errors bubble as `Result<String,
  serde_json::Error>` (pretty returns `String` because it never fails).

## Depends on

- `plumb-core` (types only).

## Anti-patterns

- Reading a file to fetch rule metadata. `Violation::doc_url` carries
  the link; if the formatter needs richer metadata, extend `Violation`
  in `plumb-core`, not here.
- Writing to stdout/stderr directly. Formatters return strings;
  `plumb-cli` writes.
