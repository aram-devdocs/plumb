# docs/src/rules — per-rule documentation

See `/AGENTS.md` and `docs/AGENTS.md` for the docs contract.

## Naming

One file per built-in rule. File name = rule id with `/` → `-`.
Example: `spacing/hard-coded-gap` → `docs/src/rules/spacing-hard-coded-gap.md`.

## Required sections

Every file has, in order:

1. `# <category>/<id>` header.
2. `**Status:**` (active | placeholder | deprecated) + `**Default severity:**` (info | warning | error).
3. `## What it checks` — precise English.
4. `## Why it matters` — user-facing rationale.
5. `## Example violation` — JSON excerpt.
6. `## Configuration` — every `plumb.toml` knob that affects this rule.
7. `## Suppression` — how to disable or ignore.
8. `## See also` — links to adjacent rules + PRD refs.

## Enforced by

- `cargo xtask sync-rules-index` (part of `cargo xtask pre-release`)
  fails if any `register_builtin` rule is missing its doc page.
- `gh-review` flags rule-authoring PRs that add a rule without its doc.

## Anti-patterns

- Linking to external design-system docs as the rationale. Plumb rules
  must stand on their own; external links go under `See also`.
- Showing diff-style suggestions. Rules emit `Fix` payloads; docs
  describe the violation, not a patch.
