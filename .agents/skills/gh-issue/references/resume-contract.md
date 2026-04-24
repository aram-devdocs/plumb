# Resume contract

Protocol for resuming a `/gh-issue` run after session compaction or interruption.

## Pre-conditions for resume

Before resuming, verify:

1. Run directory exists: `.agents/runs/gh-issue/<primary>-<slug>/`.
2. `state.json` is valid and parseable.
3. `plan.md` exists (unless still in `investigating` phase).
4. Branch exists in git: `git branch --list <branch>`.

Run the validation command:

```bash
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py validate-resume <primary> <slug>
```

## Resume steps by phase

### investigating

- Re-read the issue: `gh issue view <N> --repo aram-devdocs/plumb --json number,title,body,labels,milestone,assignees,comments`.
- Continue codebase exploration.
- Complete `init-run` if state was not saved.

### planning

- Read any partial `plan.md`.
- Complete the plan and present to user for approval.

### bootstrapped

- Verify the branch exists: `git branch --list <branch>`.
- If missing, recreate from `main`: `git checkout main && git pull && git checkout -b <branch>`.
- Continue to `implementing`.

### implementing

- Read `plan.md` for the dispatch plan.
- Check which commits exist vs what was planned (`git log main..HEAD --oneline`).
- Dispatch remaining subagents for unfinished work.

### verifying

- Re-run `just validate` (or the narrow loop: `cargo fmt --check && cargo clippy -p <crate> -- -D warnings && cargo nextest run -p <crate>`).
- Fix any failures, then advance to `reviewing`.

### reviewing

- Check `state.reviews` to see which gates passed.
- Resume from the first non-passed required gate.
- Order: spec → quality → architecture → test. `security` runs in parallel when triggered.

### pr

- Verify the PR exists: `gh pr view <pr> --repo aram-devdocs/plumb`.
- If PR not created yet, create it now.
- Advance to `waiting-ci`.

### waiting-ci

- Poll: `python3 .agents/skills/gh-issue/scripts/gh_issue_run.py poll-pr <primary> <slug>`.
- Handle any failures.

### cleanup

- Complete cleanup steps.
- Mark as done.

### done

- Run is complete, nothing to resume.

## State reconstruction

If `state.json` is corrupted or missing but the run directory exists:

1. Read `plan.md` for original intent.
2. Check `git log --oneline` for commits on the branch.
3. Check `gh pr list --repo aram-devdocs/plumb --head <branch>` for PR.
4. Reconstruct state manually via `init-run` followed by `update-state` calls.

## Session handoff notes

The `save-session` Stop hook writes a per-session summary to `.claude/state/sessions.log` so active run context survives compaction. After compaction:

1. `ls .agents/runs/gh-issue/` to list active runs.
2. `validate-resume <primary> <slug>` on each.
3. Read `state.json` + `plan.md` for the targeted run.
4. Resume from the last known phase.
