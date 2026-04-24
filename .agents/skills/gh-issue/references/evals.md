# Evals — gh-issue skill validation checks

Use these to validate a completed `/gh-issue` run or assess mid-run quality.

## Phase-completeness checks

### After implementing

- [ ] All commits recorded in `state.commits`.
- [ ] `cargo fmt --all -- --check` passes on branch.
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes.
- [ ] `cargo nextest run -p <touched-crate>` passes for each touched crate.
- [ ] No `println!`/`eprintln!` / `dbg!` / `todo!` / `unimplemented!` introduced in library crates.
- [ ] No new `unwrap` / `expect` / `panic!` in library crates.
- [ ] No new `SystemTime::now` / `Instant::now` in `plumb-core`.
- [ ] No new `unsafe` outside `plumb-cdp`.

### After reviewing

- [ ] `02-spec-reviewer` issued `Verdict: APPROVE`.
- [ ] `03-code-quality-reviewer` issued `Verdict: APPROVE` (after spec).
- [ ] `05-architecture-validator` issued `Verdict: APPROVE`.
- [ ] `04-test-runner` issued `Verdict: APPROVE`.
- [ ] `06-security-auditor` issued `Verdict: APPROVE` (when required).
- [ ] `state.reviews` has `pass` for every required gate.

### After PR creation

- [ ] PR targets `main`, not any other branch.
- [ ] PR title follows Conventional Commits.
- [ ] PR body references `Fixes #<primary>`.
- [ ] `state.pr` is set.

### After CI passes

- [ ] Every check green in `gh pr checks <PR> --repo aram-devdocs/plumb`.
- [ ] No force-push to branch.
- [ ] `state.phase` is `cleanup` or `done`.

## Code-quality checks

### Layer / lint enforcement

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo deny check
just determinism-check
```

### Schema + rules-index in sync

```bash
cargo xtask pre-release
```

## Branch checks

```bash
# Branch name follows pattern
echo "$BRANCH" | grep -E '^codex/\d+-[a-z]+-[a-z0-9-]+$'

# Branch is based on main
git merge-base --is-ancestor main "$BRANCH" && echo "OK"

# No merge commits in branch (keep linear)
git log main.."$BRANCH" --merges --oneline | wc -l   # should be 0
```

## Review-gate order check

```python
import json
state = json.load(open('state.json'))
reviews = state['reviews']
assert not (reviews['quality'] == 'pass' and reviews['spec'] != 'pass'), \
    'quality cannot pass before spec'
assert not (reviews['architecture'] == 'pass' and reviews['quality'] != 'pass'), \
    'architecture cannot pass before quality'
assert not (reviews['test'] == 'pass' and reviews['architecture'] != 'pass'), \
    'test cannot pass before architecture'
```

## Structural validation

```bash
python3 .agents/skills/gh-issue/scripts/validate_skill.py
```

## Post-merge checks

- [ ] Branch deleted from remote (or PR auto-deleted).
- [ ] Issue closed by PR merge.
- [ ] `state.phase == 'done'`.
- [ ] No leftover worktree directories.
