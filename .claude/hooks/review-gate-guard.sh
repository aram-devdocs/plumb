#!/usr/bin/env bash
# PreToolUse (matcher: Agent / Task)
# Blocks dispatch of 03-code-quality-reviewer until 02-spec-reviewer has
# APPROVED the current change. Keeps the review pipeline ordered.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_lib.sh"

input="$(hook_read_input)"
hook_init "$input"
prompt="$(printf '%s' "$input" | jq -r '.tool_input.prompt // empty' 2>/dev/null || true)"
subagent_type="$(printf '%s' "$input" | jq -r '.tool_input.subagent_type // empty' 2>/dev/null || true)"

is_code_review=0
if [ "$subagent_type" = "03-code-quality-reviewer" ]; then is_code_review=1; fi
if printf '%s' "$prompt" | grep -q "03-code-quality-reviewer"; then is_code_review=1; fi

if [ "$is_code_review" -eq 1 ]; then
    gates="$(hook_read_review_gates)"
    spec_verdict="$(printf '%s' "$gates" | sed -n 's/.*spec=\([A-Z_]*\).*/\1/p')"
    if [ "$spec_verdict" != "APPROVE" ]; then
        hook_block "Cannot dispatch 03-code-quality-reviewer: 02-spec-reviewer has not returned APPROVE yet (current: ${spec_verdict:-none})."
    fi
fi

hook_allow
