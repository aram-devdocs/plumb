# .agents/skills — tool-agnostic skills library

See `/AGENTS.md` for the read order. This file scopes to `.agents/skills/`.

## Shape

Every skill is a self-contained directory with:

- `SKILL.md` — the entry point. YAML frontmatter with `name`, `description`, optional `user_invocable`, optional `allowed-tools`.
- `scripts/` — executable scripts (`.sh`, `.py`). Each skill owns a `validate_skill.py`.
- `assets/` — templates, prompts, fixtures.
- `references/` — markdown grammars, contracts, long-form reference material.

Mirrors how GoudEngine / throne_ge / project-paws lay their skills out. `.claude/skills` is a symlink to this directory — Claude Code reads skills from here.

## Plumb-native only

Every skill here is Plumb-shaped:

- Repo references point at `aram-devdocs/plumb`.
- Build / test commands are cargo / just.
- Branch target is `main`.
- Subagent names match Plumb's 10-agent set (`01-implementer` … `10-quick-fix`).
- No Plumb-absent concepts (no stateless-UI pattern, no JS/TS toolchain references).

The only domain-free skills are `humanizer/` (language patterns) and `find-skills/` (skill discovery).

## Adding a new skill

1. Create `<skill-name>/SKILL.md` with frontmatter.
2. Add a `scripts/validate_skill.py` that checks required files, required sections in templates, and scans for forbidden phrases (fleet residue, etc.).
3. Ship the skill with one integration test or smoke test per `scripts/*.py` you add.

## Anti-patterns

- Importing workflows wholesale from other fleet repos. They were
  inspiration, not templates; adapt what maps, drop what doesn't.
- Skills that depend on deploy targets Plumb doesn't have (database
  migrations, stateless UI patterns, JS/TS toolchain).
- Skills without a validator. Drift is silent otherwise.
