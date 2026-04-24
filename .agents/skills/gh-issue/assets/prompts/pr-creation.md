# PR creation prompt

Create a pull request for GitHub issue #{{PRIMARY}} in `aram-devdocs/plumb`.

## Branch and target

- Branch: `{{BRANCH}}`
- Target: `main`

## PR title

Conventional Commits format: `<type>(<scope>): <imperative description>`.

Allowed types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`, `build`, `style`, `revert`.

Allowed scopes (match crate or area): `core`, `format`, `cdp`, `config`, `mcp`, `cli`, `xtask`, `docs`, `ci`, `deps`, or a rule id like `spacing/hard-coded-gap`.

Examples:

- `feat(cdp): Chromium detection and BYO-Chromium support`
- `fix(core): stable sort key when viewport names tie`
- `refactor(mcp): extract tool-arg parsing into shared helper`

Rules:

- Lowercase after the type prefix.
- No period at end.
- Under 72 characters total.
- Imperative mood (`add`, `fix`, `update` — not `added`, `fixed`, `updated`).

The `pr-title` workflow validates this on GitHub.

## PR body

Start from `.agents/skills/gh-issue/assets/pr-body-template.md` — it mirrors `.github/PULL_REQUEST_TEMPLATE.md` section-for-section. Fill every section; do not collapse the template into a short summary.

```bash
cp .agents/skills/gh-issue/assets/pr-body-template.md /tmp/{{PRIMARY}}-pr-body.md
# Edit placeholders and tick checklist items before creating the PR.
```

## Command

```bash
git push -u origin {{BRANCH}}
gh pr create --repo aram-devdocs/plumb --base main \
  --title "<type>(<scope>): <imperative description>" \
  --body-file /tmp/{{PRIMARY}}-pr-body.md
```

## After creation

Update run state with the PR number:

```bash
python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state {{PRIMARY}} {{SLUG}} --phase pr --pr <number>
```

## Local review gate (pre-PR)

```bash
python3 .agents/skills/gh-review/scripts/gh_review.py --local-diff main...HEAD
```

Any BLOCK-severity finding sends you back to `implementing`. REQUEST_CHANGES findings should be resolved before opening the PR so the remote review is quick.
