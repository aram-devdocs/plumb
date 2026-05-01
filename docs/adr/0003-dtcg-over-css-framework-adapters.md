# ADR 0003 — DTCG over CSS framework adapters

**Status:** Accepted
**Date:** 2026-05-01
**Deciders:** Aram Hammoudeh

## Context

Plumb needs a portable way to import design tokens into `plumb.toml`.
V0 already supports several token sources: DTCG documents, Tailwind
config, and CSS custom properties. The open question was which source
should anchor the model and the docs.

Choosing a CSS framework adapter as the primary format would tie
Plumb's token story to one toolchain and one set of conventions.
Choosing CSS custom properties as the primary format would keep the
input simple, but it would also collapse token semantics into string
parsing. Raw CSS variables do not carry the same structure as token
files with explicit type, alias, and grouping information.

Plumb's rule engine wants the opposite: one canonical token model that
can absorb multiple upstream formats without changing rule behavior.

## Decisions

### 1. DTCG is the canonical token interchange format for V0

Plumb treats DTCG as the source format that best represents token
intent across tools. When a team already has DTCG exports, Plumb can
import them directly. When a team starts from another source, Plumb
normalizes that source into the same internal config shape.

**Rationale.** DTCG is vendor-neutral and typed. It models aliases,
groups, and token kinds directly instead of forcing Plumb to infer them
from framework conventions.

### 2. CSS framework adapters remain adapters, not the contract

Framework-specific imports stay because they remove setup friction for
teams that already keep spacing, color, and type scales in existing
tooling such as `tailwind.config.*`. That support does not make any one
framework the canonical token surface.

**Rationale.** Tailwind is common, but it is still one framework with
its own evaluation model, Node dependency, and theme conventions.
Plumb should read framework configs without making every non-framework
user carry those assumptions.

### 3. CSS custom properties remain a lowest-common-denominator input

CSS variables are still useful, especially for teams that expose tokens
on `:root` but do not maintain a separate token file. They are not the
main contract for V0.

**Rationale.** CSS custom properties are easy to extract, but they are
not enough on their own. They do not reliably encode token type,
semantic grouping, or alias structure, and naming conventions vary
widely between codebases.

### 4. Rules consume the normalized Plumb config, not the original source

No rule should care whether a token came from DTCG, a CSS framework
adapter, or CSS variables. Importers are responsible for producing the
same normalized config shape before linting starts.

**Rationale.** This keeps rule behavior deterministic and keeps source
adapters from leaking framework-specific edge cases into `plumb-core`.

## Consequences

- DTCG becomes the format Plumb can describe in architecture docs
  without also endorsing one framework.
- Framework adapters stay important for onboarding, but they are
  documented as compatibility paths rather than the core contract.
- CSS-variable import stays useful for simple sites, though teams with
  richer token systems get better fidelity from DTCG.
- Future adapters should map into the same config shape. They should
  not introduce source-specific rule behavior.

## References

- [Issue #62](https://github.com/aram-devdocs/plumb/issues/62) — ADR
  tracking issue.
- `crates/plumb-config/src/dtcg.rs` — DTCG adapter.
- `crates/plumb-config/src/tailwind/mod.rs` — Tailwind adapter.
- `crates/plumb-config/src/css_props.rs` — CSS custom property loader.
- `docs/runbooks/phase-2-spec.yaml` — token adapter scope for DTCG,
  Tailwind, and CSS custom properties.
