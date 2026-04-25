#!/usr/bin/env bash
# SubagentStop hook — enforce that reviewer subagents end their final
# response with a machine-parseable `Verdict: APPROVE|REQUEST_CHANGES|BLOCK`
# line. If they don't, re-prompt the subagent rather than accepting a
# freeform review.

set -euo pipefail

input="$(cat)"
transcript_path="$(printf '%s' "$input" | jq -r '.transcript_path // empty')"
subagent="$(printf '%s' "$input" | jq -r '.subagent // empty')"

if [ -z "$transcript_path" ] || [ ! -f "$transcript_path" ]; then
    exit 0
fi

# Read the subagent's last assistant message in full. Transcripts are
# JSONL with one message per line; the verdict is conventionally the
# final line of the latest assistant message, so we read the whole text
# and grep its lines.
last_assistant="$(jq -sr 'map(select(.type == "assistant") | .message.content[0].text // empty)[-1] // empty' "$transcript_path" 2>/dev/null || true)"

if [ -z "$last_assistant" ]; then
    exit 0
fi

if printf '%s\n' "$last_assistant" | grep -Eq '^Verdict:[[:space:]]+(APPROVE|REQUEST_CHANGES|BLOCK)[[:space:]]*$'; then
    exit 0
fi

jq -n --arg subagent "$subagent" '{
    "decision": "block",
    "reason": ("reviewer subagent " + $subagent + " must end with a line matching: Verdict: APPROVE|REQUEST_CHANGES|BLOCK")
}'
