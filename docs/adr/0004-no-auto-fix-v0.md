# ADR 0004 — No auto-fix in V0

**Status:** Accepted
**Date:** 2026-05-01
**Deciders:** Aram Hammoudeh

## Context

Plumb's V0 goal is a deterministic linter for rendered websites. The
report tells the user what broke design-system policy and where it was
observed. The separate question is whether V0 should also rewrite the
source automatically.

Auto-fix sounds attractive, but it pulls the product into a different
problem space. A rendered violation does not point back to one obvious
source edit. The same DOM can come from handwritten CSS, CSS-in-JS,
Tailwind utilities, generated templates, or a component library the
user does not own.

That gap matters more in Plumb than it does in a source linter. V0
looks at the browser result, not the authoring tool that produced it.

## Decisions

### 1. V0 reports violations only; it does not rewrite source

Plumb V0 stops at diagnostics. It can describe the offending selector,
viewport, measured value, and token or scale that would have matched.
It does not edit CSS, templates, or component code.

**Rationale.** The lint result is deterministic. A safe source edit is
not. Plumb would need framework-specific mappers, ownership rules, and
conflict handling before it could claim that a fix is correct.

### 2. Guidance is allowed; mutation is not

Rules may continue to emit human-readable guidance and plausible fix
metadata in the report model. That does not make V0 an auto-fixer.

**Rationale.** Suggesting a likely replacement value is useful. Writing
that change back to a file, then claiming it preserved intent, is a
much stronger promise.

### 3. Any future auto-fix work needs its own product boundary

If Plumb adds source mutation later, that work needs a separate design
that covers source ownership, supported frameworks, dry-run behavior,
review flow, and rollback.

**Rationale.** "Lint plus maybe patch something" is not a small
extension to the current architecture. It is a second product surface.

## Consequences

- V0 stays focused on deterministic detection and reporting.
- The CLI and MCP surfaces remain easier to reason about because they
  never mutate project files.
- Users still need to make the final change themselves, which is extra
  work but also the safer contract for rendered-site linting.
- Future auto-fix proposals must justify how they bridge a browser-side
  finding back to a trustworthy source edit.

## References

- [Issue #62](https://github.com/aram-devdocs/plumb/issues/62) — ADR
  tracking issue.
- `docs/runbooks/roadmap-spec.yaml` — V0 non-goals include auto-fix
  source.
- `crates/plumb-core/src/report.rs` — report model and guidance fields.
- `docs/src/rules/overview.md` — rule docs describe what Plumb reports.
