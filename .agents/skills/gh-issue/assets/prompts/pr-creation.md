# PR Creation Prompt

Create a pull request for GitHub issue #{{PRIMARY}} in the Omnifol repository.

## Branch and Target

- Branch: `{{BRANCH}}`
- Target: `dev` (NEVER `main`)

## PR Title

Must follow conventional commit format: `<type>: <imperative description>`

Valid types: feat, fix, refactor, docs, test, chore, perf, ci, build, revert

Examples:
- `feat: add IDOR protection to transaction sync endpoints`
- `fix: scope position queries to authenticated user`
- `refactor: extract exchange credentials interface`

Rules:
- Lowercase after type prefix
- No period at end
- Under 72 characters
- Imperative mood (add, fix, update - not added, fixed, updated)

## PR Body

Reference `.github/pull_request_template.md` and start from `.agents/skills/gh-issue/assets/pr-body-template.md`.
Fill every section; do not collapse the template into a short summary.

```bash
cp .agents/skills/gh-issue/assets/pr-body-template.md /tmp/{{PRIMARY}}-pr-body.md
# Fill all placeholders and checklist sections before creating the PR
```

## Command

```bash
git push -u origin {{BRANCH}}
gh pr create --base dev \
  --title "<type>: <description>" \
  --body-file /tmp/{{PRIMARY}}-pr-body.md
```

## After Creation

Update run state with the PR number:
```bash
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state {{PRIMARY}} {{SLUG}} --phase pr --pr <number>
```

## Local Review Gate

Before PR creation, run the local review workflow:

```bash
python3 .agents/skills/gh-review/scripts/gh_review.py --local-diff dev...HEAD
```
