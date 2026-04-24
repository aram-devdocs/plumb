#!/usr/bin/env bash
# PreToolUse (matcher: Write|Edit|MultiEdit)
# Prevents the root orchestrator from directly writing .rs implementation
# files — forces dispatch to a subagent. Infrastructure files under
# .agents/, .claude/, docs/, and .github/ are always allowed.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_lib.sh"

input="$(hook_read_input)"
hook_init "$input"
file_path="$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty' 2>/dev/null || true)"
role="$(hook_read_role)"

# Subagents always allowed to write.
case "$role" in
    subagent:*) hook_allow ;;
esac

# Only guard when file is a .rs source file under crates/*/src/.
case "$file_path" in
    */crates/*/src/*.rs) ;;
    *) hook_allow ;;
esac

# Infra exceptions: never block these even from root.
case "$file_path" in
    *.claude/*|*.agents/*|*/docs/*|*/.github/*) hook_allow ;;
esac

if [ "$role" = "root" ] || [ -z "$role" ]; then
    hook_block "Root orchestrator should dispatch to a subagent for Rust implementation (file: $file_path). Use the 01-implementer agent."
fi

hook_allow
