# crates/plumb-codegen — source-tree token inference

See `/AGENTS.md` for repo-wide rules. This file scopes to `plumb-codegen`.

## Purpose

Walks a user's project tree and infers a starter `plumb.toml` from the
design-token sources it finds: CSS custom properties (`:root { --foo:
… }`), Tailwind config filenames, and DTCG token JSON files. Public
surface: `infer_config`, `render_toml`, `InferredConfig`, `CodegenError`.

The crate is consumed by `plumb-cli` only. It does not drive the
linter; it produces a starter config the user edits.

## Non-negotiable invariants

- `#![forbid(unsafe_code)]`.
- No `unwrap`/`expect`/`panic!` — every fallible path returns a
  `CodegenError` (thiserror-derived).
- No `println!`/`eprintln!`. Diagnostic noise routes through `tracing`.
- No wall-clock reads. The walker is a pure function of `(source_dir,
  filesystem state)`.
- File walks are deterministic: directory entries are sorted by their
  canonical UTF-8 path before recursion.
- Observable output uses `IndexMap` (insertion order) and sorted
  numeric scales.
- No JS evaluation. The crate notes the presence of a Tailwind config
  file but does not spawn Node — the existing
  `plumb_config::merge_tailwind` adapter handles that on demand from
  the CLI when the user runs the linter, not at `init` time.

## Depends on

- `plumb-core` — `Config` type and the spec sub-structs.
- `plumb-config` — `scrape_css_properties`, `merge_dtcg`, the typed
  error variants we re-wrap.

Never depends on `plumb-cdp`, `plumb-format`, or `plumb-mcp`.

## Anti-patterns

- Walking `node_modules` or build output. The walker hard-skips them.
- Reading network resources to resolve aliases. Inputs are local.
- Mutating the source tree. The crate is read-only on disk.
