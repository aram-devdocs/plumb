# plumb-mcp

Model Context Protocol server for [Plumb](https://plumb.aramhammoudeh.com).

Exposes Plumb's linting capabilities over stdio using the
[MCP](https://modelcontextprotocol.io) protocol, so AI coding agents
(Claude Code, Cursor, Codex, Windsurf) can lint rendered pages and
retrieve rule explanations programmatically.

## Tools

| Tool | Description |
|------|-------------|
| `lint_url` | Lint a URL and return compact violations |
| `explain_rule` | Return the docs page for a rule |
| `echo` | Health-check / connectivity test |

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT License](LICENSE-MIT) at your option.
