# Introduction

Most UI bugs live in the rendered 90% that source linters never see.
Spacing drift, off-scale type, near-miss alignment, and touch targets
that almost pass all show up after the browser computes the page.

Plumb is a deterministic design-system linter for rendered websites. It
opens a page in a headless browser at multiple viewports, measures the
computed DOM against a declared spec, and emits structured,
pixel-precise violations that an AI coding agent can act on without
guessing.

Where ESLint checks source code, Plumb checks the output your users
actually get.

## What Plumb is for

Plumb fits the gap between source linting and screenshot diffing.

- Source linters such as ESLint and stylelint catch problems in the code
  you wrote.
- Visual regression tools catch that a screenshot changed.
- Plumb checks the computed DOM and tells you which design-system rule
  broke, where it broke, and by how much.

That makes it useful in CI, local debugging, and agent workflows where a
machine-readable violation is more useful than a red screenshot.

## Two entry points

- **CLI** (`plumb lint <url>`) for developers and CI.
- **MCP server** (`plumb mcp`) for AI coding agents (Claude Code,
  Cursor, Codex, Windsurf) via the Model Context Protocol.

Both share the same rule engine. The outputs match byte-for-byte across
runs. Determinism is a hard guarantee.

## Demo

> Demo slot for Issue #63 Slice B: a short rendered-UI walkthrough will
> land here once the checked-in asset is ready.

Until then, the live docs are the easiest public target to lint:

```bash
plumb lint https://plumb.aramhammoudeh.com
```

## Install and try it

Start with the path that matches how you work:

- [Install script](./install.md#install-script-macos--linux--windows)
- [`cargo install`](./install.md#cargo)
- [Homebrew](./install.md#homebrew)
- [Build from source](./install.md#build-from-source)

Then continue with the docs for your workflow:

- [Quick start](./quickstart.md) for the first local run
- [MCP server](./mcp.md) for agent setup
- [GitHub Code Scanning](./ci/github-code-scanning.md) for SARIF in CI
- [reviewdog](./ci/reviewdog.md) for PR feedback in CI

## Status

Pre-alpha. The current public docs live at
`https://plumb.aramhammoudeh.com/`. Canonical copy may refer to
`plumb.dev`, but the root-domain deployment step is still pending.

## Next

- [Install](./install.md) — pick a release channel.
- [Quick start](./quickstart.md) — five minutes from install to first violation.
- [Configuration](./configuration.md) — `plumb.toml` reference.
- [CLI](./cli.md) — commands, flags, exit codes.
- [MCP server](./mcp.md) — JSON-RPC surface, tool list.
- [Rules](./rules/overview.md) — the catalog.
