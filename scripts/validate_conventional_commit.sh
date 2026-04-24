#!/usr/bin/env bash
# Validate a commit message against Conventional Commits v1.0.0.
#
# Usage: validate_conventional_commit.sh <commit-msg-file>
#
# Exits 0 if the first non-comment line matches the spec, non-zero otherwise.

set -euo pipefail

if [ $# -lt 1 ]; then
    echo "usage: $0 <commit-msg-file>" >&2
    exit 2
fi

msg_file="$1"
if [ ! -f "$msg_file" ]; then
    echo "error: commit message file not found: $msg_file" >&2
    exit 2
fi

# Pick the first non-comment, non-empty line.
first_line="$(grep -v '^#' "$msg_file" | grep -v '^$' | head -n 1 || true)"

if [ -z "$first_line" ]; then
    echo "error: commit message is empty." >&2
    exit 1
fi

# Allow merge / revert commits as-is — git generates their messages.
if [[ "$first_line" =~ ^Merge\  ]] || [[ "$first_line" =~ ^Revert\  ]]; then
    exit 0
fi

# Conventional Commits pattern:
#   <type>[optional scope][!]: <description>
# Types: feat, fix, perf, refactor, docs, test, build, ci, chore, style, revert.
# Scope: any identifier, path, or rule id (letters, digits, /, -, _).
pattern='^(feat|fix|perf|refactor|docs|test|build|ci|chore|style|revert)(\([a-zA-Z0-9/_-]+\))?!?: .{1,72}$'

if [[ "$first_line" =~ $pattern ]]; then
    exit 0
fi

cat >&2 <<EOF
error: commit message does not follow Conventional Commits.

  Got:      $first_line

  Expected: <type>(<scope>): <description>

  Valid types: feat, fix, perf, refactor, docs, test, build, ci, chore, style, revert

  Examples:
    feat(core): add spacing/hard-coded-gap rule
    fix(cli): exit 2 on driver errors
    chore(deps): bump chromiumoxide to 0.8

  See CONTRIBUTING.md for the full policy.
EOF
exit 1
