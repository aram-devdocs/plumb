# Runbook spec format

A runbook spec is a YAML file that describes one parent tracking issue and its child workstreams, grouped into sequential batches with a phase gate. The canonical schema lives at `schemas/runbook-spec.json`.

## Top-level fields

| Field | Required | Type | Purpose |
|-------|----------|------|---------|
| `schema` | yes | URL | Fixed: `https://plumb.dev/schemas/runbook-spec.json`. |
| `name` | yes | string | Human-readable phase name (≤80 chars). |
| `phase_number` | no | integer | 1–7 for PRD phases, omit for umbrella. |
| `repo` | yes | string | `aram-devdocs/plumb` by default. |
| `parent` | yes | object | Parent tracking issue. |
| `batches` | yes | list | One or more batch objects. |
| `phase_gate` | yes | object | Criterion to unlock the next phase. |

## `parent` object

| Field | Required | Type | Purpose |
|-------|----------|------|---------|
| `title` | yes | string | Issue title. Use `[RUNBOOK]` prefix for phase parents, `[ROADMAP]` for the umbrella. |
| `labels` | yes | list of strings | Must include `phase-N` (or `roadmap` for umbrella) + one of `kind:rfc` / `kind:feat` / `kind:chore`. |
| `milestone` | no | string | GitHub milestone name (`v0.1-phase-1` etc.). Must exist in the repo before `create-issues.sh` runs. |
| `summary` | yes | string (multi-line) | 1–3 paragraphs; the parent issue's Summary section. |
| `acceptance_criteria` | yes | list of strings | Criteria that flip the parent to closed. |
| `related_prd_sections` | no | list of strings | Citations like `§10.3`. |

## `batches` — list of batch objects

| Field | Required | Type | Purpose |
|-------|----------|------|---------|
| `id` | yes | string | `NA`, `NB`, etc. N = phase number; letter = batch order. |
| `description` | yes | string | One-line headline. |
| `parallel` | yes | boolean | True if issues in this batch may run concurrently. |
| `depends_on_batch` | no | string or list | Batch IDs that must complete before this one starts. |
| `issues` | yes | list of issue objects | 1–5 child issues. |

## `issues` — list of issue objects

| Field | Required | Type | Purpose |
|-------|----------|------|---------|
| `slug` | yes | string | Kebab-case short id, unique within the spec. |
| `title` | yes | string | Conventional Commits: `<type>(<scope>): <description>`. |
| `labels` | yes | list of strings | Must include `phase-N` + `area:<crate>` + `kind:<type>`. |
| `crate` | no | string | Primary target (`plumb-core`, `plumb-cdp`, …). |
| `effort` | yes | one of `XS` `S` `M` `L` `XL` | Rough sizing. S ≈ 1 day, M ≈ 3 days, L ≈ 1 week. |
| `prd_refs` | no | list of strings | PRD section citations. |
| `summary` | yes | string | 1–3 sentence purpose. |
| `acceptance_criteria` | yes | list of strings | Specific, checkable. |
| `dependencies` | no | list of strings | Outbound deps (other slugs or external prerequisites). |
| `reviewers` | yes | list of strings | Subagent names — e.g. `02-spec-reviewer`. |
| `suggested_delivery` | no | list of strings | Usually `["gh-issue"]`. Can include skill names. |

## `phase_gate` object

| Field | Required | Type | Purpose |
|-------|----------|------|---------|
| `criterion` | yes | string (multi-line) | Objective check — `just validate` plus phase-specific assertions. |
| `unblocks` | no | string | Next phase id (`phase-2`, or `"-"` for the terminal phase). |

## Rules

- **Intra-batch dependencies are forbidden.** If task A must land before task B, they go in separate batches and B's batch declares `depends_on_batch: "A's batch id"`.
- **A batch's `parallel: true` means the issues inside it can be dispatched in parallel `/gh-issue` sessions**; the batch as a whole still gates on everything before it.
- **Milestones must exist** in the GitHub repo before `create-issues.sh` runs. Create them via `gh api /repos/<repo>/milestones` or the UI.
- **Slug uniqueness is enforced within a spec.** Across specs, slugs can repeat (they'd be disambiguated by the phase number in the full issue title).
- **Every reviewer name must be a real subagent** under `.claude/agents/*.md`. The validator checks this.

## Validation

```bash
# via xtask (preferred — single command for the whole runbook tree)
cargo xtask validate-runbooks

# per-spec (inside the generator)
python3 .agents/skills/gh-runbook/scripts/generate_runbook.py docs/runbooks/phase-1-spec.yaml --validate-only
```

Both validate against `schemas/runbook-spec.json`.
