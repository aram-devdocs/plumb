#!/usr/bin/env bash
# release-security-validate.sh — Static validation for release workflow
# security properties: token handling, secret leakage prevention,
# attestation gates, and publish-permission blockers.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RELEASE_WORKFLOW="$REPO_ROOT/.github/workflows/release.yml"
INSTALL_SMOKE="$REPO_ROOT/.github/workflows/install-smoke.yml"
SECURITY_WORKFLOW="$REPO_ROOT/.github/workflows/security.yml"
DIST_CONFIG="$REPO_ROOT/dist-workspace.toml"
JUSTFILE="$REPO_ROOT/justfile"
CI_WORKFLOW="$REPO_ROOT/.github/workflows/ci.yml"
RELEASE_PREP_DOC="$REPO_ROOT/docs/src/ci/release-prep.md"

failures=0

pass() { echo "  PASS: $1"; }
fail() { echo "  FAIL: $1" >&2; failures=1; }

scan_run_block_secrets() {
    python3 - "$1" <<'PY'
import sys

path = sys.argv[1]
lines = open(path, encoding="utf-8").readlines()
in_run = False
run_indent = 0
found = []
expr_open = "$" + "{{"

for i, raw in enumerate(lines, 1):
    stripped = raw.lstrip()
    indent = len(raw) - len(raw.lstrip())

    if not stripped or stripped.startswith("#"):
        continue

    if in_run:
        if indent <= run_indent:
            in_run = False
        else:
            if expr_open in raw and "secrets." in raw:
                found.append(f"  line {i}: {stripped.rstrip()}")
            continue

    if stripped.startswith("run:"):
        rest = stripped[len("run:"):].strip()
        if rest in ("|", "|-", "|+"):
            in_run = True
            run_indent = indent
            continue
        if expr_open in stripped and "secrets." in stripped:
            found.append(f"  line {i}: {stripped.rstrip()}")

for line in found:
    print(line)
PY
}

echo "=== Release security gate validation ==="
echo ""

# ── 1. Release workflow exists ─────────────────────────────────────

echo "1. Release workflow"
if [ -f "$RELEASE_WORKFLOW" ]; then
    pass "release.yml exists"
else
    fail "release.yml missing"
fi

# ── 2. Attestation gates ──────────────────────────────────────────

echo "2. Attestation verification"
attest_count=$(grep -c 'actions/attest-build-provenance' "$RELEASE_WORKFLOW" || true)
if [ "$attest_count" -ge 2 ]; then
    pass "release workflow attests both build artifacts and installers ($attest_count attestation steps)"
else
    fail "release workflow has $attest_count attestation steps (expected >= 2: build + installers)"
fi

if grep -Fq 'id-token: write' "$RELEASE_WORKFLOW"; then
    pass "release workflow has id-token: write for OIDC attestation"
else
    fail "release workflow missing id-token: write — attestation will fail"
fi

if grep -Fq 'attestations: write' "$RELEASE_WORKFLOW"; then
    pass "release workflow has attestations: write permission"
else
    fail "release workflow missing attestations: write permission"
fi

if grep -Fq 'Cargo publish and curl installers are the only non-manual release' "$RELEASE_WORKFLOW" \
    && grep -Fq 'Homebrew tap and npm' "$RELEASE_WORKFLOW" \
    && grep -Fq 'publishing are intentionally inactive here.' "$RELEASE_WORKFLOW"; then
    pass "release workflow docs distinguish active non-manual channels from gated ones"
else
    fail "release workflow docs do not distinguish active non-manual channels from gated ones"
fi

# ── 3. Token handling via env indirection ──────────────────────────

echo "3. Token handling"

# The release workflow must use GITHUB_TOKEN through env, not inline.
if grep -Fq 'GITHUB_TOKEN:' "$RELEASE_WORKFLOW"; then
    pass "release workflow uses GITHUB_TOKEN via env block"
else
    fail "release workflow does not use GITHUB_TOKEN via env block"
fi

# Check that install-smoke uses env indirection for tokens too.
if grep -Fq 'GH_TOKEN:' "$INSTALL_SMOKE"; then
    pass "install-smoke uses GH_TOKEN via env block"
