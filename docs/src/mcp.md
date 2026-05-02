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
| `lint_url` | Lint a URL. Args: `{ "url": "...", "detail": "compact" | "full" }`, where `detail` is optional and defaults to `compact`. Accepts `http(s)://` URLs (driven by the bundled Chromium driver) and `plumb-fake://hello` (canned snapshot for tests). On a Chromium launch failure the response is returned with `isError: true` and a single text block carrying the typed driver error. |
| `explain_rule` | Return canonical documentation and metadata for a Plumb rule by id. Args: `{ "rule_id": "<category>/<id>" }`. |
| `list_rules` | List every built-in Plumb rule with id, default severity, and one-line summary. No args. |
| `get_config` | Return resolved `plumb.toml` for a working directory as JSON. Memoized per `(path, mtime)`. |

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
      "text": "warning spacing/grid-conformance @ html > body [desktop]: …"
    }
  ],
  "isError": false,
  "structuredContent": {
    "violations": [ /* … */ ],
    "counts": { "error": 0, "warning": 1, "info": 0, "total": 1 }
  }
}
```

`detail: "compact"` returns the existing token-efficient payload shown
above. `detail: "full"` keeps the same text block and switches
`structuredContent` to the canonical JSON envelope from `plumb lint
<url> --format json`, including `plumb_version`, `run_id`, `stats`,
`summary`, and full per-violation fields. Full mode is rejected when
the serialized structured payload exceeds 50 KB.
