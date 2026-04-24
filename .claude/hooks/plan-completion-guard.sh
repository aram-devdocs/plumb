#!/usr/bin/env bash
# Stop hook
# Warns if the session is ending with uncommitted work on a feature branch
# or pending todos in an active .agents/runs/ run. First stop warns; a
# marker file is written so a second stop passes through.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_lib.sh"

input="$(hook_read_input)"
hook_init "$input"

HOOK="plan-completion-guard"
override="$HOOK_STATE_DIR/${HOOK_SESSION_ID}.plan-guard-override"

if [ -f "$override" ]; then
    rm -f "$override" 2>/dev/null || true
    exit 0
fi

cd "$HOOK_CWD" 2>/dev/null || exit 0
warnings=()

branch="$(git branch --show-current 2>/dev/null || true)"
if [ -n "$branch" ] && [ "$branch" != "main" ] && [ "$branch" != "master" ]; then
    status="$(git status --porcelain 2>/dev/null || true)"
    if [ -n "$status" ]; then
        warnings+=("Uncommitted changes on feature branch '$branch'.")
    fi
fi

if [ -d "$HOOK_CWD/.agents/runs" ]; then
    while IFS= read -r state; do
        [ -z "$state" ] && continue
        pending="$(jq -r '[.todos // [] | .[] | select(.done == false)] | length' "$state" 2>/dev/null || echo 0)"
        if [ "${pending:-0}" -gt 0 ]; then
            warnings+=("Active run has $pending pending todo(s): $(dirname "$state")")
        fi
    done < <(find "$HOOK_CWD/.agents/runs" -name "state.json" -type f 2>/dev/null)
fi

if [ "${#warnings[@]}" -gt 0 ]; then
    mkdir -p "$HOOK_STATE_DIR"
    date -u +%Y-%m-%dT%H:%M:%SZ > "$override"
    for w in "${warnings[@]}"; do
        hook_warn "$HOOK" "$w"
    done
    hook_warn "$HOOK" "Stop again to confirm exit."
fi

exit 0
