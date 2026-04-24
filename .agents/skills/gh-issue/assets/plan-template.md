# Plan: Issue #<primary> — <title>

## Issue summary

<one to three sentence summary of what the issue asks for>

## Acceptance criteria

- [ ] <criterion 1>
- [ ] <criterion 2>
- [ ] <criterion 3>

## Affected crates

| Crate | Role | Files |
|-------|------|-------|
| `plumb-core` | rule engine / types | `crates/plumb-core/src/<module>.rs` |
| `plumb-cli` | binary entry | `crates/plumb-cli/src/<module>.rs` |

Layer discipline must hold: `plumb-core` imports nothing internal; `plumb-format` / `plumb-cdp` / `plumb-config` depend only on `plumb-core`; `plumb-mcp` depends on `plumb-core` + `plumb-format`; `plumb-cli` sits on top.

## Implementation approach

<describe the approach. Cite `.agents/rules/*` or `AGENTS.md` scopes you
are operating under. Call out determinism invariants (no wall-clock in
plumb-core, no HashMap in observable output, deterministic sort key).
Call out error style (thiserror for libs, anyhow only in plumb-cli::main).>

## Subagent dispatch plan

| Agent | Scope | Files |
|-------|-------|-------|
| `01-implementer` | <description> | `<files>` |

Parallel batches (when applicable):
- Batch 1: <agent A>, <agent B> (independent — disjoint files)
- Batch 2: <agent C> (depends on batch 1 output)

Specialized agents — use when applicable:
- `08-rule-author` for new rules under `crates/plumb-core/src/rules/`.
- `09-mcp-tool-author` for new `#[tool]` methods on `PlumbServer`.
- `10-quick-fix` only for trivial work that needs no test.
- `07-debugger` for root-cause diagnosis before fixing a failure.

## Review gates

- [ ] `02-spec-reviewer` (always required)
- [ ] `03-code-quality-reviewer` (always required, runs after spec passes)
- [ ] `05-architecture-validator` (always required)
- [ ] `04-test-runner` (always required)
- [ ] `06-security-auditor` (required when: `plumb-cdp` / `plumb-mcp` / URL or config parsing / dependency-graph changes)

Security-auditor trigger reason: <explain or "not required">

## Verification

```bash
just validate
```

Narrow iteration loop while implementing:

```bash
cargo fmt --all -- --check && \
  cargo clippy -p <crate> -- -D warnings && \
  cargo nextest run -p <crate>
```

## Branch

`codex/<primary>-<type>-<slug>`

PR target: `main`

PR title: `<type>(<scope>): <imperative description>` (Conventional Commits)

## Notes

<any architectural decisions, risks, or caveats. If the docs subtree is
touched, note that the humanizer skill must run before review.>
