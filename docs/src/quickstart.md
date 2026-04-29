# Quick start

Five minutes from a fresh checkout to your first violation. This page
assumes you already followed the [Install](./install.md) page (or are
running from a source build) and have Chrome or Chromium installed for
the real-URL step.

## 1. Sanity-check the binary

```bash
plumb --version
plumb lint plumb-fake://hello
```

`plumb-fake://hello` is a built-in canned snapshot. It runs without a
browser and proves the rule engine works.

## 2. Drop a starter config

```bash
plumb init
```

This writes a `plumb.toml` in the current directory. The starter file
includes the three default viewports (`mobile`, `tablet`, `desktop`),
a 4-pixel spacing grid, a typographic scale, a small color palette,
and the touch-target spec.

The same file is checked into the repo at
[`examples/plumb.toml`](https://github.com/aram-devdocs/plumb/blob/main/examples/plumb.toml).
Compare against it whenever the schema changes.

## 3. Lint a real URL

```bash
plumb lint https://example.com
```

By default this snapshots the page at every viewport in
`plumb.toml` and prints `pretty` output. The exit code tells you what
happened:

| Code | Meaning |
|------|---------|
| 0 | No violations. |
| 1 | One or more `error`-severity violations. |
| 2 | CLI or infrastructure failure (bad URL, missing config, browser not found). |
| 3 | Only `warning`-severity violations. |

If Chrome is not on the standard path, point at it explicitly:

```bash
plumb lint https://example.com \
  --executable-path "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
```

See [Install Chromium](./install-chromium.md) for platform-specific
paths and the supported version range.

## 4. Switch to JSON when you wire it into CI

```bash
plumb lint https://example.com --format json > violations.json
```

The JSON output is byte-identical across runs given the same snapshot,
config, and rule set. That property is what makes Plumb safe to diff
in CI — there is no clock or hash-randomized output to wash through
`jq`.

For SARIF (GitHub code scanning, JetBrains, etc.):

```bash
plumb lint https://example.com --format sarif > plumb.sarif
```

## 5. Configure a rule

Tighten one rule and disable another. Add this to `plumb.toml`:

```toml
[rules."spacing/grid-conformance"]
severity = "error"   # promote from warning to error

[rules."edge/near-alignment"]
enabled = false       # silence this rule entirely
```

Re-run the lint. The exit code now flips to `1` when
`spacing/grid-conformance` fires, and `edge/near-alignment` no longer
shows up.

The full set of knobs lives in [Configuration](./configuration.md).
Per-rule details live under [Rules](./rules/overview.md) — each rule
documents the config it reads.

## 6. Hook into your editor

If your editor speaks JSON Schema (VS Code, JetBrains, Helix), generate
the canonical schema and point the editor at the local file:

```bash
plumb schema > plumb.schema.json
```

```jsonc
// .vscode/settings.json
{
  "evenBetterToml.schema.associations": {
    "plumb.toml": "./plumb.schema.json"
  }
}
```

You get hover docs, completion, and inline validation.

## 7. Wire it to your AI coding agent

```bash
plumb mcp
```

`plumb mcp` runs the Model Context Protocol server on stdio. See
[MCP server](./mcp.md) for the agent config snippets and the tool
list.

## What's next

- [Configuration](./configuration.md) — the full `plumb.toml` reference.
- [CLI](./cli.md) — every flag and subcommand.
- [Rules](./rules/overview.md) — per-rule docs.
- [MCP server](./mcp.md) — JSON-RPC surface and tool list.
