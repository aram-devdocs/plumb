#!/usr/bin/env bash
# End-to-end test for the delegation-guard subagent detection.
#
# Regression test for the bug where role-marker.sh tried to detect
# subagent vs root from the SessionStart payload's .agent_type field,
# but Claude Code never populates that field at SessionStart (subagents
# share the parent's session and don't fire SessionStart at all). The
# role file therefore always said "root", and delegation-guard.sh
# blocked every subagent's .rs edit because it could not tell the
# subagent apart from the root orchestrator.
#
# The fix: delegation-guard.sh reads .agent_type / .agent_id from the
# PreToolUse hook input itself, where Claude Code DOES populate the
# subagent identity. The role file is now only an additional signal
# for backwards compat.

set -euo pipefail

HOOKS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$HOOKS_DIR/../.." && pwd)"
STATE_DIR="$REPO_ROOT/.claude/state"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

cleanup_state() {
    local sid="$1"
    rm -f "$STATE_DIR/${sid}.role" 2>/dev/null || true
}

fail() {
    printf 'FAIL: %s\n' "$1" >&2
    exit 1
}

pass() {
    printf 'PASS: %s\n' "$1"
}

session_id="sess-deleg-$$"
cleanup_state "$session_id"

# --- Step 1: SessionStart on the root harness writes role=root.
# Claude Code never sends agent_type at SessionStart — that field is
# only present in PreToolUse / PostToolUse payloads when a subagent
# made the call.
session_start_input="$(jq -n \
    --arg sid "$session_id" \
    --arg cwd "$REPO_ROOT" \
    '{session_id: $sid, cwd: $cwd, hook_event_name: "SessionStart"}')"

env -u CLAUDE_SESSION_ID \
    CLAUDE_PROJECT_DIR="$REPO_ROOT" \
    bash "$HOOKS_DIR/role-marker.sh" <<<"$session_start_input" >/dev/null

role_file="$STATE_DIR/${session_id}.role"
[ -f "$role_file" ] || fail "role-marker did not write $role_file"
role="$(cat "$role_file")"
[ "$role" = "root" ] || fail "expected role=root at SessionStart, got role=$role"
pass "role-marker writes role=root at SessionStart (no agent_type in payload)"

# --- Step 2: PreToolUse for a Rust file from the root context (no
# agent_type in input) MUST be blocked.
root_edit_input="$(jq -n \
    --arg sid "$session_id" \
    --arg cwd "$REPO_ROOT" \
    --arg fp "$REPO_ROOT/crates/plumb-core/src/lib.rs" \
    '{session_id: $sid, cwd: $cwd, hook_event_name: "PreToolUse", tool_name: "Write", tool_input: {file_path: $fp, content: "x"}}')"

guard_output="$(env -u CLAUDE_SESSION_ID \
    CLAUDE_PROJECT_DIR="$REPO_ROOT" \
    bash "$HOOKS_DIR/delegation-guard.sh" <<<"$root_edit_input")"
decision="$(printf '%s' "$guard_output" | jq -r '.decision // empty' 2>/dev/null || true)"
[ "$decision" = "block" ] || fail "guard should BLOCK root .rs edit, got: $guard_output"
pass "guard blocks root orchestrator from editing crates/*/src/*.rs"

# --- Step 3: PreToolUse for the same Rust file from a subagent context
# (agent_type populated by Claude Code) MUST be allowed.
sub_edit_input="$(jq -n \
    --arg sid "$session_id" \
    --arg cwd "$REPO_ROOT" \
    --arg fp "$REPO_ROOT/crates/plumb-core/src/lib.rs" \
    '{session_id: $sid, cwd: $cwd, hook_event_name: "PreToolUse", tool_name: "Write", tool_input: {file_path: $fp, content: "x"}, agent_id: "abc123", agent_type: "01-implementer"}')"

guard_output="$(env -u CLAUDE_SESSION_ID \
    CLAUDE_PROJECT_DIR="$REPO_ROOT" \
    bash "$HOOKS_DIR/delegation-guard.sh" <<<"$sub_edit_input")"
decision="$(printf '%s' "$guard_output" | jq -r '.decision // empty' 2>/dev/null || true)"
[ "$decision" = "allow" ] || fail "guard should ALLOW subagent .rs edit (agent_type=01-implementer), got: $guard_output"
pass "guard allows subagent (agent_type populated) to edit crates/*/src/*.rs"

# --- Step 4: Non-Rust files always allowed, even from root.
root_md_input="$(jq -n \
    --arg sid "$session_id" \
    --arg cwd "$REPO_ROOT" \
    --arg fp "$REPO_ROOT/README.md" \
    '{session_id: $sid, cwd: $cwd, hook_event_name: "PreToolUse", tool_name: "Write", tool_input: {file_path: $fp, content: "x"}}')"

guard_output="$(env -u CLAUDE_SESSION_ID \
    CLAUDE_PROJECT_DIR="$REPO_ROOT" \
    bash "$HOOKS_DIR/delegation-guard.sh" <<<"$root_md_input")"
