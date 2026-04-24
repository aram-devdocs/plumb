# Evals - gh-issue Skill Validation Checks

Use these checks to validate a completed gh-issue run or assess mid-run quality.

## Phase Completeness Checks

### After implementing
- [ ] All commits recorded in `state.commits`
- [ ] `pnpm typecheck` passes on branch
- [ ] `pnpm lint` passes on branch
- [ ] Tests pass for all affected packages
- [ ] No `console.log` in implementation files (use `@omnifol/logger`)
- [ ] No `any` types introduced
- [ ] No `@ts-ignore` or `@ts-expect-error`

### After reviewing
- [ ] spec-reviewer issued APPROVED verdict
- [ ] code-quality-reviewer issued APPROVED verdict (ran after spec)
- [ ] architecture-validator issued APPROVED verdict
- [ ] security-auditor issued APPROVED verdict (if required)
- [ ] `state.reviews` has `pass` for all required gates

### After PR creation
- [ ] PR targets `dev`, not `main`
- [ ] PR title follows conventional commit format
- [ ] PR body references `Fixes #<primary>`
- [ ] `state.pr` is set

### After CI passes
- [ ] All checks green in `gh pr checks <PR>`
- [ ] No force-push to branch
- [ ] `state.phase` is `cleanup` or `done`

## Code Quality Checks

### Layer violations
```bash
pnpm validate:architecture
```

### Import boundary check
```bash
pnpm typecheck 2>&1 | grep "Module .* has no exported member"
```

### Biome check
```bash
npx biome check packages/ apps/
```

## Branch Checks

```bash
# Branch name follows pattern
echo "{{BRANCH}}" | grep -E '^\d+-[a-z]+-[a-z0-9-]+$'

# Branch is based on dev (not main)
git merge-base --is-ancestor dev {{BRANCH}} && echo "OK"

# No merge commits in branch (keep linear)
git log dev..{{BRANCH}} --merges --oneline | wc -l  # should be 0
```

## Review Gate Order Check

```python
state = json.load(open('state.json'))
reviews = state['reviews']
assert reviews['spec'] == 'pass' or reviews['quality'] is None, \
    'quality cannot pass before spec'
```

## Structural Validation

```bash
python3 .agents/skills/gh-issue/scripts/validate_skill.py
```

## Post-Merge Checks

- [ ] Branch deleted from remote (or PR auto-deleted)
- [ ] Issue closed by PR merge
- [ ] `state.phase == 'done'`
- [ ] No leftover worktree directories
