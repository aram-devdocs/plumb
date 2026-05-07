# Plumb

[![CI](https://github.com/aram-devdocs/plumb/actions/workflows/ci.yml/badge.svg)](https://github.com/aram-devdocs/plumb/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust 1.95+](https://img.shields.io/badge/rust-1.95%2B-orange.svg)](https://www.rust-lang.org)

**A deterministic design-system linter for rendered websites, not the code behind it.**

Plumb opens a web page in a headless browser at multiple viewports, extracts the computed DOM, and measures it against a declarative design-system spec. It emits structured, pixel-precise violations an AI coding agent can fix in one shot — "ESLint for rendered websites."

Plumb ships as a single Rust binary with two entry points:

- A **CLI** (`plumb lint <url>`) for developers and CI.
- An **MCP server** (`plumb mcp`) that exposes tools to AI coding agents (Claude Code, Cursor, Codex, Windsurf) via the Model Context Protocol.

## Install

```bash
# Install script (macOS / Linux / Windows)
curl -LsSf https://plumb.aramhammoudeh.com/install.sh | sh

# Cargo
cargo install plumb-cli

# Homebrew
brew install aram-devdocs/plumb/plumb

# npm
npm i -g plumb-cli
```

Per-channel notes, version pinning, and offline attestation verification live in the [Install](https://plumb.aramhammoudeh.com/install.html) page.

## Documentation

- [The Plumb Book](https://plumb.aramhammoudeh.com) — install, quick start, CLI, configuration, MCP, rules.
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [Changelog](CHANGELOG.md)

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Plumb by you, as defined in the Apache-2.0 license, shall be dual-licensed as above, without any additional terms or conditions.
