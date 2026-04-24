# Contributing to Plumb

Thanks for your interest in contributing. This document covers prerequisites, the development loop, and the process for getting changes merged.

## Prerequisites

- **Rust 1.95+** via [rustup](https://rustup.rs/). The `rust-toolchain.toml` pins the exact version; rustup will install it automatically when you run any cargo command.
- **[`just`](https://github.com/casey/just)** — task runner. Install with `cargo install just` or `brew install just`.
- **[`lefthook`](https://github.com/evilmartians/lefthook)** — git hooks. Install with `brew install lefthook` (macOS/Linux) or `scoop install lefthook` (Windows).
- **Chromium** — required at runtime by the CDP driver (later PRs). Not needed for the walking skeleton.

### Windows notes

Plumb uses symlinks (`CLAUDE.md` → `AGENTS.md`, `.claude/rules` → `.agents/rules`, `.claude/skills` → `.agents/skills`). To clone cleanly on Windows:

```powershell
git config --global core.symlinks true
```

You also need [Developer Mode](https://learn.microsoft.com/en-us/windows/apps/get-started/enable-your-device-for-development) enabled or an Administrator shell for `git clone` to materialize symlinks.

## Development loop

```bash
git clone https://github.com/aram-devdocs/plumb
cd plumb
just setup          # install hooks, verify toolchain, check Chromium
just check          # fmt + clippy, fails on any warning
just test           # full test suite via cargo-nextest
just validate       # full pre-push mirror — matches CI exactly
```

`just` with no arguments lists every available target.

## Commit conventions

All commits follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/). The allowed types are:

`feat` · `fix` · `perf` · `refactor` · `docs` · `test` · `build` · `ci` · `chore` · `style` · `revert`

Allowed scopes match the crate or area: `core`, `cli`, `mcp`, `cdp`, `config`, `format`, `docs`, `ci`, or a rule id like `spacing/hard-coded-gap`.

The `commit-msg` hook validates this locally. The `pr-title` GitHub workflow validates it on every pull request.

## Strictness — no bypasses

Plumb enforces strict quality gates end-to-end. These gates **have no opt-out**:

- `cargo fmt --check` must pass.
- `cargo clippy -- -D warnings` must pass (clippy pedantic + custom deny list).
- Every library crate has `#![forbid(unsafe_code)]`, `#![deny(missing_docs)]`, and denies `unwrap_used`/`expect_used`/`print_stdout`/`print_stderr`/`dbg_macro`/`todo`/`unimplemented`.
- `plumb-core` additionally forbids `std::time::SystemTime::now`/`Instant::now` (determinism).
- `cargo-deny` checks licenses, advisories, and banned crates on every CI run.
- Binary size must stay under 25 MiB.
- Output must be deterministic — the `determinism` CI job runs a fixture three times and byte-diffs the output.
- `cargo-audit` runs daily against the advisory database.

There is no `SKIP_VALIDATION` env var, no `--no-verify` support, and no escape hatch in the pre-push hook. If a check fails, fix the cause. If a check is genuinely wrong, open an RFC issue proposing to change it.

## Dependency policy

- `Cargo.lock` is committed. Never ignore it.
- `[patch.crates-io]` is forbidden without an ADR documenting the justification and removal plan.
- New direct dependencies require justification in the PR description. Prefer crates that are already in the tree.
- `cargo-deny` blocks GPL, AGPL, and LGPL licenses. Dual-licensed dependencies are fine if the compatible half is MIT, Apache-2.0, BSD, ISC, Unicode, Zlib, or MPL-2.0.

## Adding a rule

See `.agents/rules/rule-engine-patterns.md`. The short version:

1. Create `crates/plumb-core/src/rules/<category>/<id>.rs` implementing `Rule`.
2. Register it in `crates/plumb-core/src/rules/mod.rs::register_builtin`.
3. Add a golden snapshot test under `crates/plumb-core/tests/`.
4. Document it at `docs/src/rules/<category>-<id>.md` — this is what `plumb explain` reads.
5. Wire `doc_url` in the rule to point at `https://plumb.aramhammoudeh.com/rules/<category>-<id>`.

## Adding an MCP tool

See `.agents/rules/mcp-tool-patterns.md`. Tools live in `crates/plumb-mcp/src/lib.rs` under the `#[tool_router]` impl.

## Pull request process

1. Fork or branch, make your changes, push.
2. Open a PR. The title must follow Conventional Commits.
3. Fill out the PR template — especially the **Test plan** and **Breaking change** sections.
4. CI must be green: `preflight`, `test` (Linux/macOS/Windows), `msrv`, `determinism`, `deny`, `size-guard`, `docs`.
5. One approving review from a `CODEOWNERS` entry.
6. Maintainer merges with **Squash and merge**. The final commit message must still follow Conventional Commits — release-please depends on it.

## Release process

Releases are cut by `release-please` from conventional commits on `main`. Maintainers only — see `docs/adr/0001-bootstrap-conventions.md` for details.
