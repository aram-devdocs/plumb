#!/usr/bin/env bash
# release-readiness-local-kits-validate.sh — Static validation for the
# offline release-readiness local kits and their maintained wiring.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
README="$REPO_ROOT/tests/fixtures/release-readiness/README.md"
MANIFEST="$REPO_ROOT/tests/fixtures/release-readiness/manifest.json"
JUSTFILE="$REPO_ROOT/justfile"
CI_WORKFLOW="$REPO_ROOT/.github/workflows/ci.yml"

failures=0

pass() { echo "  PASS: $1"; }
fail() { echo "  FAIL: $1" >&2; failures=1; }

echo "=== Release-readiness local kit validation ==="
echo ""

echo "1. Checked-in contract files exist"
if [ -f "$README" ]; then
    pass "README exists"
else
    fail "README missing at tests/fixtures/release-readiness/README.md"
fi

if [ -f "$MANIFEST" ]; then
    pass "manifest exists"
else
    fail "manifest missing at tests/fixtures/release-readiness/manifest.json"
fi

echo "2. Maintained local + CI wiring"
if grep -Eq '^release-readiness-local-kits-validate:$' "$JUSTFILE"; then
    pass "justfile defines release-readiness-local-kits-validate"
else
    fail "justfile does not define release-readiness-local-kits-validate"
fi

if grep -Eq '^check:.*release-readiness-local-kits-validate' "$JUSTFILE"; then
    pass "just check depends on release-readiness-local-kits-validate"
else
    fail "just check does not depend on release-readiness-local-kits-validate"
fi

if grep -Fq 'tests/release-readiness-local-kits-validate.sh' "$CI_WORKFLOW"; then
    pass "ci.yml invokes tests/release-readiness-local-kits-validate.sh"
else
    fail "ci.yml does not invoke tests/release-readiness-local-kits-validate.sh"
fi

echo "3. Direct validator passes"
if (cd "$REPO_ROOT" && cargo xtask validate-release-readiness-kits >/dev/null); then
    pass "cargo xtask validate-release-readiness-kits passes"
else
    fail "cargo xtask validate-release-readiness-kits failed"
fi

echo ""
if [ "$failures" -ne 0 ]; then
    echo "FAILED: one or more checks failed."
    exit 1
else
    echo "PASSED: all checks passed."
fi
