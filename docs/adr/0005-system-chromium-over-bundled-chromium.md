# ADR 0005 — System Chromium over bundled Chromium

**Status:** Accepted
**Date:** 2026-05-01
**Deciders:** Aram Hammoudeh

## Context

Plumb lints rendered pages, so it needs a real browser engine. For V0,
that engine is Chromium driven over CDP. The question is whether the
`plumb` binary should ship its own browser or require one that is
already installed on the host.

Bundling Chromium would make first-run setup simpler in some cases, but
it would also turn every release into a browser-distribution problem.
Plumb would need to ship large platform-specific artifacts, decide when
to refresh them, and explain what happens when the bundled browser lags
security updates or drifts from the range the project actually tests.

ADR 0002 already defines the supported Chromium major-version contract.
This ADR answers a different question: who provides the browser bits in
V0.

## Decisions

### 1. V0 uses system Chromium

Plumb requires the user to install Chrome or Chromium separately. The
CLI and MCP server detect a local browser or accept an explicit
executable path.

**Rationale.** This keeps the shipped artifact small and keeps browser
installation under the host environment's normal package or update
flow.

### 2. The project pins support, not distribution

Plumb owns the tested version range and the error messaging around it.
It does not own downloading, packaging, or updating Chromium in V0.

**Rationale.** Version support is already a determinism concern, which
ADR 0002 covers. Distribution adds a different maintenance burden that
does not help the core linting contract.

### 3. Missing or unsupported browsers fail with guidance

When Plumb cannot find Chromium, or finds a major outside the supported
range, it returns a typed error with installation guidance instead of
trying to fetch a browser behind the user's back.

**Rationale.** A clear failure is easier to trust and easier to debug
than an implicit download path with platform-specific side effects.

## Consequences

- Installation has one extra prerequisite: a compatible Chrome or
  Chromium binary must exist on the host.
- Releases stay smaller and simpler because Plumb distributes one Rust
  binary rather than a browser bundle per platform.
- Browser updates happen through the user's normal OS or package-manager
  flow, while Plumb keeps control over the supported range.
- If V1 adds auto-fetch or managed browser installs, that work needs a
  separate ADR because it changes packaging, support, and security
  expectations.

## References

- [Issue #62](https://github.com/aram-devdocs/plumb/issues/62) — ADR
  tracking issue.
- [ADR 0002](./0002-chromium-version-range.md) — supported Chromium
  major-version contract.
- `docs/src/install-chromium.md` — install instructions and supported
  version range.
- `docs/src/install.md` — user-facing install flow.
- `docs/runbooks/roadmap-spec.yaml` — V0 non-goals include embedded
  Chromium.
