#!/usr/bin/env bash
# ci-chrome-sandbox-validate.sh — Static validation for the Chrome sandbox
# prep script and its workflow integration.
#
# Checks:
#   1. Script exists and is executable.
#   2. Script has a Linux guard (exits on non-Linux).
#   3. Script is idempotent (checks before writing).
#   4. Script fails loud (set -euo pipefail + exit 1 on verify failure).
#   5. The validator is wired into maintained local + CI gates.
#   6. Both Chrome workflows invoke the prep script before Chrome steps.
#   7. No --no-sandbox flag anywhere in the repo's workflow files.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPT="$REPO_ROOT/scripts/ci-chrome-sandbox.sh"
JUSTFILE="$REPO_ROOT/justfile"
CI_WORKFLOW="$REPO_ROOT/.github/workflows/ci.yml"
BENCHMARKS="$REPO_ROOT/.github/workflows/benchmarks.yml"
DOGFOOD="$REPO_ROOT/.github/workflows/dogfood.yml"

fail=0

pass() { echo "  PASS: $1"; }
fail() { echo "  FAIL: $1" >&2; fail=1; }

echo "=== Chrome sandbox prep validation ==="
echo ""

# ── 1. Script exists and is executable ───────────────────────────────
echo "1. Script exists and is executable"
if [ -f "$SCRIPT" ]; then
    pass "scripts/ci-chrome-sandbox.sh exists"
else
    fail "scripts/ci-chrome-sandbox.sh not found"
fi

if [ -x "$SCRIPT" ]; then
    pass "script is executable"
else
    fail "script is not executable"
fi

# ── 2. Linux guard ──────────────────────────────────────────────────
echo "2. Linux guard"
if grep -q 'uname -s.*Linux\|uname.*!=.*Linux' "$SCRIPT" 2>/dev/null; then
    pass "Linux guard present"
else
    fail "no Linux guard found"
fi

# ── 3. Idempotency ─────────────────────────────────────────────────
echo "3. Idempotency (checks before writing)"
if grep -q 'already enabled\|already disabled' "$SCRIPT" 2>/dev/null; then
    pass "idempotent check-before-write pattern found"
else
    fail "no idempotency pattern detected"
fi

echo "3a. Sysctl/procfs path split"
if grep -q '^USERNS_SYSCTL_KEY="kernel\.unprivileged_userns_clone"$' "$SCRIPT" 2>/dev/null; then
    pass "sysctl key uses dotted kernel name"
else
    fail "sysctl key is missing or does not use kernel.unprivileged_userns_clone"
fi

if grep -q '^USERNS_SYSCTL_PROCFS="/proc/sys/kernel/unprivileged_userns_clone"$' "$SCRIPT" 2>/dev/null; then
    pass "procfs path resolves to /proc/sys/kernel/unprivileged_userns_clone"
else
    fail "procfs path is missing or incorrect"
fi

if grep -Eq '/proc/sys/kernel/kernel\.unprivileged_userns_clone|/proc/sys/kernel/\$USERNS_SYSCTL([[:space:]]|$|")' "$SCRIPT" 2>/dev/null; then
    fail "double-prefixed procfs path construction detected"
else
    pass "no double-prefixed procfs path construction"
fi

# ── 4. Fail-loud ────────────────────────────────────────────────────
echo "4. Fail-loud"
if grep -q 'set -euo pipefail' "$SCRIPT" 2>/dev/null; then
    pass "set -euo pipefail present"
else
    fail "set -euo pipefail missing"
fi

if grep -q 'exit 1' "$SCRIPT" 2>/dev/null; then
    pass "explicit exit 1 on failure"
else
    fail "no exit 1 found for failure paths"
fi

if grep -Eq 'if \[ -f "\$USERNS_SYSCTL_PROCFS" \]; then' "$SCRIPT" 2>/dev/null && \
   grep -Eq 'val=\$\(cat "\$USERNS_SYSCTL_PROCFS"\)' "$SCRIPT" 2>/dev/null; then
    pass "verification reads the required procfs path explicitly"
else
    fail "verification does not read USERNS_SYSCTL_PROCFS explicitly"
fi

if grep -Eq 'if \[ -f "\$USERNS_SYSCTL" \]; then|if \[ -f "/proc/sys/kernel/\$USERNS_SYSCTL" \]; then' "$SCRIPT" 2>/dev/null; then
    fail "verification can silently skip by probing the wrong sysctl path"
else
    pass "verification does not probe the wrong sysctl path"
fi

# ── 5. Maintained gate integration ──────────────────────────────────
echo "5. Maintained gate integration"

if grep -Eq '^ci-chrome-sandbox-validate:' "$JUSTFILE" 2>/dev/null; then
    pass "justfile defines ci-chrome-sandbox-validate"
else
    fail "justfile does not define ci-chrome-sandbox-validate"
fi

if grep -Eq '^check:.*ci-chrome-sandbox-validate' "$JUSTFILE" 2>/dev/null; then
    pass "just check depends on ci-chrome-sandbox-validate"
else
    fail "just check does not depend on ci-chrome-sandbox-validate"
fi

if grep -q 'tests/ci-chrome-sandbox-validate.sh' "$CI_WORKFLOW" 2>/dev/null; then
    pass "ci.yml invokes tests/ci-chrome-sandbox-validate.sh"
else
    fail "ci.yml does not invoke tests/ci-chrome-sandbox-validate.sh"
fi

# ── 6. Workflow integration ─────────────────────────────────────────
echo "6. Workflow integration"

check_workflow_order() {
    local wf_file="$1"
    local wf_name="$2"

    if ! grep -q 'ci-chrome-sandbox.sh' "$wf_file" 2>/dev/null; then
        fail "$wf_name does not invoke ci-chrome-sandbox.sh"
        return
    fi
    pass "$wf_name invokes ci-chrome-sandbox.sh"

    # Verify sandbox step comes before Chrome install step.
    local sandbox_line chrome_line
    sandbox_line=$(grep -n 'ci-chrome-sandbox.sh' "$wf_file" | head -1 | cut -d: -f1)
    chrome_line=$(grep -n 'setup-chrome' "$wf_file" | head -1 | cut -d: -f1)

    if [ -n "$sandbox_line" ] && [ -n "$chrome_line" ] && [ "$sandbox_line" -lt "$chrome_line" ]; then
        pass "$wf_name: sandbox prep comes before Chrome install"
    else
        fail "$wf_name: sandbox prep must come before Chrome install"
    fi
}

check_workflow_order "$BENCHMARKS" "benchmarks.yml"
check_workflow_order "$DOGFOOD" "dogfood.yml"

# ── 7. No --no-sandbox ─────────────────────────────────────────────
echo "7. No --no-sandbox"
if grep -r -- '--no-sandbox' "$REPO_ROOT/.github/workflows/" 2>/dev/null; then
    fail "--no-sandbox found in workflow files"
else
    pass "no --no-sandbox in any workflow"
fi

echo ""
if [ "$fail" -ne 0 ]; then
    echo "FAILED: one or more checks failed."
    exit 1
else
    echo "PASSED: all checks passed."
fi
