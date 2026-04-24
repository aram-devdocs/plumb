# CI Polling Prompt

Monitor CI status for PR #{{PR}} on branch `{{BRANCH}}`.

## Poll Command

```bash
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py poll-pr {{PRIMARY}} {{SLUG}}
```

Or directly:
```bash
gh pr checks {{PR}} --repo aram-devdocs/omnifol
```

## CI Workflows

The Omnifol CI runs:
1. Lint (Biome check)
2. Type check (tsc)
3. Tests (per package)
4. Build (turbo)

## On Failure

1. Read the failure details:
   ```bash
   gh pr checks {{PR}} --repo aram-devdocs/omnifol --fail-fast
   gh run view <run-id> --log-failed
   ```
2. Identify the failing step and package
3. Dispatch `debugger` for root cause analysis if not obvious
4. Dispatch appropriate fix agent
5. Push fix:
   ```bash
   git push origin {{BRANCH}}
   ```
6. Update run state with fix commit:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state {{PRIMARY}} {{SLUG}} --commit <sha>
   ```
7. Poll again

## On Pass

Update run state:
```bash
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state {{PRIMARY}} {{SLUG}} --phase cleanup
```

Proceed to cleanup phase.

## Timeout

If CI has not completed after 15 minutes, check for queued runners:
```bash
gh run list --repo aram-devdocs/omnifol --branch {{BRANCH}}
```

The CI uses a self-hosted Raspberry Pi runner - allow extra time if the runner is busy.
