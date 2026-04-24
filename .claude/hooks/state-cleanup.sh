#!/usr/bin/env bash
# Stop hook
# Removes session-scoped state files.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_lib.sh"

if [ ! -d "$HOOK_STATE_DIR" ]; then
    exit 0
fi

for ext in role delegations review-gates plan-guard-override; do
    rm -f "$HOOK_STATE_DIR/${HOOK_SESSION_ID}.${ext}" 2>/dev/null || true
done

exit 0
