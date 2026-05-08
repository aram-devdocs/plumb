# Contributing

Patches, bug reports, and rule proposals are welcome. The full contributor guide — prerequisites, the development loop, commit conventions, and the no-bypass quality gates — lives at [`CONTRIBUTING.md`](https://github.com/aram-devdocs/plumb/blob/main/CONTRIBUTING.md) in the repo root.

Before opening a non-trivial PR, please read:

- [`CONTRIBUTING.md`](https://github.com/aram-devdocs/plumb/blob/main/CONTRIBUTING.md) — toolchain, `just` targets, commit format, CI gates.
- [`AGENTS.md`](https://github.com/aram-devdocs/plumb/blob/main/AGENTS.md) — repo-wide read order for humans and AI assistants.

## Architecture decision records

ADRs document the why behind non-obvious choices: workspace layout, dependency policy, the Chromium version range, and so on.

- [Architecture decision records](./adr.md) — the in-book index.

## Project rules

Project-wide invariants — determinism, dependency hierarchy, no-legacy-code, rule-engine and MCP-tool patterns, testing, documentation — live alongside the code at [`.agents/rules/`](https://github.com/aram-devdocs/plumb/tree/main/.agents/rules). Every contributor (human or agent) is expected to follow them; CI enforces most of them automatically.

## Reporting bugs and security issues

- Bugs and feature requests: [GitHub Issues](https://github.com/aram-devdocs/plumb/issues).
- Security vulnerabilities: see the [security policy](./security.md). Do not open a public issue.
