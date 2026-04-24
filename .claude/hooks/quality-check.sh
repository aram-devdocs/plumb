#!/usr/bin/env bash
# PostToolUse (matcher: Write|Edit|MultiEdit)
# Runs `cargo clippy` against the crate containing a modified .rs file.
# Warns only — does not block.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/_lib.sh"

HOOK="quality-check"
input="$(hook_read_input)"
file_path="$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty' 2>/dev/null || true)"

case "$file_path" in
    */crates/*/src/*.rs) ;;
    *) exit 0 ;;
esac

# Map file path → crate (crates/<name>/src/...).
crate_name="$(printf '%s' "$file_path" | sed -nE 's|.*/crates/([^/]+)/src/.*|\1|p')"
if [ -z "$crate_name" ]; then
    exit 0
fi

cd "$HOOK_CWD" 2>/dev/null || exit 0

if ! out="$(cargo clippy -p "$crate_name" --all-targets --all-features -- -D warnings 2>&1)"; then
    hook_warn "$HOOK" "cargo clippy for $crate_name reported issues:"
    printf '%s\n' "$out" | head -n 40 >&2
fi

exit 0
