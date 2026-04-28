# ADR 0002 — Chromium supported version range

**Status:** Accepted
**Date:** 2026-04-28
**Deciders:** Aram Hammoudeh
**Supersedes:** the exact-pin gate originally landed in [`plumb-cdp`](https://github.com/aram-devdocs/plumb/tree/main/crates/plumb-cdp).

## Context

`plumb-cdp` validates that the Chromium binary it just launched is one
Plumb has actually been tested against. Pinning matters because the
`PlumbSnapshot` output format is part of the determinism contract (PRD
§9, §16): if a future Chromium changes how `DOMSnapshot.captureSnapshot`
flattens nodes or rounds computed-style values, the byte-identical
guarantee breaks silently.

The first version of the gate hardcoded a single major:

```rust
const PINNED_CHROMIUM_MAJOR: u32 = 131;
if found == PINNED_CHROMIUM_MAJOR { … }
```

Two issues fell out of that choice:

- **[#117](https://github.com/aram-devdocs/plumb/issues/117)** — `plumb
  lint <real-url>` refused to run on every developer machine running a
  current Chrome or Chromium build, because Chromium 131 (November 2024)
  was no longer the default install. The Phase 1 acceptance criterion
  (real-page lint against `plumb.aramhammoudeh.com`) could not be met
  without re-installing an old binary.
- **[#118](https://github.com/aram-devdocs/plumb/issues/118)** — the
  `e2e-chromium` test suite caught `ChromiumNotFound` and
  `UnsupportedChromium` and returned `Ok(())`. CI reported `ok` whether
  Chromium was driven or not, which masked #117 entirely until the CLI
  was tried by hand.

PR [#126](https://github.com/aram-devdocs/plumb/pull/126) shipped the
remediation. It merged before the Claude review thread reached
approval, so this ADR is the durable record of the decision and the
contract that future range bumps follow.

## Decision

### 1. Replace the exact-pin gate with an inclusive major-version range

`plumb-cdp` now exposes two constants:

```rust
pub const MIN_SUPPORTED_CHROMIUM_MAJOR: u32 = 131;
pub const MAX_SUPPORTED_CHROMIUM_MAJOR: u32 = 150;
```

`validate_chromium_product_major` accepts any major in
`MIN_SUPPORTED_CHROMIUM_MAJOR..=MAX_SUPPORTED_CHROMIUM_MAJOR`. Out-of-range
binaries surface as `CdpError::UnsupportedChromium { min_supported,
max_supported, found }` so the user sees both bounds.

The lower bound is the oldest major Plumb has snapshotted against
end-to-end. The upper bound is the newest major the e2e suite has been
run against. The two are intentionally separate constants — the lower
bound moves only when Plumb drops support for an older browser; the
upper bound moves whenever the e2e suite is re-run against a new
major.

### 2. The version-range contract

Bumping `MAX_SUPPORTED_CHROMIUM_MAJOR` is not a docs change. It is a
verification step. Before raising it, the contributor MUST:

1. Install a Chromium binary at the candidate major.
2. Run `cargo test -p plumb-cdp --features e2e-chromium` end-to-end
   against that binary (the suite hard-fails when Chromium is missing
   per item 3 below).
3. Confirm `chromium_driver_snapshot_is_byte_identical` — the
   three-run determinism check — passes against the candidate major.
4. Land the constant bump in the same PR as the verification work, so
   `git blame MAX_SUPPORTED_CHROMIUM_MAJOR` points at the run that
   confirmed it.

Lowering `MIN_SUPPORTED_CHROMIUM_MAJOR` follows the same shape, but in
practice the lower bound only moves upward — once an older major drops
out of validation, it stays dropped.

Range widening that crosses a Chromium release whose CDP changed
DOMSnapshot output (e.g. a new computed-style normalization) requires
its own ADR. The constants are the gate; the ADR is the rationale.

### 3. e2e-chromium tests fail loud by default

The driver-contract suite no longer treats `ChromiumNotFound` or
`UnsupportedChromium` as a silent pass. Both propagate and fail the
test unless the user has set `PLUMB_E2E_CHROMIUM_SKIP=1`, in which case
the skip path emits a `tracing::warn!` line that names the underlying
error and the env var that triggered the skip. CI does not set the env
var; the workspace test job no longer activates the `e2e-chromium`
feature, so out-of-range hosts do not silently pass. Hosts that
genuinely lack Chromium (constrained CI runners, sandboxed local envs)
opt in explicitly.

The error name change from `ChromiumNotInstalled` / `WrongMajor`
patterns to `ChromiumNotFound` / `UnsupportedChromium` lines up with
the install hint already returned by `chromium_install_hint`, so users
get one consistent vocabulary.

### 4. The current range is `131..=150`

131 is the lower bound because it was the validated major when Plumb's
DOMSnapshot integration first landed. 150 is the upper bound because
it is the highest Chrome major shipping at the time the range was
introduced and the e2e suite was confirmed against a host with Chrome
139 in that window (PR #126 test plan). Future PRs that test against a
newer major bump the upper bound under the contract above.

## Consequences

- `plumb lint <real-url>` works on any host running a current Chrome or
  Chromium without an exotic install.
- The supported window is explicit and queryable. Library callers and
  the install docs both reference `MIN_SUPPORTED_CHROMIUM_MAJOR` and
  `MAX_SUPPORTED_CHROMIUM_MAJOR` instead of duplicating a literal.
- A Chromium release that drifts out of the upper bound is an obvious,
  actionable error rather than a silent green CI run.
- Bumping the upper bound has a procedural cost (re-run the e2e suite,
  land the constant in the same PR). That cost is the point — it is
  the validation that keeps the determinism guarantee honest.
- The `e2e-chromium` test job is not part of the workspace `cargo test`
  job. Hosts that want full browser coverage opt in via
  `--features e2e-chromium`, and out-of-range or missing browsers fail
  the run unless the user explicitly skips with
  `PLUMB_E2E_CHROMIUM_SKIP=1`.

## References

- [PR #126](https://github.com/aram-devdocs/plumb/pull/126) — the
  remediation.
- [Issue #117](https://github.com/aram-devdocs/plumb/issues/117) — exact-pin
  blocked all real-URL lints on modern systems.
- [Issue #118](https://github.com/aram-devdocs/plumb/issues/118) —
  `e2e-chromium` tests silently passed without Chromium.
- `docs/local/prd.md` §9, §16 — determinism invariants and the
  re-validation requirement when the Chromium upper bound moves.
- `docs/src/install-chromium.md` — user-facing install instructions
  pinned to the same range.
- `crates/plumb-cdp/src/lib.rs` — the constants and
  `validate_chromium_product_major`.
- `crates/plumb-cdp/CLAUDE.md` — the invariant that range changes
  require an ADR.
