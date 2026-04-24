#!/usr/bin/env bash
# SessionStart hook
# Detects root vs subagent session and writes role into state dir.
# Consumed by delegation-guard.sh.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_lib.sh"

input="$(hook_read_input)"
agent_type="$(printf '%s' "$input" | jq -r '.agent_type // empty' 2>/dev/null || true)"

if [ -n "$agent_type" ]; then
    role="subagent:$agent_type"
else
    role="root"
fi

mkdir -p "$HOOK_STATE_DIR"
printf '%s' "$role" > "$HOOK_STATE_DIR/${HOOK_SESSION_ID}.role"

exit 0
