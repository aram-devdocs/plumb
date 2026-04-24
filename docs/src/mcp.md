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

## Walking-skeleton tools

| Tool | Description |
|------|-------------|
| `echo` | Smoke-test the transport. Echoes the `message` arg back. |
| `lint_url` | Lint a URL. Accepts `plumb-fake://hello` only until the Chromium driver lands. |

The response shape follows the MCP `content` + `structuredContent`
convention:

```json
{
  "content": [
    {
      "type": "text",
      "text": "warning placeholder/hello-world @ html > body [desktop]: …"
    }
  ],
  "isError": false,
  "structuredContent": {
    "violations": [ /* … */ ],
    "counts": { "error": 0, "warning": 1, "info": 0, "total": 1 }
  }
}
```
