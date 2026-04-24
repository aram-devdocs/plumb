# Code Quality Reviewer Dispatch Template

**Purpose:** Verify implementation is well-built (clean, tested, maintainable)
**Only dispatch after spec compliance review passes.**

## Dispatch Format

Dispatch the `code-quality-reviewer` agent (defined in `.claude/agents/code-quality-reviewer.md`) with:

```
Review the implementation for code quality.

WHAT_WAS_IMPLEMENTED: [from implementer's report]
PLAN_OR_REQUIREMENTS: Task N from [plan-file]
BASE_SHA: [commit SHA before task started]
HEAD_SHA: [current commit SHA]
DESCRIPTION: [task summary]

Focus areas:
1. Architecture: L1-L6 layer compliance, import boundaries
2. Type safety: Zod schemas, no `any`, exhaustive switches
3. Code quality: No console.log, no magic numbers, proper extraction
4. Test quality: TDD (tests written first), AAA pattern, coverage
5. Accessibility: Semantic HTML, ARIA labels, keyboard navigation
```

**Reviewer returns:** Strengths, Issues (Critical/Important/Minor), Assessment

## Batch Review Variant

Use this template when reviewing code quality for a parallel batch.

```
Review the implementation quality for a parallel batch.

BATCH_SCOPE:
- Task A: [summary], SHA: [sha], Files: [list]
- Task B: [summary], SHA: [sha], Files: [list]

PLAN_OR_REQUIREMENTS: [batch plan reference]
DESCRIPTION: Batch N quality review

Focus areas:
1. Per-task quality: Architecture, type safety, code quality, test quality per task
2. Cross-task consistency: Naming conventions, patterns, approaches match across tasks
3. Integration quality: Cross-package imports correct, no circular dependencies introduced
4. Batch-level assessment: Overall quality of the combined changeset

Report format:
### Per-Task Assessment
- Task A: Strengths, Issues (Critical/Important/Minor)
- Task B: Strengths, Issues (Critical/Important/Minor)

### Cross-Task Assessment
- Pattern consistency: [pass/issues]
- Integration quality: [pass/issues]

### Batch Verdict
APPROVED / CHANGES REQUESTED / APPROVED WITH NOTES
```
