#!/usr/bin/env bash
# PreToolUse guard — block dangerous bash commands before Claude runs them.
#
# Reads the pending tool call as JSON on stdin; writes a JSON decision
# on stdout. See https://docs.claude.com/en/docs/claude-code/hooks for
# the full schema.

set -euo pipefail

input="$(cat)"
cmd="$(printf '%s' "$input" | jq -r '.tool_input.command // empty' 2>/dev/null || true)"

if [ -z "$cmd" ]; then
    exit 0
fi

# Patterns and the reason they're blocked.
patterns=(
    'rm[[:space:]]+-rf[[:space:]]+/($|[[:space:]])|refusing rm -rf / (root deletion)'
    'rm[[:space:]]+-rf[[:space:]]+\$HOME|refusing rm -rf $HOME'
    'rm[[:space:]]+-rf[[:space:]]+~/($|[[:space:]])|refusing rm -rf ~/'
    'git[[:space:]]+push([[:space:]]+[^[:space:]]+)*[[:space:]]+(-f|--force)([[:space:]]|$)|refusing git push --force / -f (use --force-with-lease instead, which is hook-safe)'
    'git[[:space:]]+[^[:space:]]+([[:space:]]+[^[:space:]]+)*[[:space:]]+--no-verify([[:space:]]|$)|refusing --no-verify (lefthook pre-commit/pre-push hooks must not be skipped; fix the hook failure)'
    'git[[:space:]]+reset[[:space:]]+--hard|refusing git reset --hard'
    'git[[:space:]]+clean[[:space:]]+-.*f|refusing git clean -f'
    'cargo[[:space:]]+publish[[:space:]]+--force|refusing cargo publish --force'
    'chmod[[:space:]]+-R[[:space:]]+777|refusing chmod -R 777'
    '>[[:space:]]*/dev/sd[a-z]|refusing raw disk write'
    ':\\(\\)[[:space:]]*\\{[[:space:]]*:[[:space:]]*\\|[[:space:]]*:&|refusing fork bomb'
)

for entry in "${patterns[@]}"; do
    regex="${entry%%|*}"
    reason="${entry#*|}"
    if printf '%s' "$cmd" | grep -Eq "$regex"; then
        jq -n --arg reason "$reason" --arg cmd "$cmd" '{
            "decision": "block",
            "reason": ($reason + ": " + $cmd)
        }'
        exit 0
    fi
done

exit 0
