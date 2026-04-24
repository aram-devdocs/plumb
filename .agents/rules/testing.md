# Rule: Testing conventions

## Tools

- **`cargo nextest`** — the default test runner. Falls back to `cargo
  test` in `just test` when nextest isn't installed.
- **`insta`** — golden snapshot tests for anything structured. The rule
  engine's golden test is the canonical example.
- **`proptest`** — property-based testing. Use it for rule invariants
  (e.g. "the engine's output is always sorted") rather than point-fixture
  cases.
- **`assert_cmd`** — end-to-end CLI tests. Spawns the real binary.

## Layout

- **Unit tests** — in `#[cfg(test)]` modules inside the source file.
- **Integration tests** — in `crates/<name>/tests/*.rs`. One file per
  concern.
- **Fixtures** — under `crates/<name>/tests/fixtures/` (never `src/`).
- **Snapshots** — `insta` places them under `tests/snapshots/`. Commit
  the `.snap` files; never the `.snap.new`.

## Determinism

- Every integration test must be deterministic. No wall-clock
  assertions; no `tempfile::TempDir` paths in snapshots (strip them).
- `insta::Settings` can redact volatile fields — use it for paths and
  timings when you can't avoid them.

## Coverage

`just test-coverage` produces `coverage.lcov`. CI uploads to Codecov.
Target ≥ 80% for `plumb-core`; the `plumb-cdp` driver is excluded by the
`coverage.toml` glob until the real driver lands.

## Don't

- Don't skip tests with `#[ignore]`. Use `#[cfg(feature = "slow-tests")]`
  and gate in CI if needed.
- Don't mock the rule engine. It's pure — exercise it with real
  snapshots.
- Don't hit the network. Every fixture is local.
