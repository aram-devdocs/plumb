# CLI

The `plumb` binary is the primary entry point for developers and CI.

## Subcommands

### `plumb lint <url>`

Lint a URL. The `plumb-fake://hello` URL scheme is still available for
local smoke tests. Real URLs require a Chrome or Chromium binary whose
major version falls in Plumb's supported range (see
[Install Chromium](install-chromium.md)).

| Flag | Description |
|------|-------------|
| `-c`, `--config <path>` | Config file path. Defaults to `plumb.toml` in CWD. |
| `--executable-path <path>` | Chrome or Chromium binary to use instead of auto-detection. |
| `--format <pretty\|json\|sarif>` | Output format. Default: `pretty`. |
| `--output <path>` | Write rendered output to a file instead of stdout. |
| `-v`, `--verbose` | Increase log verbosity. `-vv` for trace. |

Exit codes:

| Code | Meaning |
|------|---------|
| 0 | No violations. |
| 1 | One or more `error`-severity violations. |
| 2 | CLI or infrastructure failure (bad URL, missing config, etc.). |
| 3 | Only `warning`-severity violations. |

### `plumb init`

Write a starter `plumb.toml` to the current directory. Pass `--force` to
overwrite.

Pass `--from <path>` to bootstrap from an existing project tree. The
walker discovers CSS custom properties (`:root { --token: value; }`),
Tailwind config files, and DTCG token JSON, and folds them into a
starter config. Output is deterministic — two runs against the same
tree produce byte-identical files. Token names that don't match a
known prefix (e.g. `--space-*`, `--color-*`, `--radius-*`) are skipped;
edit the file to fill the gaps.

### `plumb explain <rule-id>`

Print the long-form documentation for a rule. The argument is a slash-
separated id like `spacing/grid-conformance`.

### `plumb schema`

Emit the JSON Schema for `plumb.toml` on stdout. Redirect into a file
and point your editor at it for autocomplete:

```bash
plumb schema > plumb.schema.json
```

### `plumb mcp`

Run the Model Context Protocol server on stdio. See [MCP server](./mcp.md).
