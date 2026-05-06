# plumb-e2e

End-to-end harness that drives the locally built `plumb` binary
against the framework fixtures under `e2e-sites/`.

## What it does

For each fixture:

1. Optionally runs `just build` inside the fixture directory.
2. Spawns a loopback HTTP server pointed at the fixture's `dist/`.
3. Runs `plumb lint http://127.0.0.1:<port>/ --config
   e2e-sites/plumb.toml --format json` three times.
4. Asserts the three outputs are byte-identical.
5. Parses the violations array and asserts:
   - Each `rule_id` listed in `expected.json::target_rules` has the
     count given by `expected.json::by_rule_id`.
   - The total target-rule count equals
     `expected.json::total_target_violations`.

Non-target rule violations are reported as `non_target` in the run
report but are not asserted on. This narrows the assertion surface to
the design-system invariants the matrix is built to validate, while
staying robust against incidental Chromium-side rendering differences.

## Usage

```sh
# Build plumb first.
cargo build --release -p plumb-cli

# Run against every fixture.
cargo run -p plumb-e2e -- --all

# Run against one fixture.
cargo run -p plumb-e2e -- --site html-css

# Skip the `just build` step (e.g. when `dist/` is already up to date).
cargo run -p plumb-e2e -- --all --no-build

# Use a non-default Chromium binary.
cargo run -p plumb-e2e -- --all --chrome-path /usr/bin/google-chrome-stable

# Override the plumb binary path.
cargo run -p plumb-e2e -- --all --plumb-bin /tmp/plumb
```

The simpler entry point is `just test-e2e`, which builds the binary
and runs the harness against every fixture.

## Why a separate crate

`plumb-e2e` is a dev-only harness (`publish = false`) and lives outside
`default-members` so `cargo build` ignores it. It depends on nothing
upstream of `plumb-cli` — it shells out to the binary rather than
linking the library so the failure modes match what end users see.

## Determinism

The harness asserts byte-equality across three back-to-back lint runs,
matching the `just determinism-check` invariant. Any drift fails the
run.
