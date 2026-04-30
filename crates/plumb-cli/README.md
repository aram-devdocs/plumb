# plumb-cli

The `plumb` command-line interface — a deterministic design-system linter
for rendered websites.

This crate builds the `plumb` binary. For library usage, depend on
[`plumb-core`](https://crates.io/crates/plumb-core) instead.

## Usage

```bash
# Lint a URL at default viewports
plumb lint https://example.com

# Output as SARIF for GitHub Code Scanning
plumb lint https://example.com --format sarif

# Start the MCP server for AI agents
plumb mcp

# Explain a rule
plumb explain spacing/scale-conformance
```

## Install

```bash
cargo install plumb-cli
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT License](LICENSE-MIT) at your option.
