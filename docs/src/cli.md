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
| `--suggest-ignores` | Append a suggested `.plumbignore` block. See [`--suggest-ignores`](./cli/suggest-ignores.md). |
| `--auto-fetch-chromium` | Download Chrome-for-Testing into Plumb's cache when no `--executable-path` is given and no system Chromium is detected. See [Install Chromium](./install-chromium.md#auto-fetch-opt-in). |

Exit codes:

| Code | Meaning |
|------|---------|
| 0 | No violations, or only `info`-severity violations. |
| 1 | One or more `error`-severity violations. |
| 2 | CLI or infrastructure failure (bad URL, missing config, etc.). |
| 3 | Only `warning`-severity violations (no errors). |

`info`-severity violations are reported in the output but never fail
the run on their own — the bucket is reserved for advisory checks
(suggestions, low-confidence fixes) you might want surfaced without
breaking CI. Use `[rules."<id>"] severity = "warning"` to promote a
specific advisory rule into the CI-failing tier.

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

### `plumb watch [<url>]`

Re-run `plumb lint` whenever a file under the current directory (or
`--path <dir>`) changes. The first cycle runs immediately on startup so
you get a baseline; subsequent cycles fire after a 250 ms debounce
window collapses each burst of editor events into a single re-lint.

Press Ctrl-C to exit. The status line on stderr after every cycle
records the cycle's shape:

```text
watching… changed: 3 files; lint: 2 violations; took 412 ms
```

Stdout carries the rendered lint output (`pretty` by default;
`--format json` and `--format sarif` work the same as `lint`), so you
can tail the watch output with the JSON consumer of your choice
without losing the status line.

Watch flags mirror `plumb lint`'s. One extra:

| Flag | Description |
|------|-------------|
| `--path <dir>` | Directory to watch. Repeatable. Defaults to CWD. |

A `.plumbignore` file at the root of any watched directory excludes
paths whose substring matches any of its lines. Blank lines and lines
starting with `#` are ignored. The defaults already skip `.git/`,
`target/`, `node_modules/`, `.idea/`, and `.vscode/`.
