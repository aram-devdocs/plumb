# CI + review polling prompt

Drive PR #{{PR}} to green on branch `{{BRANCH}}`. The session does NOT advance to cleanup until both of these converge:

- Every CI check green on `gh pr checks {{PR}}`.
- Latest Claude code-review comment ends with `Verdict: APPROVE`.

## Poll

```bash
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py poll-pr {{PRIMARY}} {{SLUG}}
```

The poller prints one of:

| CI | Review | Action |
|----|--------|--------|
| pass | approve | Advance — `--phase cleanup`. Exit the loop. |
| pass | request_changes / block | Read the review comment, dispatch fix, push, re-poll. |
| pass | pending | Wait ≥ 60s and re-poll. Claude reviewer is still writing. |
| pass | none | Wait ≥ 60s and re-poll. Review workflow may not have fired yet. |
| fail | (any) | Read failure details, dispatch fix, push, re-poll. |
| pending | (any) | Wait ≥ 60s and re-poll. CI in flight. |

## CI workflows on Plumb

`.github/workflows/ci.yml` runs on every push:

1. **preflight** — `scripts/check-agents-md.sh`, `cargo fmt --check`, `cargo clippy -D warnings`, `cargo check`.
2. **test** matrix — Linux / macOS / Windows × stable, `cargo nextest`.
3. **msrv** — `cargo check` on exact toolchain 1.85.0.
4. **determinism** — `just determinism-check` (3× byte-diff fixture run).
5. **coverage** — `cargo llvm-cov` → Codecov.
6. **size-guard** — strip release binary, assert < 25 MiB.
7. **deny** — `cargo deny check`.
8. **docs** — `mdbook build docs/` + `cargo doc --no-deps`.

`.github/workflows/claude-code-review.yml` runs on every non-draft PR push. It posts a single review comment ending with a verdict line.

## Fix loop

Applies to both CI failures and review `REQUEST_CHANGES` / `BLOCK` verdicts.

1. Read the failure / review comment:

   ```bash
   gh pr checks {{PR}} --repo aram-devdocs/plumb --fail-fast
   gh pr view {{PR}} --repo aram-devdocs/plumb --json comments --jq '.comments[-1].body'
   ```

2. Dispatch:
   - `07-debugger` first if the failure / feedback is non-obvious.
   - `01-implementer` to apply the fix (or `10-quick-fix` for a one-liner).

3. Push the fix:

   ```bash
   git push origin {{BRANCH}}
   ```

4. Record the fix commit:

   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state {{PRIMARY}} {{SLUG}} --commit <sha>
   ```

5. Re-poll. CI re-runs on every push; the Claude reviewer also auto-triggers on every push.

## Timeout + hard budget

- Each poll wait: ≥ 60s between invocations (rate-limit-friendly).
- Hard budget: **10 iterations** of the fix loop. If you exceed that, stop and surface to the user — something deeper is wrong that needs human judgment. Do not spin forever.
- If CI queues for > 20 minutes, check `gh run list --repo aram-devdocs/plumb --branch {{BRANCH}}` and consider re-triggering with `gh workflow run ci.yml --ref {{BRANCH}}`.

## On both pass

```bash
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state {{PRIMARY}} {{SLUG}} --phase cleanup
```

Then proceed to the cleanup prompt. Do not advance until both the CI rollup is green AND the Claude reviewer has posted `Verdict: APPROVE`.
