# CI polling prompt

Monitor CI for PR #{{PR}} on branch `{{BRANCH}}`.

## Poll command

```bash
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py poll-pr {{PRIMARY}} {{SLUG}}
```

Or directly:

```bash
gh pr checks {{PR}} --repo aram-devdocs/plumb
```

## CI workflows

Plumb CI (`.github/workflows/ci.yml`) runs:

1. **preflight** — `cargo fmt --check`, `cargo clippy -D warnings`, `cargo check`.
2. **test** matrix — Linux / macOS / Windows × stable, `cargo nextest`.
3. **msrv** — `cargo check` on exact toolchain 1.85.0.
4. **determinism** — `just determinism-check` (3× byte-diff fixture run).
5. **coverage** — `cargo llvm-cov` → Codecov.
6. **size-guard** — strip release binary, assert < 25 MiB.
7. **deny** — `cargo deny check`.
8. **docs** — `mdbook build docs/` + `cargo doc --no-deps`.

Also: `claude-code-review.yml` runs the automated Claude reviewer on every non-draft PR.

## On failure

1. Read failure details:

   ```bash
   gh pr checks {{PR}} --repo aram-devdocs/plumb --fail-fast
   gh run view <run-id> --repo aram-devdocs/plumb --log-failed
   ```

2. Identify the failing step and crate.

3. Dispatch:
   - `07-debugger` for root-cause diagnosis if the failure is non-obvious.
   - `01-implementer` (or `10-quick-fix` for a one-liner) to apply the fix.

4. Push the fix:

   ```bash
   git push origin {{BRANCH}}
   ```

5. Record the fix commit:

   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state {{PRIMARY}} {{SLUG}} --commit <sha>
   ```

6. Poll again.

## On pass

Update run state:

```bash
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state {{PRIMARY}} {{SLUG}} --phase cleanup
```

Proceed to cleanup.

## Timeout

If CI has not completed after 20 minutes, check for queued runners:

```bash
gh run list --repo aram-devdocs/plumb --branch {{BRANCH}}
```

GitHub-hosted runners usually complete the test matrix in 10–15 minutes. If the determinism or size-guard jobs are queued beyond that, re-trigger with `gh workflow run`.
