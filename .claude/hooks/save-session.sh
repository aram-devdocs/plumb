#!/usr/bin/env bash
# Stop hook
# Appends a compact session summary to .claude/state/sessions.log.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_lib.sh"

input="$(hook_read_input)"
hook_init "$input"
mkdir -p "$HOOK_STATE_DIR"
log="$HOOK_STATE_DIR/sessions.log"

started="$(printf '%s' "$input" | jq -r '.session_started_at // empty' 2>/dev/null || true)"
ended="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
turns="$(printf '%s' "$input" | jq -r '.turn_count // 0' 2>/dev/null || echo 0)"
role="$(hook_read_role)"

printf '%s | session=%s | role=%s | started=%s | turns=%s\n' \
    "$ended" "$HOOK_SESSION_ID" "${role:-root}" "${started:-unknown}" "$turns" \
    >> "$log" 2>/dev/null || true

exit 0
