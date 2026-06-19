# MCP server

`plumb mcp` runs an [MCP](https://modelcontextprotocol.io/) server on
stdio by default. AI coding agents (Claude Code, Cursor, Codex,
Windsurf) connect to it the same way they connect to any other MCP
server.

## Configuring your agent

Point your agent at the `plumb` binary. In Claude Code's `.mcp.json`:

```json
{
  "mcpServers": {
    "plumb": {
      "command": "plumb",
      "args": ["mcp"]
    }
  }
}
```

For local development against a source checkout:

```json
{
  "mcpServers": {
    "plumb": {
      "command": "cargo",
      "args": ["run", "--quiet", "-p", "plumb-cli", "--", "mcp"]
    }
  }
}
```

## Transports

Stdio remains the default transport. Existing agent configs that invoke
`plumb mcp` without extra flags do not change.

Plumb also supports Streamable HTTP:

```sh
plumb mcp --transport http --port 4242
```

HTTP boot requires `PLUMB_MCP_TOKEN` to be set to a non-empty bearer
token. If the variable is missing or empty, `plumb mcp --transport
http` refuses to boot.

Every HTTP request must send `Authorization: Bearer <token>`. Missing or
invalid tokens return `401 Unauthorized` with
`WWW-Authenticate: Bearer`.

Keep the token private. Do not log it, paste it into chat, or commit it
to the repository. The HTTP server binds to `127.0.0.1` and logs the
bind address, not the token value.

## Tools

| Tool | Description |
|------|-------------|
| `echo` | Smoke-test the transport. Echoes the `message` arg back. |
| `lint_url` | Lint a URL. Args: `{ "url": "...", "detail": "compact" | "full", "working_dir"?: "/abs/path" }`. `detail` is optional and defaults to `compact`; `working_dir` is optional and names the directory whose `plumb.toml` configures the lint (the server's own working directory is used when omitted). Accepts `http(s)://` URLs (driven by the bundled Chromium driver) and `plumb-fake://hello` (canned snapshot for tests). The compact payload aggregates duplicate violations into capped findings under a 10 KB `structuredContent` budget. On a Chromium launch failure the response is returned with `isError: true` and a single text block carrying the typed driver error. |
| `lint_page_html` | Lint a self-contained HTML string by rendering it in the same Chromium as `lint_url`, loaded as a `data:` URL so embedded `<style>` blocks and inline `style` attributes produce real computed styles and geometry. Args: `{ "html": "...", "base_url": "https://example.com/", "working_dir"?: "/abs/path" }`. External stylesheets and resources are not fetched — a relative `<link>` or `<img>` will not load, so use `lint_url` for a full page. Same aggregated compact response shape as `lint_url`. Hard-capped at 1 MiB of input and 10 000 elements, checked before rendering; oversized inputs surface as JSON-RPC `invalid_params` (-32602). When Chromium is unavailable the response carries `isError: true` with the driver error, never a misleading empty result. |
| `explain_rule` | Return canonical documentation and metadata for a Plumb rule by id. Args: `{ "rule_id": "<category>/<id>" }`. |
| `list_rules` | List every built-in Plumb rule with id, default severity, and one-line summary. No args. |
| `get_config` | Return resolved `plumb.toml` for a working directory as JSON. Memoized per `(path, mtime)`. |
| `compare_viewports` | Capture snapshots at 2+ viewports and return a deterministic diff: missing nodes, size changes above a pixel threshold, document-order reorderings, and computed-style differences. Args: `{ "url": "...", "viewports": [{ "name", "width", "height", "dpr" }, ...], "size_threshold_px"?: 4 }`. 10 KB `structuredContent` budget; aggregate counts plus a capped diff list. Full reference: [`compare_viewports`](./mcp/compare-viewports.md). |

## Resources

| Resource | Description |
|----------|-------------|
| `plumb://config` | Return the resolved `plumb.toml` for the MCP server's current working directory as JSON. The payload matches `get_config`'s `structuredContent` shape: `{ "config": { ... }, "source": "file" | "default", "path": "/abs/path/to/plumb.toml" }`. |

The response shape follows the MCP `content` + `structuredContent`
convention:

```json
{
  "content": [
    {
      "type": "text",
      "text": "warning spacing/grid-conformance ×1: `html > body` has off-grid padding-top 13px; …"
    }
  ],
  "isError": false,
  "structuredContent": {
    "by_rule": { "spacing/grid-conformance": 1 },
    "counts": { "error": 0, "info": 0, "total": 1, "warning": 1 },
    "findings": [
      {
        "rule_id": "spacing/grid-conformance",
        "severity": "warning",
        "message": "`html > body` has off-grid padding-top 13px; …",
        "instances": 1,
        "examples": ["html > body"],
        "fix": "Snap `padding-top` to the nearest spacing-grid value (12px).",
        "doc_url": "https://plumb.aramhammoudeh.com/rules/spacing-grid-conformance"
      }
    ],
    "truncated": false
  }
}
```

`detail: "compact"` returns the token-efficient payload shown above:
violations are grouped by rule and message shape into `findings`, each
with an `instances` count and up to three example selectors, so 600
identical low-contrast rows collapse to one entry instead of 600.
`counts` and `by_rule` always reflect every violation, and `truncated`
is `true` when groups were dropped to fit the 10 KB budget. `detail:
"full"` keeps the same text block and switches `structuredContent` to
the canonical JSON envelope from `plumb lint <url> --format json`,
including `plumb_version`, `run_id`, `stats`, `summary`, and full
per-violation fields. Full mode is rejected when the serialized
structured payload exceeds 50 KB.

## Common issues

These come up across every agent integration. Per-agent pages
([Claude Code](./mcp/claude.md), [Cursor](./mcp/cursor.md),
[Codex](./mcp/codex.md)) link here instead of repeating the list.

**PATH resolution.** Many agents launch the MCP server from a GUI
process that does not inherit your shell's full `PATH`. macOS GUI apps
are the usual offender. If `plumb` is installed somewhere like
`~/.cargo/bin` and the agent reports "command not found", use an
absolute path in the agent config:

```json
{
  "mcpServers": {
    "plumb": {
      "command": "/Users/you/.cargo/bin/plumb",
      "args": ["mcp"]
    }
  }
}
```

For agents that register the server with a CLI (e.g. `codex mcp add`),
pass the absolute binary path the same way.

**Working directory.** The MCP server resolves `plumb.toml` from the
working directory where the agent launches it — usually the project
root. Place `plumb.toml` there, or call `get_config` with an explicit
path argument.

**Large responses.** `lint_url` defaults to `detail: "compact"`, which
is the token-efficient payload. `detail: "full"` returns the canonical
`--format json` envelope and is hard-capped at 50 KB of
`structuredContent`; oversized responses are rejected with a JSON-RPC
error. For pages with many violations, stay on `compact` and request
`full` only for specific follow-ups.

**Tool approval prompts.** Some agents prompt on first MCP tool use.
Accept the prompt to allow Plumb tools.

**Sandboxed network access.** Agents that run inside a sandbox (Codex
is one) may restrict outbound network. `lint_url` against a real URL
needs the sandbox to allow Chromium to reach the target host.
`plumb-fake://hello` works without network access and is useful for
verifying the tool chain end to end.
