#!/usr/bin/env bash
# PreToolUse (matcher: Write|Edit|MultiEdit)
# Prevents the root orchestrator from directly writing .rs implementation
# files — forces dispatch to a subagent. Infrastructure files under
# .agents/, .claude/, docs/, and .github/ (relative to the repo root)
# are always allowed.
#
# Subagent context is detected from the .agent_type field of the
# PreToolUse input — Claude Code populates it for every tool call
# originating inside a subagent. The role file written by
# role-marker.sh at SessionStart is consulted as a secondary signal
# (e.g. for harnesses that propagate role through the file rather
# than the per-call payload).

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_lib.sh"

input="$(hook_read_input)"
hook_init "$input"
file_path="$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty' 2>/dev/null || true)"
agent_type="$(printf '%s' "$input" | jq -r '.agent_type // empty' 2>/dev/null || true)"
role="$(hook_read_role)"

# Subagents always allowed to write. Primary signal is .agent_type
# from the hook input itself; fallback is the role file.
if [ -n "$agent_type" ]; then
    hook_allow
fi
case "$role" in
    subagent:*) hook_allow ;;
esac

# Only guard when file is a .rs source file under crates/*/src/.
case "$file_path" in
    */crates/*/src/*.rs) ;;
    *) hook_allow ;;
esac

# Infra exceptions: never block these even from root. Match against
# the repo-relative path so a worktree at .claude/worktrees/<name>/
# does not accidentally match the .claude/ infra prefix on every
# crate file inside that worktree.
rel_path="${file_path#"$HOOK_CWD/"}"
case "$rel_path" in
    .claude/*|.agents/*|docs/*|.github/*) hook_allow ;;
esac

# Merge conflict resolution is mechanical arbitration, not new Rust
# implementation work. Limit the exception to the Rust file that is
# currently unmerged so unrelated implementation files stay guarded.
conflicted_paths="$(git -C "$HOOK_CWD" diff --name-only --diff-filter=U 2>/dev/null || true)"
if printf '%s\n' "$conflicted_paths" | grep -qxF "$rel_path"; then
    hook_allow
fi

if [ "$role" = "root" ] || [ -z "$role" ]; then
    hook_block "Root orchestrator should dispatch to a subagent for Rust implementation (file: $file_path). Use the 01-implementer agent."
fi

hook_allow
