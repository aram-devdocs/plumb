---
name: subagent-driven-development
description: Execute implementation plans in Plumb via fresh subagents per task, parallel batches for independent work, and a 4-gate review chain (spec → code-quality → architecture → test). Use when a plan has multiple independent tasks and you want to stay in this session.
user_invocable: true
---

# Subagent-driven development

Execute a plan by dispatching a fresh subagent per task. After each task, run the Plumb review-gate chain. For independent tasks, dispatch in parallel batches for maximum throughput.

**Core principle:** fresh subagent per task + parallel batches for independent work + ordered review gates = high quality, fast iteration.

## When to use

- You have a plan with ≥2 tasks.
- Tasks are mostly independent (different crates, no shared edits, no producer-consumer edges).
- You want to stay in the current session (no context switch).

If tasks are tightly coupled or the plan needs brainstorming, execute manually or use `/gh-issue` with a single primary.

## Process overview

```
Per task:
  Dispatch 01-implementer → self-review → dispatch 02-spec-reviewer → (fixes loop)
    → dispatch 03-code-quality-reviewer → (fixes loop)
    → dispatch 05-architecture-validator → (fixes loop)
    → dispatch 04-test-runner → (fixes loop)
    → (if CDP/MCP/URL/dep touched) dispatch 06-security-auditor in parallel with any of the above
  Mark task complete.

After all tasks:
  Dispatch a final 04-test-runner pass on the full changeset.
  Run `just validate` + `cargo xtask pre-release`.
```

## Parallel batch workflow

### Step 1 — task decomposition

For each task, identify:

- **Target files** — which files will be created / modified.
- **Target crates** — which of `plumb-core`, `plumb-format`, `plumb-cdp`, `plumb-config`, `plumb-mcp`, `plumb-cli`, `xtask` are touched.
- **Dependencies** — what public items does the task import?
- **Outputs** — what does the task produce that another might consume (new public API, new MCP tool schema, new config field)?

### Step 2 — dependency analysis

Classify each task pair as INDEPENDENT or DEPENDENT.

**INDEPENDENT (safe to parallelize)** — every one of these must hold:

