# Cleanup and Completion Prompt

CI has passed for PR #{{PR}}. Complete the gh-issue run for issue #{{PRIMARY}}.

## Steps

### 1. Worktree Cleanup (if applicable)

If the run used `--worktree`:
```bash
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py cleanup-worktree {{PRIMARY}} {{SLUG}}
```

### 2. Return to dev

```bash
git checkout dev && git pull origin dev
```

### 3. Mark Done

```bash
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state {{PRIMARY}} {{SLUG}} --phase done
```

### 4. Final Report

Provide a summary:

```
## Run Complete: Issue #{{PRIMARY}}

PR: #{{PR}} - <title>
Branch: {{BRANCH}}
Issues addressed: {{ISSUES}}

Commits:
<list commit SHAs and messages>

Review gates:
- spec-reviewer: pass
- code-quality-reviewer: pass
- architecture-validator: pass
- security-auditor: <pass | not_required>

CI: passed
Status: merged / awaiting merge
```

## Notes

- The run directory `.agents/runs/gh-issue/{{PRIMARY}}-{{SLUG}}/` is preserved for audit
- The `.agents/runs/` directory is git-ignored, state is local only
- If the PR is not auto-merged, remind the user to merge via GitHub
