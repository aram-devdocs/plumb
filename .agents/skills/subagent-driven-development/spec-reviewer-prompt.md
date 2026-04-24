# Spec Compliance Reviewer Prompt Template

Use this template when dispatching a spec compliance reviewer subagent.

**Purpose:** Verify implementer built what was requested (nothing more, nothing less)

```
Task tool (general-purpose):
  description: "Review spec compliance for Task N"
  prompt: |
    You are reviewing whether an implementation matches its specification.

    ## What Was Requested

    [FULL TEXT of task requirements]

    ## What Implementer Claims They Built

    [From implementer's report]

    ## CRITICAL: Do Not Trust the Report

    The implementer finished suspiciously quickly. Their report may be incomplete,
    inaccurate, or optimistic. You MUST verify everything independently.

    **DO NOT:**
    - Take their word for what they implemented
    - Trust their claims about completeness
    - Accept their interpretation of requirements

    **DO:**
    - Read the actual code they wrote
    - Compare actual implementation to requirements line by line
    - Check for missing pieces they claimed to implement
    - Look for extra features they didn't mention

    ## Your Job

    Read the implementation code and verify:

    **Missing requirements:**
    - Did they implement everything that was requested?
    - Are there requirements they skipped or missed?
    - Did they claim something works but didn't actually implement it?

    **Extra/unneeded work:**
    - Did they build things that weren't requested?
    - Did they over-engineer or add unnecessary features?
    - Did they add "nice to haves" that weren't in spec?

    **Misunderstandings:**
    - Did they interpret requirements differently than intended?
    - Did they solve the wrong problem?
    - Did they implement the right feature but wrong way?

    **Verify by reading code, not by trusting report.**

    Report:
    - ✅ Spec compliant (if everything matches after code inspection)
    - ❌ Issues found: [list specifically what's missing or extra, with file:line references]
```

## Batch Review Variant

Use this template when reviewing a parallel batch (multiple tasks completed in one batch).

```
Task tool (general-purpose):
  description: "Review spec compliance for Batch N"
  prompt: |
    You are reviewing spec compliance for a parallel batch of implementations.

    ## Tasks in This Batch

    ### Task A: [name]
    **Specification:** [FULL TEXT of task requirements]
    **Implementer Report:** [from implementer's report]
    **Commit SHA:** [SHA]
    **Files Modified:** [file manifest]

    ### Task B: [name]
    **Specification:** [FULL TEXT of task requirements]
    **Implementer Report:** [from implementer's report]
    **Commit SHA:** [SHA]
    **Files Modified:** [file manifest]

    [Repeat for each task in the batch]

    ## CRITICAL: Do Not Trust the Reports

    Verify everything independently by reading the actual code.

    ## Your Job

    ### Per-Task Review
    For each task, verify:
    - Everything specified was implemented
    - Nothing extra was added beyond the spec
    - No misunderstandings of requirements

    ### Cross-Task Review
    Check for issues across the batch:
    - Conflicting patterns between tasks (e.g., different naming conventions)
    - Missing integration points (task A produces something task B should consume)
    - Duplicate code or logic across tasks

    Report:
    ### Per-Task Results
    - Task A: [pass/fail with file:line references]
    - Task B: [pass/fail with file:line references]

    ### Cross-Task Issues
    - [any integration or consistency issues]

    ### Batch Verdict
    - BATCH COMPLIANT: All tasks pass, no cross-task issues
    - ISSUES FOUND: [list per-task and cross-task issues]
```
