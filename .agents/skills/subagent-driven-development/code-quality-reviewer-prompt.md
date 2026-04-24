# Code-quality-reviewer dispatch template

Use this template when dispatching `03-code-quality-reviewer` against a task that has already cleared `02-spec-reviewer`.

**Purpose:** verify the Rust code is idiomatic, error shapes are clean, layering holds, and every lint suppression is justified.

**Only dispatch after `02-spec-reviewer` returned `Verdict: APPROVE`.** The `review-gate-guard` hook blocks out-of-order dispatch.

## Dispatch format

```
Task tool:
  subagent_type: 03-code-quality-reviewer
  description: "Code quality review for Task N"
  prompt: |
    Review the implementation of Task N for code quality in the Plumb
    Rust workspace.

    WHAT_WAS_IMPLEMENTED: [from implementer's report]
    PLAN_OR_REQUIREMENTS: Task N from [plan file]
    BASE_SHA: [commit SHA before task started]
    HEAD_SHA: [current commit SHA]
    DESCRIPTION: [task summary]

    Focus areas (in order):

    1. **Error handling.**
       - Library crates use `thiserror`-derived enums; `anyhow` only in `plumb-cli::main`.
       - Public fallible fns have `# Errors` sections in their rustdoc.
       - No new `unwrap`/`expect`/`panic!` in library code.
       - Errors are typed, not stringified early.

    2. **Layering + unsafe.**
       - Imports respect the crate graph (`plumb-core` has no internal deps; `plumb-cdp` is the only crate allowed `unsafe`).
       - Any new `unsafe` block has a `// SAFETY:` comment.
       - `forbid(unsafe_code)` still holds outside `plumb-cdp`.

    3. **Determinism.**
       - No new `SystemTime::now`/`Instant::now` in `plumb-core`.
       - No new `HashMap`/`HashSet` in observable output paths.
       - Sort key `(rule_id, viewport, selector, dom_order)` unchanged.

    4. **Lint suppression.**
       - Every `#[allow(...)]` is local (expression- or item-level).
       - Every suppression has a one-line rationale comment on the line above.

    5. **Naming + docs.**
       - Types `UpperCamel`, fns/values `snake_case`, constants `SCREAMING_SNAKE`.
       - Every new public item has at least a one-line rustdoc.
       - Constructors named `new`, `with_*`.

    6. **Test quality.**
       - Tests actually verify behavior (not mocking stubs).
       - Snapshots under `tests/snapshots/` are intentional (no `.snap.new`).
       - TDD evidence: test was green before implementation landed.

    7. **Docs hygiene** (if `docs/src/**` touched).
       - Humanizer skill run — no "delve", "tapestry", "landscape", "seamless", "leverage" in added prose.
       - RFC 2119 keywords used where contracts are documented.

    Report format:

    **Strengths:**
    - [2–4 specific bullets of what's done well]

    **Issues (Critical / Important / Minor):**
    - Critical: [must-fix before merge, with file:line]
    - Important: [should-fix before merge]
    - Minor: [optional improvement]

    End with exactly one line:

        Verdict: APPROVE
        Verdict: REQUEST_CHANGES
        Verdict: BLOCK
```

## Batch review variant

For a parallel batch:

```
Task tool:
  subagent_type: 03-code-quality-reviewer
  description: "Code quality review for Batch N"
  prompt: |
    Review code quality for a parallel batch in the Plumb workspace.

    BATCH_SCOPE:
    - Task A: [summary], SHA: [sha], files: [list]
    - Task B: [summary], SHA: [sha], files: [list]

    PLAN_OR_REQUIREMENTS: [batch plan reference]

    Focus areas:
    1. Per-task quality (same 7 areas as the single-task variant).
    2. Cross-task consistency — error-type conventions, naming, pattern alignment.
    3. Integration quality — no circular deps introduced; public API shape coherent across tasks.
    4. Batch-level lint output — run mentally against `cargo clippy --workspace -- -D warnings`.

    Report:

    ### Per-task assessment
    - Task A: Strengths + Issues (Critical / Important / Minor)
    - Task B: Strengths + Issues (Critical / Important / Minor)

    ### Cross-task assessment
    - Pattern consistency: pass | issues
    - Integration quality: pass | issues

    ### Batch verdict

    End with exactly one line:

        Verdict: APPROVE
        Verdict: REQUEST_CHANGES
        Verdict: BLOCK
```
