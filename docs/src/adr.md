# Architecture decision records

ADRs capture the why behind non-obvious choices. The index lives at
[`docs/adr/`](https://github.com/aram-devdocs/plumb/tree/main/docs/adr).

## Current ADRs

- [`0001-bootstrap-conventions`](https://github.com/aram-devdocs/plumb/blob/main/docs/adr/0001-bootstrap-conventions.md)
  — workspace layout, lint policy, release pipeline.

## When to write an ADR

- Adding a new crate to the workspace.
- Changing the dependency hierarchy or lint policy.
- Introducing a new dependency with a non-MIT/Apache license.
- Adding a `[patch.crates-io]` entry.
- Changing the MCP protocol surface or output format in a
  non-backwards-compatible way.
- Any decision you'd want to re-justify 6 months from now.

Small bug fixes and straightforward features don't need an ADR.
