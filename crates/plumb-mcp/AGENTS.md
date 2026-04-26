# crates/plumb-mcp — rmcp MCP server

See `/AGENTS.md` for repo-wide rules. This file scopes to `plumb-mcp`.

## Purpose

Model Context Protocol server exposed over stdio. Public surface:
`PlumbServer`, `run_stdio`, `McpError`, `EchoArgs`, `LintUrlArgs`.
Built on `rmcp 0.2.x` with the `#[tool_router]` + `#[tool]` +
`#[tool_handler]` macros.

## Protocol

- Protocol version: `ProtocolVersion::V_2024_11_05`.
- Transport: stdio (`rmcp::transport::stdio`).
- Server info: name `plumb`, version from `CARGO_PKG_VERSION`.
- Tool response contract (PRD §14.2):
  - `content[0]` — compact human text (one line per finding typical).
  - `content[1]` — structured JSON rendered from `plumb-format::mcp_compact`.
  - `isError: false` on non-error responses.

## Non-negotiable invariants

- `#![forbid(unsafe_code)]`.
- Every tool method validates its inputs and returns a typed error on
  malformed args — no `unwrap`/`expect`.
- Response payloads are bounded: `structuredContent` caps at ~10 KB
  (aggregate + cap on violations; see PRD §14.2).
- Deterministic output — no wall-clock, no random ordering, no env
  read inside a tool call.
- `allow(missing_docs)` is scoped only to the `#[tool_router]` impl
  block (the macro synthesizes helpers that can't be doc-commented).

## Adding a new tool

See `.agents/rules/mcp-tool-patterns.md`. Summary:

1. Add a `Deserialize + JsonSchema` struct for the tool's args.
2. Add a `#[tool(description = "…")]` async method on `PlumbServer`.
3. Add a protocol test case in `crates/plumb-cli/tests/mcp_stdio.rs`.
4. Update `docs/src/mcp.md` tool table.

Use the `09-mcp-tool-author` subagent for cookie-cutter execution.

## Depends on

- `plumb-core` (types; `test-fake` feature enabled so `lint_url` can
  serve the canned snapshot for `plumb-fake://` URLs).
- `plumb-cdp` (drives Chromium for real `http(s)://` URLs in `lint_url`).
- `plumb-format` (mcp_compact).
- `rmcp` (server + macros + transport-io + schemars features).
- `tokio`, `serde`, `serde_json`, `schemars`, `thiserror`, `tracing`.

## Anti-patterns

- Streaming the full `PlumbSnapshot` back in a tool response. Snapshots
  are huge and agent-harmful — always aggregate.
- A tool that mutates shared state. Every call is pure and re-entrant.
- Embedding rule docs verbatim in `explain_rule`. Read from
  `docs/src/rules/<slug>.md` so the book and the MCP response stay in
  sync.
