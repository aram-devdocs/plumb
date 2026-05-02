# Release Readiness Local Kits

These checked-in fixtures back the offline side of the v0 release-readiness
gate.

## Contract

- Every kit is offline-only. No kit fetches remote CSS, fonts, images,
  scripts, or APIs.
- Every kit is deterministic. The files avoid wall-clock reads,
  randomness, and environment-sensitive content.
- Every kit is reusable from both release-readiness surfaces:
  `plumb lint file://...` on the CLI and `lint_url` over MCP.
- The manifest at `tests/fixtures/release-readiness/manifest.json` is the
  source of truth for kit names, file paths, purpose, and CLI/MCP reuse
  metadata.

## Kit Set

- `minimal` — smallest static page for baseline offline capture.
- `large-dom` — reuses the existing checked-in fixed DOM benchmark
  fixtures at 100, 1k, and 10k nodes.
- `responsive` — one fixture with stable mobile and desktop layout
  changes.
- `typography` — local font stacks, weights, and baseline-sensitive text.
- `contrast` — known pass and fail foreground/background pairings.
- `shadow-z-opacity-padding` — overlapping layers and spacing/token-like
  surfaces without network dependencies.
- `dynamic-wait` — deterministic delayed DOM mutation for `wait-for` and
  `wait-ms` style gates.
- `auth-storage` — local cookie, sessionStorage, and localStorage state
  hooks for future driver auth/storage checks.
- `mcp-inputs` — checked-in MCP request examples that target the local
  kits without live URLs.

## Direct Validation

Run either maintained entry point:

- `cargo xtask validate-release-readiness-kits`
- `bash tests/release-readiness-local-kits-validate.sh`
