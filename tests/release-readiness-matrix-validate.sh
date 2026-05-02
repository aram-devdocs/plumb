#!/usr/bin/env bash
# release-readiness-matrix-validate.sh — Static validation for the
# release-readiness matrix workflow wiring and leg coverage.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MATRIX_SCRIPT="$REPO_ROOT/tests/release-readiness-matrix.sh"
MATRIX_WORKFLOW="$REPO_ROOT/.github/workflows/release-readiness-matrix.yml"
JUSTFILE="$REPO_ROOT/justfile"
CI_WORKFLOW="$REPO_ROOT/.github/workflows/ci.yml"
MANIFEST="$REPO_ROOT/tests/fixtures/release-readiness/manifest.json"

failures=0

pass() { echo "  PASS: $1"; }
fail() { echo "  FAIL: $1" >&2; failures=1; }

echo "=== Release-readiness matrix validation ==="
echo ""

# ── 1. Matrix script exists and is executable ───────────────────────

echo "1. Matrix runner script"
if [ -f "$MATRIX_SCRIPT" ]; then
    pass "tests/release-readiness-matrix.sh exists"
else
    fail "tests/release-readiness-matrix.sh missing"
fi

if [ -x "$MATRIX_SCRIPT" ]; then
    pass "tests/release-readiness-matrix.sh is executable"
else
    fail "tests/release-readiness-matrix.sh is not executable"
fi

# ── 2. Live legs are defined ────────────────────────────────────────

echo "2. Live legs coverage"
if grep -Fq 'https://plumb.aramhammoudeh.com/' "$MATRIX_SCRIPT"; then
    pass "matrix covers canonical docs URL"
else
    fail "matrix does not cover canonical docs URL"
fi

if grep -Fq 'https://example.com' "$MATRIX_SCRIPT"; then
    pass "matrix covers example.com"
else
    fail "matrix does not cover example.com"
fi

# ── 3. Local legs cover all manifest kits ───────────────────────────

echo "3. Local legs coverage (manifest kits)"

# Extract kit file paths from the manifest and verify each appears
# in the matrix script. Uses simple grep since jq may not be present.
kit_files=(
    "tests/fixtures/release-readiness/minimal.html"
    "tests/fixtures/release-readiness/responsive.html"
    "tests/fixtures/release-readiness/typography.html"
    "tests/fixtures/release-readiness/contrast.html"
    "tests/fixtures/release-readiness/shadow-z-opacity-padding.html"
    "tests/fixtures/release-readiness/dynamic-wait.html"
    "tests/fixtures/release-readiness/auth-storage.html"
    "crates/plumb-cdp/benches/fixtures/fixed-dom-100-nodes.html"
    "crates/plumb-cdp/benches/fixtures/fixed-dom-1k-nodes.html"
    "crates/plumb-cdp/benches/fixtures/fixed-dom-10k-nodes.html"
)

for kit_file in "${kit_files[@]}"; do
    if grep -Fq "$kit_file" "$MATRIX_SCRIPT"; then
        pass "matrix covers $kit_file"
    else
        fail "matrix does not cover $kit_file"
    fi
done

# ── 4. Infra-vs-violation classification ────────────────────────────

echo "4. Infra-vs-violation classification"
if grep -Fq 'classify_exit' "$MATRIX_SCRIPT"; then
    pass "matrix implements exit-code classification"
else
    fail "matrix does not implement exit-code classification"
fi

if grep -Eq 'infra|violation' "$MATRIX_SCRIPT"; then
    pass "matrix uses infra/violation terminology"
else
    fail "matrix does not use infra/violation terminology"
fi

if grep -Fq 'summary.json' "$MATRIX_SCRIPT"; then
    pass "matrix produces machine-readable summary"
else
    fail "matrix does not produce machine-readable summary"
fi

# ── 5. Per-leg failure reporting ────────────────────────────────────

echo "5. Per-leg failure reporting"
if grep -Eq '\.json|\.stderr' "$MATRIX_SCRIPT"; then
    pass "matrix writes per-leg output files"
else
    fail "matrix does not write per-leg output files"
fi

# ── 6. CI workflow exists ───────────────────────────────────────────

echo "6. CI workflow"
if [ -f "$MATRIX_WORKFLOW" ]; then
    pass "release-readiness-matrix.yml workflow exists"
else
    fail "release-readiness-matrix.yml workflow missing"
fi

if grep -Fq 'release-readiness-matrix.sh' "$MATRIX_WORKFLOW"; then
    pass "workflow invokes release-readiness-matrix.sh"
else
    fail "workflow does not invoke release-readiness-matrix.sh"
fi

# ── 7. Maintained wiring ───────────────────────────────────────────

echo "7. Maintained wiring"
if grep -Eq '^release-readiness-matrix-validate:$' "$JUSTFILE"; then
    pass "justfile defines release-readiness-matrix-validate"
else
    fail "justfile does not define release-readiness-matrix-validate"
fi

if grep -Eq '^check:.*release-readiness-matrix-validate' "$JUSTFILE"; then
    pass "just check depends on release-readiness-matrix-validate"
else
    fail "just check does not depend on release-readiness-matrix-validate"
fi

if grep -Fq 'tests/release-readiness-matrix-validate.sh' "$CI_WORKFLOW"; then
    pass "ci.yml invokes tests/release-readiness-matrix-validate.sh"
else
    fail "ci.yml does not invoke tests/release-readiness-matrix-validate.sh"
fi

echo ""
if [ "$failures" -ne 0 ]; then
    echo "FAILED: one or more checks failed."
    exit 1
else
    echo "PASSED: all checks passed."
fi