decision="$(printf '%s' "$guard_output" | jq -r '.decision // empty' 2>/dev/null || true)"
[ "$decision" = "allow" ] || fail "guard should ALLOW root edit of non-Rust file, got: $guard_output"
pass "guard allows root edit of non-Rust files"

# --- Step 5: Infra path exceptions (.claude/, .agents/, docs/, .github/)
# always allowed even from root.
for path in ".claude/hooks/role-marker.sh" ".agents/rules/foo.md" "docs/src/intro.md" ".github/workflows/ci.yml"; do
    infra_input="$(jq -n \
        --arg sid "$session_id" \
        --arg cwd "$REPO_ROOT" \
        --arg fp "$REPO_ROOT/$path" \
        '{session_id: $sid, cwd: $cwd, hook_event_name: "PreToolUse", tool_name: "Edit", tool_input: {file_path: $fp, content: "x"}}')"
    guard_output="$(env -u CLAUDE_SESSION_ID \
        CLAUDE_PROJECT_DIR="$REPO_ROOT" \
        bash "$HOOKS_DIR/delegation-guard.sh" <<<"$infra_input")"
    decision="$(printf '%s' "$guard_output" | jq -r '.decision // empty' 2>/dev/null || true)"
    [ "$decision" = "allow" ] || fail "guard should ALLOW root infra edit ($path), got: $guard_output"
done
pass "guard allows root edits of infra files (.claude/, .agents/, docs/, .github/)"

# --- Step 6: Merge conflict resolution is allowed from root. Conflict
# marker arbitration is mechanical, so the root orchestrator may edit
# Rust source files while the worktree has unmerged paths.
merge_repo="$TMP_DIR/merge-repo"
mkdir -p "$merge_repo/crates/plumb-core/src"
git -C "$merge_repo" init -q
git -C "$merge_repo" config user.name "Hook Test"
git -C "$merge_repo" config user.email "hook-test@example.com"
printf 'pub fn value() -> u8 { 1 }\n' > "$merge_repo/crates/plumb-core/src/lib.rs"
git -C "$merge_repo" add crates/plumb-core/src/lib.rs
git -C "$merge_repo" commit -qm "initial"
git -C "$merge_repo" checkout -qb feature
printf 'pub fn value() -> u8 { 2 }\n' > "$merge_repo/crates/plumb-core/src/lib.rs"
git -C "$merge_repo" commit -am "feature" -q
git -C "$merge_repo" checkout -q -
printf 'pub fn value() -> u8 { 3 }\n' > "$merge_repo/crates/plumb-core/src/lib.rs"
git -C "$merge_repo" commit -am "main" -q
git -C "$merge_repo" merge feature >/dev/null 2>&1 && fail "expected merge conflict"

merge_input="$(jq -n \
    --arg sid "$session_id" \
    --arg cwd "$merge_repo" \
    --arg fp "$merge_repo/crates/plumb-core/src/lib.rs" \
    '{session_id: $sid, cwd: $cwd, hook_event_name: "PreToolUse", tool_name: "Edit", tool_input: {file_path: $fp, content: "x"}}')"
guard_output="$(env -u CLAUDE_SESSION_ID \
    CLAUDE_PROJECT_DIR="$merge_repo" \
    bash "$HOOKS_DIR/delegation-guard.sh" <<<"$merge_input")"
decision="$(printf '%s' "$guard_output" | jq -r '.decision // empty' 2>/dev/null || true)"
[ "$decision" = "allow" ] || fail "guard should ALLOW root .rs edit during merge conflict resolution, got: $guard_output"
pass "guard allows root Rust edits while unmerged paths exist"

# --- Step 7: Backwards-compat — if a role file exists with
# subagent:<name>, guard still allows. This preserves the existing
# escape hatch for harnesses that DO populate agent_type at
# SessionStart (future Claude Code versions) or for manual override.
echo "subagent:01-implementer" > "$role_file"
sub_via_role_input="$(jq -n \
    --arg sid "$session_id" \
    --arg cwd "$REPO_ROOT" \
    --arg fp "$REPO_ROOT/crates/plumb-core/src/lib.rs" \
    '{session_id: $sid, cwd: $cwd, hook_event_name: "PreToolUse", tool_name: "Write", tool_input: {file_path: $fp, content: "x"}}')"
guard_output="$(env -u CLAUDE_SESSION_ID \
    CLAUDE_PROJECT_DIR="$REPO_ROOT" \
    bash "$HOOKS_DIR/delegation-guard.sh" <<<"$sub_via_role_input")"
decision="$(printf '%s' "$guard_output" | jq -r '.decision // empty' 2>/dev/null || true)"
[ "$decision" = "allow" ] || fail "guard should ALLOW when role file says subagent:*, got: $guard_output"
pass "guard still honors role-file subagent marker for backwards compat"

cleanup_state "$session_id"
pass "all delegation-guard subagent tests passed"
