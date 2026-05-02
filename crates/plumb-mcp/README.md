# plumb-mcp

Model Context Protocol server for [Plumb](https://plumb.aramhammoudeh.com).

Exposes Plumb's linting capabilities over the
[MCP](https://modelcontextprotocol.io) protocol, so AI coding agents
(Claude Code, Cursor, Codex, Windsurf) can lint rendered pages and
retrieve rule explanations programmatically. `plumb mcp` uses stdio by
default and can also serve Streamable HTTP.

## Tools

| Tool | Description |
|------|-------------|
| `lint_url` | Lint a URL and return compact violations |
| `explain_rule` | Return the docs page for a rule |
| `echo` | Health-check / connectivity test |

## HTTP transport

`plumb mcp --transport http --port 4242` binds the server to
`127.0.0.1:<port>` and requires `PLUMB_MCP_TOKEN` at boot.

Security notes:

- There is no default token and no hardcoded fallback.
- Empty `PLUMB_MCP_TOKEN` values are rejected before the server starts.
- Every HTTP request must send `Authorization: Bearer <token>`.
- Missing or invalid bearer tokens return `401 Unauthorized`.
- The server logs the bind address, never the token value.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT License](LICENSE-MIT) at your option.
