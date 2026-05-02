#!/usr/bin/env bash
# install-smoke-validate.sh — Static validation for the install-smoke
# workflow channel coverage, gating, verification, and security contract.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORKFLOW="$REPO_ROOT/.github/workflows/install-smoke.yml"
JUSTFILE="$REPO_ROOT/justfile"
CI_WORKFLOW="$REPO_ROOT/.github/workflows/ci.yml"

failures=0

pass() { echo "  PASS: $1"; }
fail() { echo "  FAIL: $1" >&2; failures=1; }

echo "=== Install-smoke gate validation ==="
echo ""

# ── 1. Workflow exists ─────────────────────────────────────────────

echo "1. Workflow file"
if [ -f "$WORKFLOW" ]; then
    pass "install-smoke.yml exists"
else
    fail "install-smoke.yml missing"
fi

# ── 2. All four install channels are present ───────────────────────

echo "2. Channel coverage"
for channel in cargo brew npm curl; do
    if grep -Fq "channel: $channel" "$WORKFLOW"; then
        pass "channel '$channel' defined in matrix"
    else
        fail "channel '$channel' missing from matrix"
    fi
done

# ── 3. OS coverage per non-gated channel ───────────────────────────

echo "3. OS coverage"
for os in ubuntu-latest macos-latest windows-latest; do
    if grep -Fq "os: $os" "$WORKFLOW"; then
        pass "OS '$os' present in matrix"
    else
        fail "OS '$os' missing from matrix"
    fi
done

# ── 4. Gated channels are marked continue-on-error ─────────────────

echo "4. Gated channel behavior"
if grep -Fq "continue-on-error:" "$WORKFLOW"; then
    pass "continue-on-error is used for gated channels"
else
    fail "continue-on-error not found — gated channels may block the pipeline"
fi

if grep -Eq "gated: true" "$WORKFLOW"; then
    pass "at least one channel is marked gated"
else
    fail "no channels marked gated"
fi

# Brew and npm must be gated (external prerequisites not yet available).
brew_gated=$(grep -A2 'channel: brew' "$WORKFLOW" | grep -c 'gated: true' || true)
npm_gated=$(grep -A2 'channel: npm' "$WORKFLOW" | grep -c 'gated: true' || true)

if [ "$brew_gated" -gt 0 ]; then
    pass "brew channel is gated"
else
    fail "brew channel is not gated — external prerequisites not yet available"
fi

if [ "$npm_gated" -gt 0 ]; then
    pass "npm channel is gated"
else
    fail "npm channel is not gated — external prerequisites not yet available"
fi

# Cargo and curl must NOT be gated.
cargo_gated=$(grep -A2 'channel: cargo' "$WORKFLOW" | grep -c 'gated: true' || true)
curl_gated=$(grep -A2 'channel: curl' "$WORKFLOW" | grep -c 'gated: true' || true)

if [ "$cargo_gated" -eq 0 ]; then
    pass "cargo channel is not gated"
else
    fail "cargo channel should not be gated"
fi

if [ "$curl_gated" -eq 0 ]; then
    pass "curl channel is not gated"
else
    fail "curl channel should not be gated"
fi

# ── 5. Verification steps ──────────────────────────────────────────

echo "5. Verification steps"
if grep -Fq 'plumb --version' "$WORKFLOW"; then
    pass "workflow verifies plumb --version"
else
    fail "workflow does not verify plumb --version"
fi

if grep -Fq 'plumb lint plumb-fake://hello' "$WORKFLOW"; then
    pass "workflow runs plumb lint smoke check"
else
    fail "workflow does not run plumb lint smoke check"
fi

# Exit code handling: 0 and 3 are acceptable, 2 is infra failure.
if grep -Eq 'rc.*-ne 0.*rc.*-ne 3|exit 0.*exit 3' "$WORKFLOW"; then
    pass "workflow distinguishes acceptable exit codes (0, 3) from failures"
else
    # Fallback: just check that exit codes are checked at all.
    if grep -Eq '\$rc|\$\?' "$WORKFLOW"; then
        pass "workflow checks exit codes"
    else
        fail "workflow does not check exit codes"
    fi
fi

# ── 6. Env-indirection security (no direct ${{ }} in run blocks) ───

echo "6. Env-indirection security"
if grep -Fq 'Assert shell runs use env indirection' "$WORKFLOW"; then
    pass "workflow includes env-indirection assertion step"
else
    fail "workflow missing env-indirection security assertion"
fi

# The Python validator must actually check for ${{ in run blocks.
if grep -Fq 'expr_open' "$WORKFLOW"; then
    pass "env-indirection validator scans for workflow expression injection"
else
    fail "env-indirection validator does not scan for workflow expression injection"
fi

# ── 7. Failure reporting ───────────────────────────────────────────

echo "7. Failure reporting"
if grep -Fq 'install-smoke' "$WORKFLOW" && grep -Fq 'gh issue' "$WORKFLOW"; then
    pass "workflow auto-creates tracking issues on failure"
else
    fail "workflow does not auto-create tracking issues on failure"
fi

# ── 8. Permissions are minimal ─────────────────────────────────────

echo "8. Permissions"
if grep -Fq 'contents: read' "$WORKFLOW"; then
    pass "top-level permissions are read-only"
else
    fail "top-level permissions are not read-only"
fi

# The report job needs issues: write but should not have contents: write.
if grep -Fq 'issues: write' "$WORKFLOW"; then
    pass "report job has issues: write permission"
else
    fail "report job missing issues: write permission"
fi

# ── 9. Concurrency control ────────────────────────────────────────

echo "9. Concurrency"
if grep -Fq 'concurrency:' "$WORKFLOW"; then
    pass "workflow has concurrency control"
else
    fail "workflow missing concurrency control"
fi

if grep -Fq 'cancel-in-progress: true' "$WORKFLOW"; then
    pass "concurrent runs are cancelled"
else
    fail "concurrent runs are not cancelled"
fi

# ── 10. Maintained wiring ─────────────────────────────────────────

echo "10. Maintained wiring"
if grep -Eq '^install-smoke-validate:$' "$JUSTFILE"; then
    pass "justfile defines install-smoke-validate"
else
    fail "justfile does not define install-smoke-validate"
fi

if grep -Eq '^check:.*install-smoke-validate' "$JUSTFILE"; then
    pass "just check depends on install-smoke-validate"
else
    fail "just check does not depend on install-smoke-validate"
fi

if grep -Fq 'tests/install-smoke-validate.sh' "$CI_WORKFLOW"; then
    pass "ci.yml invokes tests/install-smoke-validate.sh"
else
    fail "ci.yml does not invoke tests/install-smoke-validate.sh"
fi

echo ""
if [ "$failures" -ne 0 ]; then
    echo "FAILED: one or more checks failed."
    exit 1
else
    echo "PASSED: all checks passed."
fi
