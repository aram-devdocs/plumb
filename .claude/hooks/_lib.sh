#!/usr/bin/env bash
# Shared helpers for every Plumb Claude Code hook.
#
# Exports:
#   HOOK_CWD          repo root ($CLAUDE_PROJECT_DIR or git toplevel)
#   HOOK_SESSION_ID   per-session id for state files
#   HOOK_STATE_DIR    .claude/state (gitignored)
#
# Functions:
#   hook_allow            — emit JSON decision=allow and exit 0
#   hook_block <reason>   — emit JSON decision=block with reason and exit 0
#   hook_warn <hook> <msg> — write a stderr warning; never block
#   hook_read_role        — echo the current role (root|subagent:<name>|empty)
#   hook_read_review_gates — echo `spec=<verdict>;code=<verdict>` or empty

set -euo pipefail

HOOK_CWD="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
HOOK_SESSION_ID="${CLAUDE_SESSION_ID:-$(date +%s)-$$}"
HOOK_STATE_DIR="$HOOK_CWD/.claude/state"

hook_allow() {
    printf '%s' '{"decision":"allow"}'
    exit 0
}

hook_block() {
    local reason="${1:-}"
    jq -n --arg reason "$reason" '{"decision":"block","reason":$reason}'
    exit 0
}

hook_warn() {
    local name="${1:-hook}"
    local msg="${2:-}"
    printf '[%s] %s\n' "$name" "$msg" >&2
}

hook_read_role() {
    local f="$HOOK_STATE_DIR/${HOOK_SESSION_ID}.role"
    if [ -f "$f" ]; then
        cat "$f"
    else
        echo ""
    fi
}

hook_read_review_gates() {
    local f="$HOOK_STATE_DIR/${HOOK_SESSION_ID}.review-gates"
    if [ -f "$f" ]; then
        cat "$f"
    else
        echo ""
    fi
}

hook_read_input() {
    cat
}

export -f hook_allow hook_block hook_warn hook_read_role hook_read_review_gates hook_read_input
export HOOK_CWD HOOK_SESSION_ID HOOK_STATE_DIR
