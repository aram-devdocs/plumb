#!/usr/bin/env bash
# SubagentStop (matcher: 02-spec-reviewer|03-code-quality-reviewer|04-test-runner)
# Parses the subagent's verdict line from the transcript and records it in
# .claude/state/<session>.review-gates so review-gate-guard.sh can
# enforce ordering on the next dispatch.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_lib.sh"

input="$(hook_read_input)"
hook_init "$input"
transcript="$(printf '%s' "$input" | jq -r '.transcript_path // empty' 2>/dev/null || true)"
subagent="$(printf '%s' "$input" | jq -r '.subagent // empty' 2>/dev/null || true)"

if [ -z "$transcript" ] || [ ! -f "$transcript" ]; then
    hook_allow
fi

# Pull the full text of the latest assistant message, then grep its
# lines for the verdict marker. The verdict is conventionally the last
# line of a reviewer response, so we must search the whole text — not
# just the first line.
last_assistant="$(jq -sr 'map(select(.type == "assistant") | .message.content[0].text // empty)[-1] // empty' "$transcript" 2>/dev/null || true)"

verdict="$(printf '%s\n' "$last_assistant" \
    | grep -E '^Verdict:[[:space:]]+(APPROVE|REQUEST_CHANGES|BLOCK)[[:space:]]*$' \
    | awk '{print $2}' | tail -n 1 || true)"

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
