---
name: gh-runbook
description: Convert a Plumb runbook spec (YAML) into a parent tracking issue plus grouped child workstream issues organized into sequential batches with phase gates. Generates markdown drafts and an idempotent `gh issue create` script; never creates GitHub issues without explicit approval.
user_invocable: true
---

# /gh-runbook — spec-driven runbook generator

Turn a PRD-phase spec into a parent tracking issue + N child workstream issues + an idempotent `gh issue create` script. Target repo: `aram-devdocs/plumb`.

Use `/gh-issue` afterwards to drive each child issue through the delivery lifecycle.

## Usage

```bash
/gh-runbook docs/runbooks/phase-1-spec.yaml --dry-run
/gh-runbook docs/runbooks/phase-1-spec.yaml --output-dir .agents/runs/gh-runbook/phase-1/
/gh-runbook docs/runbooks/roadmap-spec.yaml --output-dir .agents/runs/gh-runbook/roadmap/
```

The underlying script:

```bash
python3 .agents/skills/gh-runbook/scripts/generate_runbook.py \
    docs/runbooks/phase-1-spec.yaml \
    --output-dir .agents/runs/gh-runbook/phase-1/ \
    --dry-run
```

## Workflow

1. Read the spec YAML. Validate it against `schemas/runbook-spec.json` (via `cargo xtask validate-runbooks` if available, or via the generator's `--validate-only` mode).
2. Render the parent issue body from `assets/parent-issue-template.md` with all batches laid out as sub-sections.
3. Render one child issue body per child from `assets/child-issue-template.md`.
4. Emit `manifest.json`, `summary.md`, `create-issues.sh` in the output directory.
5. Review `summary.md`.
6. Only execute `create-issues.sh` after approval — it creates the real GitHub issues and updates the parent body with real issue numbers.

## Spec schema

See `references/spec-format.md` for the full YAML grammar. A minimal skeleton:

```yaml
schema: https://plumb.dev/schemas/runbook-spec.json
name: "Phase N — <headline>"
phase_number: N
repo: aram-devdocs/plumb

parent:
  title: "[RUNBOOK] Phase N — <headline>"
  labels: [phase-N, kind:rfc]
  milestone: "v0.N"
  summary: |
    <1–3 paragraph description>
  acceptance_criteria:
    - <objective gate for unlocking next phase>
  related_prd_sections: ["§10.3"]

batches:
  - id: "NA"
    description: "Foundation — independent, parallel"
    parallel: true
    issues:
      - slug: cdp-chromium-launch
        title: "feat(cdp): Chromium detection, launch, BYO-Chromium"
        labels: ["area:cdp", "kind:feat", "phase-N"]
        crate: plumb-cdp
        effort: M
        prd_refs: ["§10.6"]
        summary: |
          <purpose>
        acceptance_criteria:
          - bullet 1
        reviewers: ["02-spec-reviewer", "03-code-quality-reviewer", "05-architecture-validator", "06-security-auditor"]

phase_gate:
  criterion: |
    `just validate` passes AND <phase-specific check>.
  unblocks: "phase-(N+1)"
```

## Batches and gates

- **Issues inside a batch** are parallel-dispatch safe — disjoint file scope, no cross-dep.
- **Batches are sequential** — a batch cannot start until its `depends_on_batch` chain is merged.
- **Phase gate** is a single objective criterion that must hold before the next phase unlocks.

## Outputs

Each run writes:

| File | Purpose |
|------|---------|
| `00-parent-<slug>.md` | Parent issue body with batch sub-sections + phase-gate text. |
| `NN-<batch>-<slug>.md` | One per child. Frontmatter: `title`, `labels`, `milestone`, `batch`, `reviewers`. |
| `manifest.json` | Full metadata keyed by slug. |
| `summary.md` | Human-readable TOC. |
| `create-issues.sh` | Idempotent `gh issue create` + parent-body rewrite. |

Run `create-issues.sh --dry-run` to see the planned `gh` commands without executing them.

## Rules

- **Dry-run is the default.** The generator emits markdown + a script; it never touches GitHub directly.
- **Output lives under `.agents/runs/gh-runbook/<phase>/`** — gitignored. Specs go under `docs/runbooks/` and ARE committed.
- **Re-running is safe**: the generator refuses to overwrite an existing output dir without `--force`. `create-issues.sh` is idempotent via the manifest.
- **No intra-batch dependencies.** If A must land before B, they go in different batches.
- **Every child issue names its reviewers.** Default is the four Plumb gates. Add `06-security-auditor` for `plumb-cdp` / `plumb-mcp` / URL / dep changes.
- **Labels include `phase-N`** so runbook progress is trivially queryable: `gh issue list --repo aram-devdocs/plumb --label phase-1 --state open`.
