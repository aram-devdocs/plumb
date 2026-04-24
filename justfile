# Plumb task runner.
#
# Every gate here has a corresponding CI job. Running `just validate`
# locally must pass before pushing — it mirrors CI exactly.
#
# There is no `SKIP_VALIDATION`, no `--no-verify`, no bypass. If a check
# fails, fix the cause.

set shell := ["bash", "-euo", "pipefail", "-c"]
set dotenv-load := false

# Default target — list available recipes.
default:
    @just --list --unsorted

# One-time developer setup. Installs git hooks, verifies toolchain and deps.
setup:
    @echo "▸ Installing git hooks via lefthook…"
    @command -v lefthook >/dev/null 2>&1 || { echo "✖ lefthook not installed. See CONTRIBUTING.md."; exit 1; }
    lefthook install
    @echo "▸ Verifying Rust toolchain…"
    @rustc --version
    @cargo --version
    @echo "▸ Done."

# Format the workspace.
fmt:
    cargo fmt --all

# All static checks — fmt + clippy with zero tolerance. Matches CI preflight.
check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Full test run.
test:
    cargo nextest run --workspace --all-features 2>/dev/null || cargo test --workspace --all-features

# Test with coverage via cargo-llvm-cov. Output: coverage.lcov.
test-coverage:
    cargo llvm-cov --workspace --all-features --lcov --output-path coverage.lcov

# Review insta snapshots interactively.
snapshot-review:
    cargo insta review

# Build the workspace (debug).
build:
    cargo build --workspace

# Build release artifacts.
build-release:
    cargo build --workspace --release

# Run the CLI against the walking-skeleton fake URL.
run-cli *ARGS:
    cargo run --quiet -p plumb-cli -- {{ARGS}}

# Run the MCP server on stdio (dev mode).
run-mcp:
    cargo run --quiet -p plumb-cli -- mcp

# Emit the JSON Schema for plumb.toml.
schema:
    cargo run --quiet -p plumb-cli -- schema

# Run cargo-doc + mdbook build.
doc:
    cargo doc --workspace --no-deps
    @command -v mdbook >/dev/null 2>&1 && mdbook build || echo "▸ mdbook not installed; skipping book build."

# Live-reload the Book.
serve-docs:
    mdbook serve --open

# Security + license audit.
audit:
    cargo audit
    cargo deny check

# Determinism check: run the CLI three times and byte-diff the output.
#
# `plumb lint` exits 3 when only warnings are present, which is the
# walking-skeleton steady state. Bash `set -e` would treat that as a
# failure, so each invocation is wrapped to swallow the expected code.
determinism-check:
    @echo "▸ Determinism check (3 runs, byte-diff JSON output)…"
    @cargo run --quiet -p plumb-cli -- lint plumb-fake://hello --format json > /tmp/plumb-det-1.json || [ $? -eq 3 ]
    @cargo run --quiet -p plumb-cli -- lint plumb-fake://hello --format json > /tmp/plumb-det-2.json || [ $? -eq 3 ]
    @cargo run --quiet -p plumb-cli -- lint plumb-fake://hello --format json > /tmp/plumb-det-3.json || [ $? -eq 3 ]
    @diff -q /tmp/plumb-det-1.json /tmp/plumb-det-2.json
    @diff -q /tmp/plumb-det-2.json /tmp/plumb-det-3.json
    @echo "▸ OK — all three runs produced byte-identical output."

# Size guard: stripped release binary must stay under 25 MiB.
size-guard:
    cargo build --release -p plumb-cli
    @bytes=$(wc -c < target/release/plumb | tr -d ' '); \
    limit=26214400; \
    if [ "$bytes" -ge "$limit" ]; then \
        echo "✖ binary size $bytes exceeds 25 MiB ($limit)"; exit 1; \
    else \
        echo "▸ binary size $bytes bytes — under budget."; \
    fi

# Full pre-push mirror — matches ci.yml exactly. No bypass.
validate: check test determinism-check audit
    @echo "▸ All gates passed."