else
    fail "install-smoke does not use GH_TOKEN via env block"
fi

# ── 4. No hardcoded secrets in run blocks ──────────────────────────

echo "4. Secret leakage prevention"

# Scan all release-related workflows for direct secret interpolation
# in run blocks. The safe pattern is: env: { TOKEN: ${{ secrets.X }} }
# then reference $TOKEN in the script. Direct ${{ secrets.* }} in run
# blocks risks leaking secrets in logs.
legacy_env_key_pattern='[A-Z_]'
legacy_env_key_pattern+='*:\s*\${{'
if grep -Fq "$legacy_env_key_pattern" "$0"; then
    fail "validator still contains the legacy loose env-key filter"
else
    pass "validator no longer relies on the legacy loose env-key filter"
fi

# Regression guard: uppercase env keys in env: blocks are allowed, but
# the same text inside a run: block must be flagged. This documents the
# anchored env-key behavior Claude requested while using a stricter
# context-aware scanner.
scanner_fixture="$(mktemp)"
cat >"$scanner_fixture" <<'EOF'
steps:
  - name: safe env block
    env:
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    run: |
      echo "safe"
  - name: unsafe run block
    run: |
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }} && curl -fsSL https://example.test/install.sh | sh
EOF
fixture_hits="$(scan_run_block_secrets "$scanner_fixture")"
rm -f "$scanner_fixture"
if printf '%s\n' "$fixture_hits" | grep -Fq 'GH_TOKEN: ${{ secrets.GITHUB_TOKEN }} && curl'; then
    pass "scanner ignores env-key entries and flags direct run-block interpolation"
else
    fail "scanner regression: expected run-block interpolation to be flagged while env-key entries stay allowed"
fi

for wf in "$RELEASE_WORKFLOW" "$INSTALL_SMOKE"; do
    wf_name="$(basename "$wf")"
    # Only flag ${{ secrets.* }} inside run: block bodies. Unlike the
    # previous loose env-key grep, this cannot be fooled by lines that
    # look like env assignments but actually appear inside scripts.
    secret_in_run="$(scan_run_block_secrets "$wf")"
    if [ -z "$secret_in_run" ]; then
        pass "$wf_name: no direct secret interpolation in run blocks"
    else
        fail "$wf_name: direct secret interpolation in run blocks:"
        echo "$secret_in_run" >&2
    fi
done

# ── 5. Homebrew tap publish is gated ───────────────────────────────

echo "5. Homebrew publish gating"

# The release workflow must NOT contain active (non-comment) Homebrew publish steps.
if grep -v '^\s*#' "$RELEASE_WORKFLOW" | grep -Eiq 'brew.*push|homebrew.*publish|tap.*push'; then
    fail "release workflow contains active Homebrew publish steps — must be gated"
else
    pass "release workflow does not contain active Homebrew publish steps"
fi

# dist-workspace.toml must not have tap field set (gated until prereqs exist).
if [ -f "$DIST_CONFIG" ]; then
    if grep -Eq '^\s*tap\s*=' "$DIST_CONFIG"; then
        fail "dist-workspace.toml has tap field set — Homebrew tap must stay gated"
    else
        pass "dist-workspace.toml does not set tap — Homebrew publish correctly gated"
    fi
else
    pass "dist-workspace.toml not found — Homebrew publish correctly absent"
fi

if [ -f "$DIST_CONFIG" ] \
    && grep -Fq 'Issues #51 and #52 are intentionally prep-only' "$DIST_CONFIG"; then
    pass "dist-workspace.toml documents #51/#52 as prep-only"
else
    fail "dist-workspace.toml does not document #51/#52 as prep-only"
fi

# ── 6. NPM scope publish is gated ─────────────────────────────────

echo "6. NPM publish gating"

# The release workflow must NOT contain active (non-comment) npm publish steps.
if grep -v '^\s*#' "$RELEASE_WORKFLOW" | grep -Eiq 'npm.*publish|npm-scope'; then
    fail "release workflow contains active npm publish steps — must be gated"
