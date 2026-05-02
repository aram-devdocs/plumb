# xtask — developer task runner

See `/AGENTS.md` for repo-wide rules. This file scopes to `xtask`.

## Purpose

Dev-only Rust tasks that benefit from type-safety and avoiding shell
quoting. Not published. Invoked via the `cargo xtask` alias in
`.cargo/config.toml`.

## Subcommands

- `schema` — emit the JSON Schema for `plumb.toml` to `schemas/plumb.toml.json`.
- `sync-rules-index` — verify every `register_builtin` rule has a matching `docs/src/rules/<slug>.md`.
- `validate-runbooks` — validate every `docs/runbooks/*.yaml` against `schemas/runbook-spec.json` (delegates to the Python generator's `--validate-only`).
- `validate-landing-page` — verify the docs landing page uses checked-in demo assets, rejects remote embeds, and keeps install CTA targets valid.
- `validate-release-readiness-kits` — verify the checked-in offline local-kit manifest, required kit set, reuse metadata, and offline/deterministic content rules.
- `pre-release` — chains the above checks + schema-currency check.

## Non-negotiable invariants

- `#![forbid(unsafe_code)]`.
- `publish = false` — not a crates.io artifact.
- Runs MUST be idempotent. Re-running a subcommand on an unchanged
  workspace is a no-op.
- Keep deps lean. Do not add `serde_yaml`, `jsonschema`, or other heavy
  deps for tasks that delegate cleanly to the Python generator.

## Depends on

- `plumb-core`, `plumb-config` (internal).
- `anyhow`, `clap` (external).

## Anti-patterns

- Shell-outs for work that `plumb-core` / `plumb-config` already do in
  Rust (e.g. emitting schema by running the CLI rather than calling
  `plumb_config::emit_schema` directly).
- Silent success. Every subcommand prints a `▸` line on the happy path
  so CI logs show what ran.
