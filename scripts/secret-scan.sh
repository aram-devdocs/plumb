#!/usr/bin/env bash
# Scan staged files for common secret patterns.
#
# Usage: secret-scan.sh [file ...]
#
# Exits 0 if no matches; non-zero on the first match. Emits a short
# explanation so the author knows what matched.

set -euo pipefail

if [ $# -eq 0 ]; then
    exit 0
fi

# Patterns. Each line is `regex|description`. Keep these conservative —
# false positives annoy the author and teach them to bypass. Tune over time.
patterns=(
    'AKIA[0-9A-Z]{16}|AWS access key id'
    'ASIA[0-9A-Z]{16}|AWS STS access key id'
    'aws_secret_access_key[[:space:]]*=[[:space:]]*[A-Za-z0-9/+=]{40}|AWS secret access key assignment'
    'sk-[A-Za-z0-9]{32,}|OpenAI/Anthropic-style API token'
    'sk-ant-[A-Za-z0-9_-]{80,}|Anthropic API token'
    'ghp_[A-Za-z0-9]{36}|GitHub personal access token'
    'gho_[A-Za-z0-9]{36}|GitHub OAuth token'
    'ghu_[A-Za-z0-9]{36}|GitHub user-to-server token'
    'ghs_[A-Za-z0-9]{36}|GitHub server token'
    'xox[baprs]-[A-Za-z0-9-]+|Slack token'
    '-----BEGIN (RSA |OPENSSH |PGP |EC )?PRIVATE KEY-----|PEM private key'
)

found_any=0

for file in "$@"; do
    # Skip binary and deleted files.
    if [ ! -f "$file" ]; then continue; fi
    if ! grep -Iq . "$file" 2>/dev/null; then continue; fi

    for entry in "${patterns[@]}"; do
        regex="${entry%%|*}"
        desc="${entry#*|}"
        if grep -E --binary-files=without-match -qn "$regex" "$file"; then
            found_any=1
            echo "✖ secret-scan: $file matches pattern for $desc" >&2
            grep -E --binary-files=without-match -n "$regex" "$file" >&2 || true
        fi
    done
done

if [ "$found_any" -ne 0 ]; then
    echo >&2
    echo "One or more staged files contain what looks like a secret." >&2
    echo "Remove or rotate the value. If this is a false positive, narrow the" >&2
    echo "pattern in scripts/secret-scan.sh rather than skipping the hook." >&2
    exit 1
fi

exit 0
