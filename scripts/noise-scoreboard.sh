#!/usr/bin/env bash
# noise-scoreboard.sh — dogfood Plumb's signal-to-noise on real pages.
#
# Lints the local `noise kitchen-sink` fixture (every known
# false-positive pattern plus true-positives that must survive) and,
# when `--live` is passed, a set of real public sites. Prints a
# per-rule violation breakdown so a precision change can be measured
# before/after.
#
# This is a developer instrument, not a CI gate: the per-rule golden
# tests in `crates/plumb-core/tests/golden_*.rs` are the deterministic
# regression guards. The kitchen-sink deliberately uses some
# user-agent-default values (e.g. an unstyled <h1> margin) whose exact
# pixels vary by Chromium version, so its counts are indicative, not
# pinned.
#
# Usage:
#   scripts/noise-scoreboard.sh              # local fixture only
#   scripts/noise-scoreboard.sh --live       # + shadcn / HN / example.com
#
# Env:
#   PLUMB_BIN     path to the plumb binary  (default: ./target/debug/plumb)
#   PLUMB_CHROME  path to Chrome/Chromium   (default: macOS Google Chrome)

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${PLUMB_BIN:-$ROOT/target/debug/plumb}"
CHROME="${PLUMB_CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
KITCHEN_SINK="file://$ROOT/e2e-sites/noise-kitchen-sink/dist/index.html"

bucket() { # reads JSON on stdin, prints "total" + per-rule counts
  python3 - <<'PY'
import json,sys,collections
try:
    d=json.load(sys.stdin)
except Exception:
    print("  (no JSON — lint failed)"); sys.exit(0)
s=d.get("summary",{})
print(f"  total={s.get('total','?')}  (error={s.get('error',0)} warning={s.get('warning',0)} info={s.get('info',0)})")
c=collections.Counter(v["rule_id"] for v in d.get("violations",[]))
for k,v in sorted(c.items()):
    print(f"     {v:5d}  {k}")
PY
}

lint() { # $1=label $2=url [$3=config-flag...]
  local label="$1"; shift
  local url="$1"; shift
  rm -rf "${TMPDIR:-/tmp}/chromiumoxide-runner" 2>/dev/null || true
  echo "#### $label"
  "$BIN" lint "$url" --executable-path "$CHROME" --format json "$@" 2>/dev/null | bucket || echo "  (lint errored)"
  echo
}

[ -x "$BIN" ] || { echo "plumb binary not found at $BIN (build it: cargo build)"; exit 1; }

echo "== Plumb noise scoreboard =="
lint "noise-kitchen-sink (curated config)" "$KITCHEN_SINK" --config "$ROOT/e2e-sites/plumb.toml"

if [ "${1:-}" = "--live" ]; then
  lint "example.com (default config)"        "https://example.com"
  lint "ui.shadcn.com (default config)"      "https://ui.shadcn.com"
  lint "news.ycombinator.com (default config)" "https://news.ycombinator.com"
fi
