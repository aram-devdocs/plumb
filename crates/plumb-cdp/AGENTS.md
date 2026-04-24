# crates/plumb-cdp — Chromium DevTools Protocol driver

See `/AGENTS.md` for repo-wide rules. This file scopes to `plumb-cdp`.

## Purpose

The only crate that drives a browser. Owns: `BrowserDriver` trait,
`Target`, `CdpError`, `ChromiumDriver` (real), `FakeDriver` (for the
`plumb-fake://` scheme), `PINNED_CHROMIUM_MAJOR` constant.

## Unique permissions

This is the ONLY crate in the workspace permitted to use `unsafe`.
The crate-level lints:

```rust
#![deny(unsafe_op_in_unsafe_fn)]
```

Every `unsafe` block carries a `// SAFETY:` comment documenting the
invariants. No exceptions.

## Non-negotiable invariants

- The pinned Chromium major version (`PINNED_CHROMIUM_MAJOR`) must match
  the PRD (§9, §16) and the docs. Changing it needs an ADR.
- `ChromiumDriver::snapshot` never touches the wall clock for anything
  that flows into `PlumbSnapshot` output. Snapshot content depends only
  on the page and the viewport.
- `FakeDriver` only accepts `plumb-fake://` URLs. The scheme is
  reserved and tests depend on it.
- No `println!`/`eprintln!` — use `tracing`.
- Library-crate error shape: `thiserror`-derived `CdpError`.

## Depends on

- `plumb-core` (types only).
- `chromiumoxide` (tokio-runtime feature).
- `tokio` (runtime + sync + process).

## Anti-patterns

- Capturing a screenshot when `PlumbSnapshot` is the only output needed.
- Logging raw CDP protocol bytes at anything below `trace` level.
- Spawning a new browser per `snapshot` call from the MCP server —
  `plumb-mcp` owns browser warmth; expose a constructor that accepts a
  pre-warmed `Browser` handle.
