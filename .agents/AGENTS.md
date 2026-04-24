# .agents/

Tool-agnostic AI library for Plumb. The canonical entry point is
`/AGENTS.md` at the repo root — this file exists only as a pointer.

## Layout

- `rules/` — project-specific rules every agent must honor.
- `skills/` — reusable skills (humanizer, code review, etc.).
- `runs/` — gitignored scratch space for long-running agent runs.

## Integration with Claude Code

`.claude/rules` and `.claude/skills` are symlinks into this directory.
Any other AI tool that honors [AGENTS.md](https://agents.md) should point
its rule loader here.
