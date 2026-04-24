#!/usr/bin/env bash
# SubagentStop (matcher: 02-spec-reviewer|03-code-quality-reviewer|04-test-runner)
# Parses the subagent's verdict line from the transcript and records it in
# .claude/state/<session>.review-gates so review-gate-guard.sh can
# enforce ordering on the next dispatch.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_lib.sh"

input="$(hook_read_input)"
transcript="$(printf '%s' "$input" | jq -r '.transcript_path // empty' 2>/dev/null || true)"
subagent="$(printf '%s' "$input" | jq -r '.subagent // empty' 2>/dev/null || true)"

if [ -z "$transcript" ] || [ ! -f "$transcript" ]; then
    hook_allow
fi

last_assistant="$(tac "$transcript" \
    | jq -r 'select(.type == "assistant") | .message.content[0].text // empty' 2>/dev/null \
    | head -n 1 || true)"

verdict="$(printf '%s' "$last_assistant" | grep -Eo '^Verdict:[[:space:]]+(APPROVE|REQUEST_CHANGES|BLOCK)' \
    | awk '{print $NF}' | head -n 1 || true)"

if [ -z "$verdict" ]; then
    hook_allow
fi

mkdir -p "$HOOK_STATE_DIR"
state_file="$HOOK_STATE_DIR/${HOOK_SESSION_ID}.review-gates"

# Determine which slot to update.
slot=""
case "$subagent" in
    02-spec-reviewer) slot="spec" ;;
    03-code-quality-reviewer) slot="code" ;;
    04-test-runner) slot="test" ;;
esac

if [ -n "$slot" ]; then
    prior="$(cat "$state_file" 2>/dev/null || true)"
    # Remove any existing entry for this slot.
    new="$(printf '%s' "$prior" | tr ';' '\n' | grep -v "^${slot}=" || true)"
    printf '%s;%s=%s' "$new" "$slot" "$verdict" | sed 's/^;//' > "$state_file"
fi

hook_allow
