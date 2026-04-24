# Feedback Triage Prompt

You are triaging review feedback for GitHub issue #{{PRIMARY}}.

## Reviewer: {{REVIEWER}}
## Verdict: {{VERDICT}}

## Feedback

{{FEEDBACK}}

## Your Task

Categorize each piece of feedback:

1. **Must fix** - spec violation, bug, security issue, layer violation
2. **Should fix** - code quality, missing test, pattern inconsistency
3. **Consider** - style preference, minor optimization, non-blocking suggestion
4. **Disagree** - explain why the feedback does not apply

## Dispatch Decision

For each "Must fix" and "Should fix" item, identify:
- Which file(s) need changing
- Which subagent to dispatch (implementer, trpc-procedure, ui-component, etc.)
- Whether fixes are independent (parallel) or sequential

## After Fixes

Re-run verification:
```bash
pnpm typecheck && pnpm lint && pnpm --filter @omnifol/<pkg> test
```

Then re-dispatch the same reviewer gate.

Update run state only after the reviewer issues APPROVED verdict:
```bash
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state {{PRIMARY}} {{SLUG}} --review {{GATE}} pass
```

## Rules

- Never mark a gate as passed without an explicit APPROVED verdict
- spec-reviewer must pass before dispatching code-quality-reviewer
- Never skip a review gate
- Never modify test assertions to force passing tests
