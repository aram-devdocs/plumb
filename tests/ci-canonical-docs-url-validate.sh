#!/usr/bin/env bash
# ci-canonical-docs-url-validate.sh — Static validation for canonical docs URL
# usage in CI and release acceptance paths.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DOGFOOD="$REPO_ROOT/.github/workflows/dogfood.yml"
JUSTFILE="$REPO_ROOT/justfile"
CI_WORKFLOW="$REPO_ROOT/.github/workflows/ci.yml"

ACTIVE_PATHS=(
    "$REPO_ROOT/.github/workflows"
    "$REPO_ROOT/justfile"
    "$REPO_ROOT/scripts"
    "$REPO_ROOT/tests"
    "$REPO_ROOT/docs/runbooks"
    "$REPO_ROOT/docs/src/ci"
)

failures=0

pass() { echo "  PASS: $1"; }
fail() { echo "  FAIL: $1" >&2; failures=1; }

echo "=== Canonical docs URL validation ==="
echo ""

echo "1. Dogfood workflow targets the canonical docs URL"
if grep -Eq '^  lint-canonical-docs:$' "$DOGFOOD"; then
    pass "dogfood job id is lint-canonical-docs"
else
    fail "dogfood job id is not lint-canonical-docs"
fi

if grep -Fq 'name: plumb lint https://plumb.aramhammoudeh.com/' "$DOGFOOD"; then
    pass "dogfood job name uses canonical docs URL"
else
    fail "dogfood job name does not use canonical docs URL"
fi

if grep -Fq 'target/release/plumb lint https://plumb.aramhammoudeh.com/ \' "$DOGFOOD"; then
    pass "dogfood lint command uses canonical docs URL"
else
    fail "dogfood lint command does not use canonical docs URL"
fi

echo "2. Maintained validation wiring"
if grep -Eq '^ci-canonical-docs-url-validate:$' "$JUSTFILE"; then
    pass "justfile defines ci-canonical-docs-url-validate"
else
    fail "justfile does not define ci-canonical-docs-url-validate"
fi

if grep -Eq '^check:.*ci-canonical-docs-url-validate' "$JUSTFILE"; then
    pass "just check depends on ci-canonical-docs-url-validate"
else
    fail "just check does not depend on ci-canonical-docs-url-validate"
fi

if grep -Fq 'tests/ci-canonical-docs-url-validate.sh' "$CI_WORKFLOW"; then
    pass "ci.yml invokes tests/ci-canonical-docs-url-validate.sh"
else
    fail "ci.yml does not invoke tests/ci-canonical-docs-url-validate.sh"
fi

echo "3. No plumb.dev in active CI/release acceptance paths"
matches=()
for path in "${ACTIVE_PATHS[@]}"; do
    while IFS= read -r line; do
        if [[ "$line" == *"tests/ci-canonical-docs-url-validate.sh:"* ]]; then
            continue
        fi
        matches+=("$line")
    done < <(rg -n 'plumb\.dev' "$path" || true)
done

if [ "${#matches[@]}" -eq 0 ]; then
    pass "no active CI/release acceptance path references plumb.dev"
else
    fail "active CI/release acceptance paths still reference plumb.dev"
    printf '%s\n' "${matches[@]}" >&2
fi

echo ""
if [ "$failures" -ne 0 ]; then
    echo "FAILED: one or more checks failed."
    exit 1
else
    echo "PASSED: all checks passed."
fi
