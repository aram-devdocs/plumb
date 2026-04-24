# .agents/rules — normative rules

See `/AGENTS.md` for the repo-wide read order. This file scopes to
`.agents/rules/`.

## Shape

Every file here is a normative rule document. Naming: `<topic>.md`
(kebab-case). One page each — if a topic grows beyond ~150 lines, split
it rather than nest.

## What counts as a rule

A rule is project-wide guidance that:

- Applies to every contributor (human or agent).
- Has enforcement somewhere in the pipeline (workspace lint, CI job,
  lefthook hook, xtask check, or review gate).
- Encodes an invariant, not a preference.

Language-of-the-week opinions don't belong here; they belong in a PR
discussion.

## Current rules

- `determinism.md` — byte-identical output invariants.
- `dependency-hierarchy.md` — crate layer graph + enforcement.
- `rule-engine-patterns.md` — how to add a rule.
- `mcp-tool-patterns.md` — how to add an MCP tool.
- `testing.md` — nextest + insta + proptest conventions.
- `documentation.md` — humanizer + RFC 2119 + anti-AI-writing list.
- `dispatch-strategy.md` — when to split vs bundle vs cluster `/gh-issue` sessions.

## Anti-patterns

- Duplicating content from `AGENTS.md` files. Rules link out; scoped
  AGENTS.md files link back to the rules. One canonical source per
  invariant.
- Adding rules with no enforcement mechanism. If a rule can't be
  checked by CI or a hook, it's an opinion, not a rule.
