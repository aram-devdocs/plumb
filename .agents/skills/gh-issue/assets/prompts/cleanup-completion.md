# Cleanup and completion prompt

CI has passed for PR #{{PR}}. Complete the `/gh-issue` run for issue #{{PRIMARY}}.

## Steps

### 1. Worktree cleanup (if applicable)

If the run used `--worktree`:

```bash
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py cleanup-worktree {{PRIMARY}} {{SLUG}}
```

### 2. Return to main

```bash
git checkout main && git pull origin main
```

### 3. Mark done

```bash
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state {{PRIMARY}} {{SLUG}} --phase done
```

### 4. Final report

Provide a summary:

```
## Run complete: issue #{{PRIMARY}}

PR: #{{PR}} — <title>
Branch: {{BRANCH}}
Issues addressed: {{ISSUES}}

Commits:
<list commit SHAs + subjects>

Review gates:
- 02-spec-reviewer: pass
- 03-code-quality-reviewer: pass
- 05-architecture-validator: pass
- 04-test-runner: pass
- 06-security-auditor: <pass | not_required>

CI: passed
Status: merged / awaiting merge
```

## Notes

- The run directory `.agents/runs/gh-issue/{{PRIMARY}}-{{SLUG}}/` is preserved for audit.
- `.agents/runs/*/` is gitignored; state is local only.
- If the PR has not auto-merged, remind the user to merge via GitHub.
- If this issue was part of a runbook batch, check whether the batch is complete (all sibling PRs merged) before unblocking the next batch in `docs/runbooks/<phase>-spec.yaml`.