- Different target files (no shared edits).
- Different crates OR no upward dependency change.
- No change to a shared public API consumed by another task.
- No producer-consumer relationship (one task's public output isn't another's input).
- Can be merged without conflicts.

**DEPENDENT (must sequence)** — any of these triggers sequencing:

- Same target files.
- One task changes a `plumb-core` public API another task consumes.
- One task adds a `Rule` the other task registers in `register_builtin`.
- One task modifies `plumb.toml` schema, another adds a config-consuming feature.
- Shared `Cargo.toml` edits that would race (same `[dependencies]` block).

### Step 3 — batch formation

- Max 4 tasks per batch.
- Group by independence, not by similarity.
- Dependent tasks go in separate, ordered batches.
- `plumb-cdp` or `plumb-mcp` touching changes always pair with `06-security-auditor` (parallel).
- `plumb-core` public API changes are hard-sequential — they block every downstream change until merged.
- Rule-authoring tasks go through `08-rule-author`; MCP tool tasks through `09-mcp-tool-author`; trivial fixes through `10-quick-fix`; everything else through `01-implementer`.

### Step 4 — parallel dispatch

Send ALL batch implementers in a single message with multiple `Task` tool calls:

```
[Single message, multiple Task calls]
Task 1 (01-implementer): Implement "rule spacing/grid-conformance"
  batchId: batch-1, position 1 of 3, files: [crates/plumb-core/src/rules/spacing/grid.rs, tests/golden_spacing_grid.rs, docs/src/rules/spacing-grid-conformance.md], peers: [Task 2, Task 3]
Task 2 (01-implementer): Implement "rule type/scale-conformance"
  batchId: batch-1, position 2 of 3, files: […], peers: [Task 1, Task 3]
Task 3 (01-implementer): Implement "rule radius/scale-conformance"
  batchId: batch-1, position 3 of 3, files: […], peers: [Task 1, Task 2]
```

**Critical**: a single message for true parallelism. Sequential messages = sequential execution.

### Step 5 — join point

After a batch completes:

1. Collect file manifests from each agent (created / modified / deleted + commit SHAs).
2. Check for scope violations (two agents wrote the same file → decomposition error).
3. Run the narrow gate:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo nextest run --workspace --all-features
   ```
4. If the gate fails, identify the offending agent and re-dispatch with the fix scope.

### Step 6 — batch-aware review

1. Dispatch `02-spec-reviewer` with all batch task specs + actual commits.
2. If issues: collect fixes, dispatch as a fresh focused batch.
3. Dispatch `03-code-quality-reviewer` (after spec passes) with the batch diff.
4. If issues: collect fixes.
5. Dispatch `05-architecture-validator` for layering / deny-lints / determinism.
6. Dispatch `04-test-runner` to run `just validate` on the batch.
7. Dispatch `06-security-auditor` in parallel with any of the above when `plumb-cdp`, `plumb-mcp`, URL handling, or deps changed.
8. Mark batch tasks complete only after every required gate returns `Verdict: APPROVE`.

### Step 7 — track batch state

Use `TaskCreate` metadata per task:

```json
{
  "batchId": "batch-1",
  "batchPosition": 1,
  "batchTotal": 3,
  "targetCrates": ["plumb-core"],
  "targetFiles": [
    "crates/plumb-core/src/rules/spacing/grid.rs",
    "crates/plumb-core/tests/golden_spacing_grid.rs",
    "docs/src/rules/spacing-grid-conformance.md"
  ],
  "status": "dispatched",
  "commitSHA": null,
  "reviews": {
    "spec": null,
    "quality": null,
    "architecture": null,
    "test": null,
    "security": "not_required"
  }
}
```

### Step 8 — iterate

Repeat for remaining batches.

### Example

```
Plan has 5 tasks:
- Task 1: Add the `spacing/grid-conformance` rule (plumb-core).
- Task 2: Add the `type/scale-conformance` rule (plumb-core).
- Task 3: Add the `--viewport` CLI flag (plumb-cli).
- Task 4: Add the `list_rules` MCP tool (plumb-mcp) — depends on Task 1 + 2 registering.
- Task 5: Add CDP `DOMSnapshot.captureSnapshot` integration (plumb-cdp) — touches unsafe.

Analysis:
- Tasks 1, 2, 3 are independent (different crates, no overlapping API).
- Task 4 depends on Tasks 1 + 2 (lists them via `register_builtin`).
- Task 5 is isolated — it lives in plumb-cdp and needs security-auditor.

Batches:
- Batch 1 (parallel, 01-implementer ×2 + 08-rule-author ×0 — use 08-rule-author for 1 & 2): Tasks 1, 2, 3.
- Batch 2 (sequential): Task 4 (09-mcp-tool-author).
- Batch 3 (isolated + security-auditor): Task 5.

Execution:
[Batch 1: dispatch 3 agents in a single message]
[Wait for all 3; collect manifests; join gate]
[Batch-aware review: 02-spec → 03-quality → 05-architecture → 04-test]
[Batch 2: dispatch 09-mcp-tool-author]
[Four-gate review]
[Batch 3: dispatch 01-implementer for CDP work]
[Four-gate review + 06-security-auditor in parallel]
[Complete]
```

## Security-sensitive work (never parallel with other implementers)

For `plumb-cdp`, `plumb-mcp` (tool surface exposed to AI agents), URL parsing, or dependency-graph changes:

1. Complete other batches first.
2. Dispatch a single implementer in an isolated batch.
3. Run the usual four gates.
4. Dispatch `06-security-auditor` — in parallel with any of the four gates is fine, but never slip it.
5. Block merge on any critical finding.

## Prompt templates

- `./implementer-prompt.md` — dispatch `01-implementer` (or specialized variants `08-rule-author`, `09-mcp-tool-author`, `10-quick-fix`).
- `./spec-reviewer-prompt.md` — dispatch `02-spec-reviewer`.
- `./code-quality-reviewer-prompt.md` — dispatch `03-code-quality-reviewer`.

## Red flags

**Never:**

- Skip a required gate (spec → quality → architecture → test, plus security when triggered).
- Accept "close enough" on spec — `02-spec-reviewer` finding issues = not done.
- Start `03-code-quality-reviewer` before spec returned APPROVE.
- Let implementer self-review replace a formal reviewer gate.
- Move to the next task while any gate is still open.

**Parallel-specific:**

- Parallelize tasks that touch the same file.
- Parallelize tasks with a `plumb-core` public-API dependency chain.
- Send parallel dispatches in separate messages (defeats parallelism).
- Group more than 4 tasks in a batch.
- Mix `plumb-cdp`-touching tasks with unrelated implementation in the same batch.
- Skip independence analysis — when unsure, sequence.

**If subagent asks questions:**

- Answer clearly and completely.
- Provide additional context if needed.
- Don't rush them into implementation.

**If reviewer finds issues:**

- Same implementer fixes them.
- Re-dispatch the reviewer.
- Repeat until APPROVE.

**If subagent fails a task:**

- Dispatch `07-debugger` first for root-cause diagnosis.
- Then dispatch `01-implementer` with the specific fix scope.
- Don't try to fix manually — context pollution.

## Integration

**Plumb subagents (required):**

- `01-implementer` — general code implementation.
- `02-spec-reviewer` — spec compliance (first gate).
- `03-code-quality-reviewer` — Rust idioms + lint-suppression review (second gate, after spec).
- `05-architecture-validator` — layering + unsafe + determinism (third gate).
- `04-test-runner` — `just validate` + `just determinism-check` + `cargo deny check` (fourth gate).
- `06-security-auditor` — opus-grade security review for `plumb-cdp` / `plumb-mcp` / URL / deps (parallel with any of the four gates).

**Specialized implementers:**

- `08-rule-author` — new rule + golden test + docs page + `register_builtin` entry.
- `09-mcp-tool-author` — new `#[tool]` method + protocol test.
- `10-quick-fix` — single-commit trivial fixes (typo, snapshot churn, dep bump with no API change).

**Supporting:**

- `07-debugger` — root-cause analysis for test failures / CI regressions.

Plumb has no domain-expert subagents. The PRD in `docs/local/prd.md` and the rules under `.agents/rules/` are the authoritative domain spec — read them directly instead of dispatching a separate expert.
