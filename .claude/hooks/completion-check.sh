#!/usr/bin/env bash
# Stop hook
# Advisory check for uncommitted work and TODO/FIXME markers before the
# session ends. Never blocks — only warns.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_lib.sh"

HOOK="completion-check"
cd "$HOOK_CWD" 2>/dev/null || exit 0

if ! git rev-parse --show-toplevel >/dev/null 2>&1; then
    exit 0
fi

status="$(git status --porcelain 2>/dev/null || true)"
if [ -n "$status" ]; then
    count="$(printf '%s\n' "$status" | wc -l | tr -d ' ')"
    hook_warn "$HOOK" "$count uncommitted file(s) — consider committing before ending the session."
fi

recent="$(git diff --name-only HEAD 2>/dev/null || true)"
if [ -n "$recent" ]; then
    while IFS= read -r file; do
        [ -z "$file" ] && continue
        [ ! -f "$file" ] && continue
        matches="$(grep -n 'TODO\|FIXME' "$file" 2>/dev/null || true)"
        if [ -n "$matches" ]; then
            hook_warn "$HOOK" "TODO/FIXME in $file: $matches"
        fi
    done <<<"$recent"
fi

exit 0
