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
| `--viewport <name>` | Restrict the run to the named viewport. Repeatable. |
| `--selector <css>` | Restrict linting to a CSS subtree. |
| `--wait-for <css>` | Wait for a selector to appear before capturing. |
| `--wait-ms <ms>` | Sleep N ms after navigation (and after `--wait-for`). |
| `--cookie <name=value>` | Pre-set a cookie before navigation. Repeatable. |
| `--header <name: value>` | Add an extra HTTP header to every request. Repeatable. |
| `--auth-script <path>` | Evaluate a `.js` file on every new document. Path MUST resolve under CWD. |
| `--storage-state <path>` | Load a Playwright `storage-state.json`. |
| `--disable-animations [bool]` | CSS animation/transition killer. Default `true`. |
| `--hide-scrollbars [bool]` | CSS scrollbar killer. Default `true`. |
| `--dpr <factor>` | Pin device-pixel ratio for `Emulation.setDeviceMetricsOverride`. |

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
