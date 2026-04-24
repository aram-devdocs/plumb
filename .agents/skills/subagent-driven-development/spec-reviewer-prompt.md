# Spec-reviewer dispatch template

Use this template when dispatching `02-spec-reviewer` against a completed task or batch.

**Purpose:** verify the implementer built what was requested (nothing more, nothing less).

```
Task tool:
  subagent_type: 02-spec-reviewer
  description: "Review spec compliance for Task N"
  prompt: |
    You are reviewing whether an implementation matches its specification
    in the Plumb Rust workspace.

    ## What was requested

    [FULL TEXT of task requirements from the plan]

    ## What implementer claims they built

    [FROM implementer's report]

    ## Commits to inspect

    [List of commit SHAs, with `git show <sha>` the authority, not the report]

    ## Critical: do not trust the report

    The implementer's report may be incomplete, inaccurate, or optimistic.
    Verify everything independently.

    DO NOT:
    - Take their word for what they implemented.
    - Accept their interpretation of requirements.

    DO:
    - Read the actual code they wrote (`git show`, `git diff`).
    - Compare implementation to requirements line by line.
    - Check for missing pieces they claimed to implement.
    - Look for scope creep — extras they didn't mention.

    ## Your job

    Verify by reading code:

    **Missing requirements:**
    - Did they implement everything specified?
    - Are any acceptance criteria unmet?
    - Do the tests actually cover the specified behavior?

    **Scope creep:**
    - Did they build things not requested?
    - Did they add "nice to haves"?

    **Misunderstandings:**
    - Did they interpret the requirement differently than intended?
    - Did they solve the wrong problem?

    ## Output format

    End with exactly one line:

        Verdict: APPROVE
        Verdict: REQUEST_CHANGES
        Verdict: BLOCK

    Above the verdict, give a punch list with `file:line` references. Do
    not rubber-stamp — if nothing is wrong, say what you checked.

    `APPROVE` = spec fully met, no scope creep.
    `REQUEST_CHANGES` = specific gaps with a clear fix path.
    `BLOCK` = fundamental misread of the spec; redo.
```

## Batch review variant

For a parallel batch of N tasks, adapt the prompt:

```
Task tool:
  subagent_type: 02-spec-reviewer
  description: "Review spec compliance for Batch N"
  prompt: |
    You are reviewing spec compliance for a parallel batch of implementations.

    ## Tasks in this batch

    ### Task A: [name]
    **Spec:** [FULL TEXT]
    **Implementer report:** [...]
    **Commit SHA:** [...]
    **Files modified:** [manifest]

    ### Task B: [name]
    **Spec:** [FULL TEXT]
    ...

    ## Your job

    ### Per-task review

    For each task, verify:
    - Everything specified was implemented.
    - Nothing extra was added.
    - No misunderstandings.

    ### Cross-task review

    - Conflicting patterns between tasks (e.g., different error-type conventions).
    - Missing integration points (task A's output should feed task B but doesn't).
    - Duplicate logic across tasks.

    ## Output format

    End with exactly one line for the batch:

        Verdict: APPROVE
        Verdict: REQUEST_CHANGES
        Verdict: BLOCK

    Above the verdict:

    #### Per-task results
    - Task A: [pass/fail with file:line]
    - Task B: [pass/fail with file:line]

    #### Cross-task issues
    - [any integration or consistency issues]
```
