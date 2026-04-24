#!/usr/bin/env bash
# PreToolUse hook — scan the content of pending Write/Edit/MultiEdit
# operations for secrets before the file hits disk.

set -euo pipefail

input="$(cat)"

# The payload shape differs per tool; `content`, `new_string`, and each
# `edits[].new_string` are the candidate fields.
content="$(printf '%s' "$input" | jq -r '
  [
    .tool_input.content // empty,
    .tool_input.new_string // empty,
    (.tool_input.edits // [] | .[] | .new_string)
  ] | map(select(. != null)) | join("\n")
' 2>/dev/null || true)"

if [ -z "$content" ]; then
    exit 0
fi

patterns=(
    'AKIA[0-9A-Z]{16}|AWS access key id'
    'sk-ant-[A-Za-z0-9_-]{80,}|Anthropic API token'
    'sk-[A-Za-z0-9]{32,}|OpenAI-style API token'
    'ghp_[A-Za-z0-9]{36}|GitHub personal access token'
    'gho_[A-Za-z0-9]{36}|GitHub OAuth token'
    'ghs_[A-Za-z0-9]{36}|GitHub server token'
    'xox[baprs]-[A-Za-z0-9-]+|Slack token'
    '-----BEGIN (RSA |OPENSSH |PGP |EC )?PRIVATE KEY-----|PEM private key'
)

for entry in "${patterns[@]}"; do
    regex="${entry%%|*}"
    reason="${entry#*|}"
    if printf '%s' "$content" | grep -Eq "$regex"; then
        jq -n --arg reason "$reason" '{
            "decision": "block",
            "reason": ("write blocked: content matches pattern for " + $reason)
        }'
        exit 0
    fi
done

exit 0
