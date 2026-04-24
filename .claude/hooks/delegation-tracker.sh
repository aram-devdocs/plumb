#!/usr/bin/env bash
# PostToolUse (matcher: Agent / Task)
# Logs every subagent dispatch to .claude/state/<session>.delegations
# for audit + debug. Non-blocking.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_lib.sh"

input="$(hook_read_input)"
agent_type="$(printf '%s' "$input" | jq -r '.tool_input.subagent_type // .tool_input.agent_type // "unknown"' 2>/dev/null || echo unknown)"
prompt="$(printf '%s' "$input" | jq -r '.tool_input.prompt // empty' 2>/dev/null || true)"
snippet="$(printf '%s' "$prompt" | head -c 120 | tr '\n' ' ')"

mkdir -p "$HOOK_STATE_DIR"
# Deterministic logline: no wall-clock in plumb-core, but hooks are meta.
printf '%s | %s | %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$agent_type" "$snippet" \
    >> "$HOOK_STATE_DIR/${HOOK_SESSION_ID}.delegations" 2>/dev/null || true

exit 0
