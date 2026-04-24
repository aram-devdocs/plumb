# docs — Plumb documentation

See `/AGENTS.md` for repo-wide rules. This file scopes to `docs/`.

## Structure

- `docs/src/` — mdBook source for <https://plumb.dev>.
- `docs/src/rules/` — one file per built-in rule (naming: `<category>-<id>.md`).
- `docs/adr/` — architecture decision records. Numbered `NNNN-<slug>.md`.
- `docs/runbooks/` — YAML runbook specs consumed by `/gh-runbook`.
- `docs/local/` — gitignored scratch space (PRD lives here until extracted).

## Humanizer is mandatory

Every PR that touches `docs/src/**` runs the humanizer skill
(`.agents/skills/humanizer/`) before review. The validator flags:

- AI vocabulary — `delve`, `tapestry`, `landscape`, `leverage`, `robust`, `seamless`, `comprehensive`, `vibrant`.
- Stilted openers — `Dive in`, `In conclusion`, `It's important to note`.
- Rule-of-three overuse, negative parallelism, sycophantic tone.

## RFC 2119 keywords

When documenting a contract (Rule trait, MCP protocol, config file
semantics), use MUST / MUST NOT / SHOULD / SHOULD NOT / MAY per
RFC 2119. Lowercase the same words for non-normative prose.

## Rule docs shape

Every `docs/src/rules/<category>-<id>.md` MUST have these sections:

1. `# <category>/<id>` header.
2. `**Status:** …` + `**Default severity:** …`.
3. `## What it checks` — precise English.
4. `## Why it matters` — user-facing rationale.
5. `## Example violation` — JSON excerpt.
6. `## Configuration` — knobs from `plumb.toml`.
7. `## Suppression` — how to disable / ignore.
8. `## See also` — links to adjacent rules + PRD refs.

`cargo xtask sync-rules-index` fails if any `register_builtin` rule is
missing its doc page.

## Anti-patterns

- Documenting rules anywhere other than `docs/src/rules/`.
- Multi-paragraph docstrings in source — one short line max; put the
  long-form in the book.
- Running mdBook plugins that add non-determinism to the rendered HTML.
