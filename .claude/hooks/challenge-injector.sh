#!/usr/bin/env bash
# UserPromptSubmit hook
# Pre-pends a short "challenge-me" nudge when the user submits a prompt
# that looks like a final approval ("ship it", "LGTM", "looks good"). Helps
# surface missed validation before the session continues.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_lib.sh"

input="$(hook_read_input)"
prompt="$(printf '%s' "$input" | jq -r '.prompt // empty' 2>/dev/null || true)"

triggers='(^|\s)(ship it|looks good|lgtm|all good|perfect|go for it|approved)(\s|\?|!|\.|$)'

if printf '%s' "$prompt" | grep -Eiq "$triggers"; then
    jq -n '{
        "hookSpecificOutput": {
            "additionalContext": "Challenge: before proceeding, confirm (1) every gate in `just validate` passed, (2) no new unwrap/expect/println in library crates, (3) snapshots are intentional, (4) CHANGELOG updated if user-visible. If any are unclear, say so instead of acting."
        }
    }'
    exit 0
fi

exit 0
