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
#   hook_read_input       — cat hook input JSON from stdin
#   hook_init <input>     — promote .session_id from the hook input JSON
#                           into HOOK_SESSION_ID. Must be called after
#                           hook_read_input in every hook that touches
#                           session-scoped state.
#   hook_read_role        — echo the current role (root|subagent:<name>|empty)
#   hook_read_review_gates — echo `spec=<verdict>;code=<verdict>` or empty

set -euo pipefail

HOOK_CWD="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
HOOK_STATE_DIR="$HOOK_CWD/.claude/state"

# Initial value — prefer $CLAUDE_SESSION_ID when the harness exports it,
# otherwise a stable per-worktree sentinel. Callers with stdin JSON in
# hand should follow up with `hook_init "$input"` so state files align
# across the hook chain. Historically the fallback was `date +%s-$$`,
# but that produced a fresh id per hook invocation on harnesses that
# don't export CLAUDE_SESSION_ID (e.g. Claude Desktop on macOS), which
# broke every cross-hook read (role, review-gates, delegations, Stop
# cleanup).
HOOK_SESSION_ID="${CLAUDE_SESSION_ID:-current}"

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

hook_read_input() {
    cat
}

hook_init() {
    local input="${1:-}"
    [ -z "$input" ] && return 0
    local sid
    sid="$(printf '%s' "$input" | jq -r '.session_id // empty' 2>/dev/null || true)"
    if [ -n "$sid" ]; then
        HOOK_SESSION_ID="$sid"
    fi
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

# Reverse a file line-by-line. Portable across BSD (macOS) and GNU
# (Linux). GNU has `tac`; BSD has `tail -r`; awk works everywhere as
# the last-resort fallback.
hook_reverse_file() {
    local f="${1:-}"
    [ -z "$f" ] && return 0
    if command -v tac >/dev/null 2>&1; then
        tac "$f"
    elif tail -r "$f" >/dev/null 2>&1; then
        tail -r "$f"
    else
        awk '{ a[NR] = $0 } END { for (i = NR; i > 0; i--) print a[i] }' "$f"
    fi
}

export -f hook_allow hook_block hook_warn hook_read_role hook_read_review_gates hook_read_input hook_init hook_reverse_file
export HOOK_CWD HOOK_SESSION_ID HOOK_STATE_DIR
