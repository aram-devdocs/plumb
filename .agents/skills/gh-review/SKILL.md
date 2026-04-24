---
name: gh-review
description: Local GitHub PR review workflow for Omnifol. Mirrors .github/workflows/claude-code-review.yml, reviews a PR number or local diff, classifies changed files, applies blocker and warning taxonomy, and drafts a structured markdown review body before optionally posting it.
user_invocable: true
---

# /gh-review - Omnifol Local PR Review

Run the same review shape used by the GitHub review workflow, but locally and in dry-run mode by default.

## Usage

```bash
/gh-review 512
/gh-review 512 --instructions "focus on auth regressions"
/gh-review --local-diff dev...HEAD
/gh-review --local-diff HEAD~3..HEAD --instructions "check docs tone and UI wording"
```

## Inputs

- **PR mode**: review an existing GitHub PR by number
- **Local diff mode**: review an unpublished or pre-PR diff range
- **Optional instructions**: extra reviewer focus areas

## Workflow

1. Read `AGENTS.md` and any scoped rules for touched files.
2. Mirror `.github/workflows/claude-code-review.yml`.
3. Gather review context with:
   ```bash
   python3 .agents/skills/gh-review/scripts/gh_review.py --pr <number>
   python3 .agents/skills/gh-review/scripts/gh_review.py --local-diff dev...HEAD
   ```
4. Inspect high-risk files manually:
   - auth / exchange / financial changes require a security pass
   - UI changes require semantic HTML, keyboard access, WCAG AA, and breakpoint review
   - strategy / omniscript changes require domain-rule verification
5. Draft the review body using `.agents/skills/gh-review/assets/review-template.md`.
6. Only post with `--post` after confirming the draft is accurate.

## Review Contract

- File buckets: schema, UI, API, config, migration, strategy, trading
- Blockers and warnings follow the workflow contract in `references/workflow-contract.md`
- Output must end with exactly one verdict:
  - `APPROVED`
  - `CHANGES REQUESTED`
  - `NEEDS DISCUSSION`

## Output

Default output is markdown to stdout. Optional flags:

```bash
python3 .agents/skills/gh-review/scripts/gh_review.py --pr <number> --output /tmp/review.md
python3 .agents/skills/gh-review/scripts/gh_review.py --pr <number> --post
```

## Rules

- Dry run is the default; do not post comments automatically.
- Use the exact section order from the GitHub workflow template.
- Treat `any`, `@ts-ignore`, `@ts-expect-error`, `console.log`, hardcoded config, missing migrations for schema changes, direct database access, and business logic in tRPC procedures as blocker-class findings.
- If documentation, comments, or user-facing copy changed, run a humanizer pass before approving.
- For auth, exchange, or financial changes, add explicit security commentary even if the verdict is clean.
