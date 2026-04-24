---
name: gh-review
description: Local PR review workflow for the Plumb Rust workspace. Mirrors .github/workflows/claude-code-review.yml, reviews a PR number or local diff, classifies changed files by crate, applies Plumb's Rust blocker and warning taxonomy, and drafts a structured markdown review body before optionally posting it.
user_invocable: true
---

# /gh-review — local PR review for Plumb

Run the same review shape used by the GitHub review workflow, but locally and in dry-run mode by default. Target repo: `aram-devdocs/plumb`.

## Usage

```bash
/gh-review 42
/gh-review 42 --instructions "focus on new MCP tool schema"
/gh-review --local-diff main...HEAD
/gh-review --local-diff HEAD~3..HEAD --instructions "check docs tone"
```

## Inputs

- **PR mode**: review an existing GitHub PR by number.
- **Local diff mode**: review an unpublished or pre-PR diff range.
- **Optional instructions**: extra reviewer focus areas.

## Workflow

1. Read `AGENTS.md` and every scoped `crates/*/AGENTS.md` for touched crates.
2. Mirror `.github/workflows/claude-code-review.yml` — same rules, same verdict format.
3. Gather review context:
   ```bash
   python3 .agents/skills/gh-review/scripts/gh_review.py --pr <number>
   python3 .agents/skills/gh-review/scripts/gh_review.py --local-diff main...HEAD
   ```
4. Inspect high-risk files manually:
   - `plumb-cdp` changes need a security pass (unsafe + CDP surface).
   - `plumb-mcp` changes need a security pass (agent-exposed tool surface).
   - Dependency changes need a cargo-deny pass.
   - Config/schema changes need a `cargo xtask pre-release` pass.
5. Draft the review body using `.agents/skills/gh-review/assets/review-template.md`.
6. Only post with `--post` after confirming the draft is accurate.

## Review contract

- **File buckets**: `plumb-core`, `plumb-format`, `plumb-cdp`, `plumb-config`, `plumb-mcp`, `plumb-cli`, `xtask`, `docs`, `ci`, `deps`.
- Blockers and warnings follow `references/workflow-contract.md`.
- Output ends with exactly one verdict:
  - `Verdict: APPROVE`
  - `Verdict: REQUEST_CHANGES`
  - `Verdict: BLOCK`

## Output

Default output is markdown to stdout. Optional flags:

```bash
python3 .agents/skills/gh-review/scripts/gh_review.py --pr <number> --output /tmp/review.md
python3 .agents/skills/gh-review/scripts/gh_review.py --pr <number> --post
```

## Rules

- Dry run is the default. Never post comments automatically.
- Use the exact section order from the GitHub workflow template.
- Blocker-class findings (Rust):
  - New `unsafe` outside `plumb-cdp`.
  - New `unwrap`/`expect`/`panic!` in a library crate.
  - New `println!`/`eprintln!` outside `plumb-cli`.
  - New `SystemTime::now`/`Instant::now`/`std::env::temp_dir` in `plumb-core`.
  - New `todo!`/`unimplemented!`/`dbg!` anywhere.
  - New `HashMap`/`HashSet` in observable output paths (prefer `IndexMap`).
  - New dep with GPL/AGPL/LGPL license (cargo-deny will catch; surface in review).
  - Rule added without a golden test + `register_builtin` entry + `docs/src/rules/` page.
  - MCP tool added without a protocol test in `crates/plumb-cli/tests/mcp_stdio.rs`.
  - Config schema changed without `cargo xtask schema` update committed.
  - Binary size regression ≥ 25 MiB (the CI size-guard job would fail).
- If `docs/src/**` changed, run the humanizer skill before approving.
- For `plumb-cdp` or `plumb-mcp` changes, add explicit security commentary even if verdict is clean.
