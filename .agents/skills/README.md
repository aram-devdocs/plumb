# Skills

Reusable, tool-agnostic skills Plumb's contributors and agents share.

## Planned

The following skills ship from the fleet in PR #2:

- `humanizer` — rewrites AI-ish docs prose into something a human would
  write. Runs on every PR touching `docs/src/**`.
- `code-reviewer` — runs a structured review pass before merge.
- `rust-tdd` — walks the red-green-refactor loop using `cargo nextest` +
  `cargo insta`.

Each skill has its own directory here with `skill.md` + any support
files. `.claude/skills` is a symlink to this directory.
