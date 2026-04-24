# docs/runbooks — runbook specs

See `/AGENTS.md` and `docs/AGENTS.md`. This file scopes to `docs/runbooks/`.

## What lives here

YAML files consumed by `/gh-runbook`. Each spec describes one parent
tracking issue, its child workstreams grouped into sequential batches,
and a phase-gate criterion. The schema is at `schemas/runbook-spec.json`
and documented in `.agents/skills/gh-runbook/references/spec-format.md`.

## Contract

- Specs are the source of truth. Never edit generated runbook drafts
  under `.agents/runs/gh-runbook/` (gitignored) — regenerate from the
  spec.
- **Batches are sequential.** Issues INSIDE a batch are parallel-safe.
  If A must land before B, put them in different batches.
- **Slugs unique per spec.** Across phases they can repeat.
- **Every child issue names its reviewers** from Plumb's 10 subagents.
  Default set is spec / quality / architecture / test. Add
  `06-security-auditor` when `plumb-cdp` / `plumb-mcp` / URL / deps
  are touched.

## Workflow

1. Edit (or add) a spec under this directory.
2. Validate: `cargo xtask validate-runbooks`.
3. Generate: `python3 .agents/skills/gh-runbook/scripts/generate_runbook.py docs/runbooks/<spec>.yaml --output-dir .agents/runs/gh-runbook/<phase>/`.
4. Review the generated `summary.md`.
5. Run `bash .agents/runs/gh-runbook/<phase>/create-issues.sh` to create the real GitHub issues.
6. Dispatch `/gh-issue <N>` for each child in the current batch (parallel sessions).

## Anti-patterns

- Committing generated output. The `summary.md`, `create-issues.sh`,
  `manifest.json`, and rendered markdown bodies belong in
  `.agents/runs/gh-runbook/` (gitignored).
- Specs that reference a milestone that doesn't exist in GitHub. Create
  the milestone first (`gh api /repos/<repo>/milestones -f title=…`).
- Intra-batch dependencies. If A depends on B, they belong in separate
  batches and B's batch declares `depends_on_batch`.
