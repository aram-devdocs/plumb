#!/usr/bin/env bash
# End-to-end test for the review-gate session id pinning.
#
# Regression test for the bug where HOOK_SESSION_ID fell back to
# `$(date +%s)-$$` per hook invocation, so review-gate-tracker.sh and
# review-gate-guard.sh wrote and read different state files and the
# guard always blocked 03-code-quality-reviewer dispatch even after
# 02-spec-reviewer approved.
#
# Simulates two separate bash processes (as the real harness would)
# with CLAUDE_SESSION_ID UNSET, pushing a consistent .session_id via
# the stdin JSON payload that Claude Code always sends.

set -euo pipefail

HOOKS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$HOOKS_DIR/../.." && pwd)"
STATE_DIR="$REPO_ROOT/.claude/state"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

# Drop anything we write into the real .claude/state so we don't pollute
# the developer's session.
cleanup_state() {
    local sid="$1"
    rm -f "$STATE_DIR/${sid}.review-gates" \
          "$STATE_DIR/${sid}.role" \
          "$STATE_DIR/${sid}.delegations" 2>/dev/null || true
}

fail() {
    printf 'FAIL: %s\n' "$1" >&2
    exit 1
}

pass() {
    printf 'PASS: %s\n' "$1"
}

# --- Fixture: minimal transcript with a spec-reviewer APPROVE verdict.
transcript="$TMP_DIR/spec-transcript.jsonl"
cat >"$transcript" <<'EOF'
{"type":"user","message":{"role":"user","content":"review"}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Verdict: APPROVE"}]}}
EOF

session_id="sess-test-$$"
cleanup_state "$session_id"

# --- Step 1: run review-gate-tracker.sh in its own shell, no
# CLAUDE_SESSION_ID exported. It must derive the session from stdin
# JSON and write to $STATE_DIR/$session_id.review-gates.
tracker_input="$(jq -n \
    --arg sid "$session_id" \
    --arg tp "$transcript" \
    --arg cwd "$REPO_ROOT" \
    --arg sub "02-spec-reviewer" \
    '{session_id: $sid, transcript_path: $tp, cwd: $cwd, subagent: $sub}')"

env -u CLAUDE_SESSION_ID \
    CLAUDE_PROJECT_DIR="$REPO_ROOT" \
    bash "$HOOKS_DIR/review-gate-tracker.sh" <<<"$tracker_input" >/dev/null

gates_file="$STATE_DIR/${session_id}.review-gates"
[ -f "$gates_file" ] || fail "tracker did not write $gates_file (session_id from stdin was ignored)"
grep -q 'spec=APPROVE' "$gates_file" || fail "expected spec=APPROVE in $gates_file, got: $(cat "$gates_file")"
pass "tracker wrote $gates_file with spec=APPROVE"

# --- Step 2: a *different* bash process (new PID, possibly new epoch
# second) runs review-gate-guard.sh with the same session_id in stdin
# JSON. It must read the same state file and allow the dispatch.
guard_input="$(jq -n \
    --arg sid "$session_id" \
    --arg cwd "$REPO_ROOT" \
    '{session_id: $sid, cwd: $cwd, tool_input: {subagent_type: "03-code-quality-reviewer", prompt: "review the code"}}')"

guard_output="$(env -u CLAUDE_SESSION_ID \
    CLAUDE_PROJECT_DIR="$REPO_ROOT" \
    bash "$HOOKS_DIR/review-gate-guard.sh" <<<"$guard_input")"

decision="$(printf '%s' "$guard_output" | jq -r '.decision // empty')"
[ "$decision" = "allow" ] || fail "guard should have allowed 03-code-quality-reviewer after spec APPROVE, got: $guard_output"
pass "guard allows 03-code-quality-reviewer after spec APPROVE (cross-process session pinning works)"

# --- Step 3: guard must still block when spec has NOT approved.
cleanup_state "$session_id"

guard_output="$(env -u CLAUDE_SESSION_ID \
    CLAUDE_PROJECT_DIR="$REPO_ROOT" \
    bash "$HOOKS_DIR/review-gate-guard.sh" <<<"$guard_input")"

decision="$(printf '%s' "$guard_output" | jq -r '.decision // empty')"
[ "$decision" = "block" ] || fail "guard should have blocked 03-code-quality-reviewer with no prior spec verdict, got: $guard_output"
pass "guard still blocks 03-code-quality-reviewer when no spec verdict present"

# --- Step 4: fallback path — when neither CLAUDE_SESSION_ID nor
# .session_id in input is available, both hooks share the stable
# "current" sentinel so ordering still holds.
cleanup_state "current"

fallback_tracker_input="$(jq -n \
    --arg tp "$transcript" \
    --arg cwd "$REPO_ROOT" \
    --arg sub "02-spec-reviewer" \
    '{transcript_path: $tp, cwd: $cwd, subagent: $sub}')"

env -u CLAUDE_SESSION_ID \
    CLAUDE_PROJECT_DIR="$REPO_ROOT" \
    bash "$HOOKS_DIR/review-gate-tracker.sh" <<<"$fallback_tracker_input" >/dev/null

[ -f "$STATE_DIR/current.review-gates" ] || fail "fallback tracker did not write $STATE_DIR/current.review-gates"

fallback_guard_input="$(jq -n \
    --arg cwd "$REPO_ROOT" \
    '{cwd: $cwd, tool_input: {subagent_type: "03-code-quality-reviewer", prompt: "review"}}')"

guard_output="$(env -u CLAUDE_SESSION_ID \
    CLAUDE_PROJECT_DIR="$REPO_ROOT" \
    bash "$HOOKS_DIR/review-gate-guard.sh" <<<"$fallback_guard_input")"

decision="$(printf '%s' "$guard_output" | jq -r '.decision // empty')"
[ "$decision" = "allow" ] || fail "fallback path: guard should allow 03-code-quality-reviewer, got: $guard_output"
pass "fallback (no session_id anywhere) shares stable 'current' id"

cleanup_state "current"
cleanup_state "$session_id"

# --- Step 5: regression for the macOS portability + verdict-on-last-
# line bug. Real reviewer responses are multi-line with `Verdict:` on
# the final line; the tracker must extract the verdict from anywhere
# in the latest assistant message, not just the first line. This case
# also exercises the `tac`-less reversal path on BSD/macOS.
multiline_transcript="$TMP_DIR/multiline-spec-transcript.jsonl"
multiline_text="All key claims verified.\n\nPunch list:\n- placeholder retired\n- docs in place\n\nVerdict: APPROVE"
jq -n --arg t "$(printf '%b' "$multiline_text")" '
    [
        {type: "user", message: {role: "user", content: "review"}},
        {type: "assistant", message: {role: "assistant", content: [{type: "text", text: $t}]}}
    ]
    | .[]
' >"$multiline_transcript"

multiline_session_id="sess-test-multiline-$$"
cleanup_state "$multiline_session_id"

multiline_input="$(jq -n \
    --arg sid "$multiline_session_id" \
    --arg tp "$multiline_transcript" \
    --arg cwd "$REPO_ROOT" \
    --arg sub "02-spec-reviewer" \
    '{session_id: $sid, transcript_path: $tp, cwd: $cwd, subagent: $sub}')"

env -u CLAUDE_SESSION_ID \
    CLAUDE_PROJECT_DIR="$REPO_ROOT" \
    bash "$HOOKS_DIR/review-gate-tracker.sh" <<<"$multiline_input" >/dev/null

multiline_gates_file="$STATE_DIR/${multiline_session_id}.review-gates"
[ -f "$multiline_gates_file" ] || fail "tracker did not write $multiline_gates_file for multi-line response (verdict on last line)"
grep -q 'spec=APPROVE' "$multiline_gates_file" || fail "tracker missed verdict on last line of multi-line response, got: $(cat "$multiline_gates_file")"
pass "tracker extracts verdict from final line of multi-line response"

# Validator must accept the same multi-line shape.
validator_output="$(env -u CLAUDE_SESSION_ID \
    CLAUDE_PROJECT_DIR="$REPO_ROOT" \
    bash "$HOOKS_DIR/review-verdict-validator.sh" <<<"$multiline_input" || true)"
decision="$(printf '%s' "$validator_output" | jq -r '.decision // empty' 2>/dev/null || true)"
[ "$decision" != "block" ] || fail "validator wrongly blocked multi-line response with verdict on last line, got: $validator_output"
pass "validator accepts multi-line response with verdict on last line"

cleanup_state "$multiline_session_id"
pass "all review-gate session id tests passed"
