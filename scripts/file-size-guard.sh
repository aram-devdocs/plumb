#!/usr/bin/env bash
# Warn when a staged Rust source file exceeds the soft 500-line guideline.
#
# Large files aren't forbidden, but they warrant explicit attention —
# usually a sign the module should split. The hook fails only if a file
# crosses the hard 1000-line limit.

set -euo pipefail

SOFT=500
HARD=1000

exit_code=0

for file in "$@"; do
    # Guard only Rust sources under crates/**/src.
    case "$file" in
        crates/*/src/*.rs) ;;
        *) continue ;;
    esac
    if [ ! -f "$file" ]; then continue; fi

    lines=$(wc -l < "$file" | tr -d ' ')
    if [ "$lines" -ge "$HARD" ]; then
        echo "✖ file-size-guard: $file has $lines lines (hard limit $HARD)." >&2
        echo "  Split the module before committing." >&2
        exit_code=1
    elif [ "$lines" -ge "$SOFT" ]; then
        echo "▸ file-size-guard: $file has $lines lines (soft limit $SOFT)." >&2
        echo "  Consider splitting; this is a warning only." >&2
    fi
done

exit "$exit_code"
