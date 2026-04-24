# Resume Contract

Protocol for resuming a /gh-issue run after session compaction or interruption.

## Pre-Conditions for Resume

Before resuming, verify:

1. Run directory exists: `.agents/runs/gh-issue/<primary>-<slug>/`
2. `state.json` is valid and parseable
3. `plan.md` exists (unless still in `investigating` phase)
4. Branch exists in git: `git branch --list <branch>`

Use the validation command:
```bash
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py validate-resume <primary> <slug>
```

## Resume Steps by Phase

### investigating
- Re-read the issue: `gh issue view <N> --repo aram-devdocs/omnifol --json ...`
- Continue codebase exploration
- Complete `init-run` if state was not saved

### planning
- Read existing partial plan from `plan.md` if it exists
- Complete the plan and present to user for approval

### bootstrapped
- Verify the branch exists: `git branch --list <branch>`
- If branch is missing, recreate from `dev`
- Continue to implementing phase

### implementing
- Read `plan.md` for dispatch plan
- Check which commits exist vs what was planned
- Dispatch remaining subagents for unfinished work

### verifying
- Re-run verification: `pnpm typecheck && pnpm lint && pnpm --filter @omnifol/<pkg> test`
- Fix any failures, then advance to reviewing

### reviewing
- Check `state.reviews` to see which gates passed
- Resume from the first non-passed required gate
- Order: spec -> quality -> architecture -> security (if required)

### pr
- Verify the PR exists: `gh pr view <pr> --repo aram-devdocs/omnifol`
- If PR not created yet, create it now
- Advance to waiting-ci

### waiting-ci
- Poll CI: `python3 .agents/skills/gh-issue/scripts/gh_issue_run.py poll-pr <primary> <slug>`
- Handle any failures

### cleanup
- Complete cleanup steps
- Mark as done

### done
- Run is complete, nothing to resume

## State Reconstruction

If `state.json` is corrupted or missing but the run directory exists:

1. Read `plan.md` for original intent
2. Check `git log --oneline` for commits on the branch
3. Check `gh pr list --repo aram-devdocs/omnifol --head <branch>` for PR
4. Reconstruct state manually using `init-run` then `update-state` calls

## Session Handoff Notes

The `save-session.js` PreCompact hook writes active run details to stdout so the compaction summary includes run state. After compaction:

1. Read MEMORY.md for session context
2. Run `ls .agents/runs/gh-issue/` to list active runs
3. Run `validate-resume` on each active run
4. Resume from last known phase
