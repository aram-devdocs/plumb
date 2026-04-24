## Target branch

> All PRs target `main`. Plumb has no `dev` branch.

- [x] This PR targets `main`

## Spec

<!-- Link the issue, runbook spec, or ADR this PR implements. -->

Fixes #

## Summary

<!-- 1–3 bullets: what changed and why. -->

-

## Crates touched

- [ ] `plumb-core`
- [ ] `plumb-format`
- [ ] `plumb-cdp`
- [ ] `plumb-config`
- [ ] `plumb-mcp`
- [ ] `plumb-cli`
- [ ] `xtask`
- [ ] `docs/`
- [ ] `.agents/` or `.claude/`
- [ ] `.github/`

## System impact

- [ ] New public API item (needs doc + `# Errors` section if fallible)
- [ ] New MCP tool (needs `tools/list` entry + protocol test)
- [ ] New rule (needs docs page + golden test + `register_builtin` entry)
- [ ] CDP / browser surface change (needs security-auditor review)
- [ ] Config schema change (run `cargo xtask schema` + commit result)
- [ ] Dependency added / bumped (cargo-deny must still pass)
- [ ] Determinism invariant touched (see `.agents/rules/determinism.md`)

## Architectural compliance

- [ ] Layer discipline: `plumb-core` has no internal deps; unsafe only in `plumb-cdp`; `println!`/`eprintln!` only in `plumb-cli`.
- [ ] Error shape: `thiserror`-derived in libs; `anyhow` only in `plumb-cli::main`.
- [ ] No new `unwrap`/`expect`/`panic!` in library crates.
- [ ] No new `SystemTime::now` / `Instant::now` in `plumb-core`.
- [ ] No new `HashMap` in observable output paths (use `IndexMap`).
- [ ] Every `#[allow(...)]` is local and has a one-line rationale.

## Test plan

- [ ] `just validate` passes locally
- [ ] `cargo xtask pre-release` passes (if rule or schema changed)
- [ ] `just determinism-check` passes (3× byte-diff clean)
- [ ] `cargo deny check` passes
- [ ] New/changed behavior has a test (unit, golden snapshot, or integration)

## Documentation

- [ ] Rustdoc added for every new public item
- [ ] `# Errors` section on every new public fallible fn
- [ ] `docs/src/` updated when user-visible behavior changed
- [ ] `docs/src/rules/<category>-<id>.md` written for new rules
- [ ] CHANGELOG updated if user-visible (otherwise release-please handles it)
- [ ] Humanizer skill run on docs changes

## Breaking change?

- [ ] No
- [ ] Yes — describe migration path

{{BREAKING_CHANGES}}

## Checklist

- [ ] Conventional Commits title
- [ ] Branch name: `codex/<primary>-<type>-<slug>`
- [ ] All review gates passed: spec → quality → architecture → test (+ security if triggered)
- [ ] `/gh-review --local-diff main...HEAD` run locally

## Reviewer notes

{{REVIEWER_NOTES}}
