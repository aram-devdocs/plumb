# plumb-format

Output formatters for [Plumb](https://plumb.aramhammoudeh.com) violations —
pretty, JSON, SARIF, and MCP-compact.

Each formatter is a pure function: violations in, formatted string out.
No filesystem, no network, no wall-clock — deterministic by construction.

## Formats

| Format | Function | Use case |
|--------|----------|----------|
| Pretty | `pretty` | Human-readable terminal output |
| JSON | `json` | Machine-readable, CI integrations |
| SARIF | `sarif` | GitHub Code Scanning, IDE extensions |
| MCP-compact | `mcp_compact` | Token-efficient AI agent responses |

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT License](LICENSE-MIT) at your option.
