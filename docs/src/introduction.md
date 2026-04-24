# Introduction

Plumb is a deterministic design-system linter for rendered websites. It
opens a page in a headless browser at multiple viewports, measures the
computed DOM against a declared spec, and emits structured, pixel-precise
violations that an AI coding agent can act on without guessing.

Where ESLint checks source code, Plumb checks the output — the thing
your users actually see.

## Two entry points

- **CLI** (`plumb lint <url>`) for developers and CI.
- **MCP server** (`plumb mcp`) for AI coding agents (Claude Code,
  Cursor, Codex, Windsurf) via the Model Context Protocol.

Both share the same rule engine. The outputs match byte-for-byte across
runs — determinism is a hard guarantee, not a goal.

## Status

Pre-alpha. The walking skeleton is in place; real rules land over the
next several PRs. See the PRD in `docs/local/prd.md` for the full
roadmap.

## Next

- [CLI](./cli.md) — commands, flags, exit codes.
- [MCP server](./mcp.md) — JSON-RPC surface, tool list.
- [Configuration](./configuration.md) — `plumb.toml` reference.
- [Rules](./rules/overview.md) — the catalog.