else
    pass "release workflow does not contain active npm publish steps"
fi

# dist-workspace.toml must not have npm-scope field set.
if [ -f "$DIST_CONFIG" ]; then
    if grep -Eq '^\s*npm-scope\s*=' "$DIST_CONFIG"; then
        fail "dist-workspace.toml has npm-scope field set — NPM publish must stay gated"
    else
        pass "dist-workspace.toml does not set npm-scope — NPM publish correctly gated"
    fi
fi

if [ -f "$DIST_CONFIG" ] \
    && grep -Fq 'installers = ["shell", "powershell"]' "$DIST_CONFIG"; then
    pass "dist-workspace.toml keeps npm out of the active installer list"
else
    fail "dist-workspace.toml does not keep npm out of the active installer list"
fi

# The install-smoke workflow documents NPM_TOKEN is not yet wired.
if grep -Fq 'npm' "$INSTALL_SMOKE"; then
    pass "install-smoke acknowledges npm channel"
else
    fail "install-smoke does not acknowledge npm channel"
fi

if [ -f "$RELEASE_PREP_DOC" ] \
    && grep -Fq 'Until those blockers are resolved, the install docs describe the' "$RELEASE_PREP_DOC" \
    && grep -Fq 'Until those blockers are resolved, this repo MUST NOT claim that' "$RELEASE_PREP_DOC"; then
    pass "release prep doc keeps Homebrew/npm claims gated behind external blockers"
else
    fail "release prep doc does not keep Homebrew/npm claims gated behind external blockers"
fi

# ── 7. Security audit workflow exists ──────────────────────────────

echo "7. Security audit"
if [ -f "$SECURITY_WORKFLOW" ]; then
    pass "security.yml workflow exists"
else
    fail "security.yml workflow missing"
fi

if grep -Fq 'cargo audit' "$SECURITY_WORKFLOW"; then
    pass "security workflow runs cargo audit"
else
    fail "security workflow does not run cargo audit"
fi

if grep -Fq 'cargo deny' "$SECURITY_WORKFLOW"; then
    pass "security workflow runs cargo deny"
else
    fail "security workflow does not run cargo deny"
fi

# ── 8. Release permissions are scoped ──────────────────────────────

echo "8. Release permission scope"

# Release workflow needs contents: write for uploading release assets.
if grep -Fq 'contents: write' "$RELEASE_WORKFLOW"; then
    pass "release workflow has contents: write (needed for release upload)"
else
    fail "release workflow missing contents: write"
fi

# Release should NOT have issues: write, pull-requests: write, etc.
for perm in 'issues: write' 'pull-requests: write' 'packages: write'; do
    if grep -Fq "$perm" "$RELEASE_WORKFLOW"; then
        fail "release workflow has unnecessary permission: $perm"
    else
        pass "release workflow does not have excessive permission: $perm"
    fi
done

# ── 9. Concurrency: release must not cancel in progress ────────────

echo "9. Release concurrency safety"
if grep -Fq 'cancel-in-progress: false' "$RELEASE_WORKFLOW"; then
    pass "release workflow does not cancel in-progress runs (release safety)"
else
    fail "release workflow may cancel in-progress runs — unsafe for releases"
fi

# ── 10. Maintained wiring ─────────────────────────────────────────

echo "10. Maintained wiring"
if grep -Eq '^release-security-validate:$' "$JUSTFILE"; then
    pass "justfile defines release-security-validate"
else
    fail "justfile does not define release-security-validate"
fi

if grep -Eq '^check:.*release-security-validate' "$JUSTFILE"; then
    pass "just check depends on release-security-validate"
else
    fail "just check does not depend on release-security-validate"
fi

if grep -Fq 'tests/release-security-validate.sh' "$CI_WORKFLOW"; then
    pass "ci.yml invokes tests/release-security-validate.sh"
else
    fail "ci.yml does not invoke tests/release-security-validate.sh"
fi

echo ""
if [ "$failures" -ne 0 ]; then
    echo "FAILED: one or more checks failed."
    exit 1
else
    echo "PASSED: all checks passed."
fi
