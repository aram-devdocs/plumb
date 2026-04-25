#!/usr/bin/env bash
# SessionStart hook
# Writes role=root into the per-session state file for the root harness.
#
# Subagents do NOT fire SessionStart in Claude Code — they share the
# parent's session. Subagent context is detected at PreToolUse time
# from the .agent_type field of the hook input (Claude Code populates
# it for every tool call originating from a subagent). See
# delegation-guard.sh for the consumer.
#
# This hook therefore only ever runs in a root context and only
# writes role=root. It exists so save-session.sh has a stable place
# to read the role from when summarising the session on Stop.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_lib.sh"

input="$(hook_read_input)"
hook_init "$input"

mkdir -p "$HOOK_STATE_DIR"
printf '%s' "root" > "$HOOK_STATE_DIR/${HOOK_SESSION_ID}.role"

exit 0
