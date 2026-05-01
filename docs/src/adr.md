# Architecture decision records

ADRs capture the why behind non-obvious choices. The index lives at
[`docs/adr/`](https://github.com/aram-devdocs/plumb/tree/main/docs/adr).

## Current ADRs

- [`0001-bootstrap-conventions`](https://github.com/aram-devdocs/plumb/blob/main/docs/adr/0001-bootstrap-conventions.md)
  — workspace layout, lint policy, release pipeline.
- [`0002-chromium-version-range`](https://github.com/aram-devdocs/plumb/blob/main/docs/adr/0002-chromium-version-range.md)
  — exact-pin replaced by `MIN_SUPPORTED_CHROMIUM_MAJOR..=MAX_SUPPORTED_CHROMIUM_MAJOR`,
  with the contract for moving the upper bound.
- [`0003-dtcg-over-tailwind-css`](https://github.com/aram-devdocs/plumb/blob/main/docs/adr/0003-dtcg-over-tailwind-css.md)
  — DTCG is the canonical token interchange format; Tailwind and CSS
  custom properties remain adapters.
- [`0004-no-auto-fix-v0`](https://github.com/aram-devdocs/plumb/blob/main/docs/adr/0004-no-auto-fix-v0.md)
  — V0 reports violations and guidance but does not rewrite source.
- [`0005-chromium-not-bundled`](https://github.com/aram-devdocs/plumb/blob/main/docs/adr/0005-chromium-not-bundled.md)
  — Plumb requires a host-installed Chromium instead of shipping one.

## When to write an ADR

- Adding a new crate to the workspace.
- Changing the dependency hierarchy or lint policy.
- Introducing a new dependency with a non-MIT/Apache license.
- Adding a `[patch.crates-io]` entry.
- Changing the MCP protocol surface or output format in a
  non-backwards-compatible way.
- Any decision you'd want to re-justify 6 months from now.

Small bug fixes and straightforward features don't need an ADR.
