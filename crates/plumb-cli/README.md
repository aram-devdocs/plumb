# plumb-cli

A deterministic design-system linter for rendered websites — CLI + MCP server for AI coding agents.

## Install

```bash
# npm (Node-tooling shops)
npm i -g plumb-cli

# Cargo (Rust toolchain)
cargo install plumb-cli

# Homebrew (macOS / Linuxbrew)
brew install aram-devdocs/plumb/plumb

# Install script (macOS / Linux)
curl -LsSf https://plumb.aramhammoudeh.com/install.sh | sh
```

> **Intel Mac**: native binaries return when [#269](https://github.com/aram-devdocs/plumb/issues/269) closes. Use `cargo install plumb-cli` in the meantime.

## What it does

Plumb opens a page in a headless browser at multiple viewports, measures the computed DOM against a declared design-system spec, and emits structured, pixel-precise violations an AI coding agent can fix in one shot.

Two entry points:

- `plumb lint <url>` — for developers and CI.
- `plumb mcp` — Model Context Protocol server for Claude Code, Cursor, Codex, Windsurf.

## Quick usage

```bash
# Sanity check — runs against a canned snapshot, no browser needed
plumb lint plumb-fake://hello

# Lint a real URL
plumb lint https://example.com

# Generate SARIF for GitHub Code Scanning
plumb lint https://example.com --format sarif --output plumb.sarif

# Run as MCP server (stdio)
plumb mcp

# Explain a rule
plumb explain spacing/grid-conformance
```

## Documentation

Full docs: <https://plumb.aramhammoudeh.com>

## License

Dual-licensed under either:

- Apache License, Version 2.0
- MIT License

at your option.
