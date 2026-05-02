#!/usr/bin/env bash
# release-readiness-matrix.sh — Run the release-readiness matrix against
# live and local legs, classifying each result as infra or violation.
#
# Exit codes from `plumb lint` (PRD §13.3):
#   0 — no violations
#   1 — error-severity violations present
#   2 — CLI / infrastructure failure (bad URL, driver error, etc.)
#   3 — warning-severity violations only
#
# This script treats 0/1/3 as "lint succeeded" (violation classification)
# and 2 as "infra failure". Any other exit code is an unexpected failure
# classified as infra.
#
# Usage:
#   bash tests/release-readiness-matrix.sh [--local-only]
#
# Options:
#   --local-only   Skip live legs (useful in CI without network access
#                  or when Chrome is unavailable for HTTPS targets).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MANIFEST="$REPO_ROOT/tests/fixtures/release-readiness/manifest.json"
PLUMB="${PLUMB_BIN:-cargo run --quiet -p plumb-cli --}"
REPORT_DIR="${REPORT_DIR:-/tmp/plumb-readiness-matrix}"

LOCAL_ONLY=false
for arg in "$@"; do
    case "$arg" in
        --local-only) LOCAL_ONLY=true ;;
        *) echo "Unknown argument: $arg" >&2; exit 2 ;;
    esac
done

mkdir -p "$REPORT_DIR"

# Counters
total=0
passed=0
violation=0
infra=0
leg_results=()

# ── helpers ─────────────────────────────────────────────────────────

classify_exit() {
    local code="$1"
    case "$code" in
        0) echo "pass"       ;;  # no violations
        1) echo "violation"  ;;  # errors present
        3) echo "violation"  ;;  # warnings only
        2) echo "infra"      ;;  # CLI / infrastructure failure
        *) echo "infra"      ;;  # unexpected
    esac
}

severity_label() {
    local code="$1"
    case "$code" in
        0) echo "clean"          ;;
        1) echo "error"          ;;
        3) echo "warning"        ;;
        2) echo "infra-failure"  ;;
        *) echo "unknown-$code"  ;;
    esac
}

run_leg() {
    local name="$1"
    local url="$2"
    local kind="$3"  # "live" or "local"

    total=$((total + 1))
    local output_file="$REPORT_DIR/${name}.json"
    local exit_code=0

    $PLUMB lint "$url" --format json > "$output_file" 2>"$REPORT_DIR/${name}.stderr" || exit_code=$?

    local class
    class="$(classify_exit "$exit_code")"
    local severity
    severity="$(severity_label "$exit_code")"

    case "$class" in
        pass)      passed=$((passed + 1)) ;;
        violation) violation=$((violation + 1)) ;;
        infra)     infra=$((infra + 1)) ;;
    esac

    local status_mark
    case "$class" in
        pass)      status_mark="PASS" ;;
        violation) status_mark="LINT" ;;
        infra)     status_mark="INFRA" ;;
    esac

    leg_results+=("  ${status_mark}: ${name} (${kind}, exit=${exit_code}, class=${severity})")
    echo "  ${status_mark}: ${name} [${kind}] exit=${exit_code} class=${severity}"
}

# ── live legs ───────────────────────────────────────────────────────

echo "=== Release-readiness matrix ==="
echo ""

if [ "$LOCAL_ONLY" = false ]; then
    echo "── Live legs ──"
    run_leg "live-canonical-docs" "https://plumb.aramhammoudeh.com/" "live"
    run_leg "live-example-com"    "https://example.com"              "live"
    echo ""
else
    echo "── Live legs (skipped: --local-only) ──"
    echo ""
fi

# ── local legs ──────────────────────────────────────────────────────

echo "── Local legs ──"

# Minimal kit
run_leg "local-minimal" \
    "file://${REPO_ROOT}/tests/fixtures/release-readiness/minimal.html" "local"

# Responsive kit
run_leg "local-responsive" \
    "file://${REPO_ROOT}/tests/fixtures/release-readiness/responsive.html" "local"

# Typography kit
run_leg "local-typography" \
    "file://${REPO_ROOT}/tests/fixtures/release-readiness/typography.html" "local"

# Contrast kit
run_leg "local-contrast" \
    "file://${REPO_ROOT}/tests/fixtures/release-readiness/contrast.html" "local"

# Shadow/z/opacity/padding kit
run_leg "local-shadow-z-opacity-padding" \
    "file://${REPO_ROOT}/tests/fixtures/release-readiness/shadow-z-opacity-padding.html" "local"

# Dynamic-wait kit
run_leg "local-dynamic-wait" \
    "file://${REPO_ROOT}/tests/fixtures/release-readiness/dynamic-wait.html" "local"

# Auth-storage kit
run_leg "local-auth-storage" \
    "file://${REPO_ROOT}/tests/fixtures/release-readiness/auth-storage.html" "local"

# Static docs kit
run_leg "local-static-docs" \
    "file://${REPO_ROOT}/tests/fixtures/release-readiness/static-docs.html" "local"

# App-like kit
run_leg "local-app-like" \
    "file://${REPO_ROOT}/tests/fixtures/release-readiness/app-like.html" "local"

# Malformed / edge-case DOM kit
run_leg "local-malformed-edge" \
    "file://${REPO_ROOT}/tests/fixtures/release-readiness/malformed-edge.html" "local"

# Large-DOM kits (reuse benchmark fixtures)
run_leg "local-large-dom-100" \
    "file://${REPO_ROOT}/crates/plumb-cdp/benches/fixtures/fixed-dom-100-nodes.html" "local"

run_leg "local-large-dom-1k" \
    "file://${REPO_ROOT}/crates/plumb-cdp/benches/fixtures/fixed-dom-1k-nodes.html" "local"

run_leg "local-large-dom-10k" \
    "file://${REPO_ROOT}/crates/plumb-cdp/benches/fixtures/fixed-dom-10k-nodes.html" "local"

echo ""

# ── summary ─────────────────────────────────────────────────────────

echo "── Summary ──"
echo "  Total legs: $total"
echo "  Passed (clean):     $passed"
echo "  Violations (lint):  $violation"
echo "  Infra failures:     $infra"
echo ""

# Write machine-readable summary
cat > "$REPORT_DIR/summary.json" <<ENDJSON
{
  "total": $total,
  "passed": $passed,
  "violation": $violation,
  "infra": $infra,
  "local_only": $LOCAL_ONLY,
  "legs": [
$(printf '    "%s",\n' "${leg_results[@]}" | sed '$ s/,$//')
  ]
}
ENDJSON

echo "Per-leg reports written to: $REPORT_DIR/"
echo ""

# ── exit classification ─────────────────────────────────────────────
# The matrix exits nonzero when any leg has an infra failure.
# Violation-only results (exit 1/3) are expected and do not fail the job.
# Per-leg reports are always written before this exit so artifacts are
# available even on failure.

if [ "$infra" -gt 0 ]; then
    echo "FAIL: $infra leg(s) had infra failures. Review per-leg stderr"
    echo "      in $REPORT_DIR/ for root-cause details."
    exit 1
fi

echo "Matrix complete."
