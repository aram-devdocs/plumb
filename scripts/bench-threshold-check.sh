#!/usr/bin/env bash
# bench-threshold-check.sh — Parse Criterion JSON estimates and enforce p50
# regression thresholds. Exits non-zero if any threshold is breached.
#
# Thresholds (nanoseconds):
#   cold_start  p50 > 2 000 000 000  (2 s)
#   warm_run    p50 >   500 000 000  (500 ms)
#
# Usage:
#   ./scripts/bench-threshold-check.sh [criterion_base_dir]
#
# criterion_base_dir defaults to target/criterion.

set -euo pipefail

CRITERION_DIR="${1:-target/criterion}"

# Thresholds in nanoseconds.
COLD_START_LIMIT_NS=2000000000
WARM_RUN_LIMIT_NS=500000000

fail=0

# extract_median <estimates.json> → prints integer nanoseconds
extract_median() {
    python3 -c "
import json, sys
with open(sys.argv[1]) as f:
    data = json.load(f)
print(int(data['median']['point_estimate']))
" "$1"
}

check_threshold() {
    local name="$1"
    local limit_ns="$2"
    local estimates_file="$CRITERION_DIR/$name/new/estimates.json"

    if [ ! -f "$estimates_file" ]; then
        echo "::error::$name estimates not found at $estimates_file"
        fail=1
        return
    fi

    local median_int
    median_int=$(extract_median "$estimates_file")

    local limit_ms=$((limit_ns / 1000000))
    local median_ms=$((median_int / 1000000))

    echo "$name: p50 = ${median_ms} ms (limit: ${limit_ms} ms)"

    if [ "$median_int" -gt "$limit_ns" ]; then
        echo "::error::$name p50 regression: ${median_ms} ms exceeds ${limit_ms} ms threshold"
        fail=1
    fi
}

echo "=== Benchmark p50 threshold check ==="
echo "Criterion dir: $CRITERION_DIR"
echo ""

# per_rule_dom has no threshold: it scales with DOM size and rule count,
# so a fixed limit would be either too tight for 10k-node runs or too
# loose for 100-node runs. Use Criterion's statistical change detection
# for per_rule_dom regressions instead.
check_threshold "cold_start" "$COLD_START_LIMIT_NS"
check_threshold "warm_run" "$WARM_RUN_LIMIT_NS"

echo ""
if [ "$fail" -ne 0 ]; then
    echo "FAILED: one or more benchmarks exceeded p50 thresholds."
    exit 1
else
    echo "PASSED: all benchmarks within thresholds."
fi
