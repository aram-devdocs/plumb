# MCP server

`plumb mcp` runs an [MCP](https://modelcontextprotocol.io/) server on
stdio. AI coding agents (Claude Code, Cursor, Codex, Windsurf) connect
to it the same way they connect to any other MCP server.

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

## Tools

| Tool | Description |
|------|-------------|
| `echo` | Smoke-test the transport. Echoes the `message` arg back. |
| `lint_url` | Lint a URL. Accepts `http(s)://` URLs (driven by the bundled Chromium driver) and `plumb-fake://hello` (canned snapshot for tests). On a Chromium launch failure the response is returned with `isError: true` and a single text block carrying the typed driver error. |
| `explain_rule` | Return canonical documentation and metadata for a Plumb rule by id. Args: `{ "rule_id": "<category>/<id>" }`. |
| `get_config` | Return resolved `plumb.toml` for a working directory as JSON. Memoized per `(path, mtime)`. |

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
