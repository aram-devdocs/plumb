<!-- aram-ai-global v3 multi-agent stack now drives PRs end-to-end. -->
# Plumb

[![CI](https://github.com/aram-devdocs/plumb/actions/workflows/ci.yml/badge.svg)](https://github.com/aram-devdocs/plumb/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust 1.95+](https://img.shields.io/badge/rust-1.95%2B-orange.svg)](https://www.rust-lang.org)

**A deterministic design-system linter for rendered websites, not the code behind it.**

Plumb opens a web page in a headless browser at multiple viewports, extracts the computed DOM, and measures it against a declarative design-system spec. It emits structured, pixel-precise violations an AI coding agent can fix in one shot — "ESLint for rendered websites."

Plumb ships as a single Rust binary with two entry points:

- A **CLI** (`plumb lint <url>`) for developers and CI/CD.
- An **MCP server** (`plumb mcp`) that exposes tools to AI coding agents (Claude Code, Cursor, Codex, Windsurf) via the Model Context Protocol.

## Status

Pre-alpha. The walking skeleton is in place; real rules and the real browser driver land in subsequent PRs. See the [PRD](docs/local/prd.md) (local-only) for the full product scope.

## Quick start

> Install commands are placeholders until the first release. For now, build from source:
>
> ```bash
> git clone https://github.com/aram-devdocs/plumb
> cd plumb
> just setup
> just build-release
> ```

After the first release:

```bash
# macOS / Linux / Windows
curl -LsSf https://plumb.aramhammoudeh.com/install.sh | sh

# Cargo
cargo install plumb-cli

# Homebrew (coming soon)
brew install plumb-dev/tap/plumb
```

## Documentation

- [The Plumb Book](https://plumb.aramhammoudeh.com) (coming soon — built from `docs/src/`)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [Code of conduct](CODE_OF_CONDUCT.md)
- [Changelog](CHANGELOG.md)

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Plumb by you, as defined in the Apache-2.0 license, shall be dual-licensed as above, without any additional terms or conditions.
