#!/usr/bin/env bash
# Enforce the hierarchical AGENTS.md contract:
#
# 1. Every scoped path has an AGENTS.md under its line-count budget.
# 2. Every AGENTS.md has a sibling CLAUDE.md symlink pointing to it.
# 3. No omnifol / omniscript / tRPC / pnpm / "target dev" residue.
# 4. Every scoped AGENTS.md references the root via "See /AGENTS.md".
#
# Invoked from `just check-agents`, the lefthook pre-commit step, and
# the CI preflight job.

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

# key=rel_path, value=max_lines.
declare -a SCOPES=(
    "AGENTS.md:80"
    "crates/plumb-core/AGENTS.md:50"
    "crates/plumb-format/AGENTS.md:40"
    "crates/plumb-cdp/AGENTS.md:50"
    "crates/plumb-config/AGENTS.md:40"
    "crates/plumb-mcp/AGENTS.md:60"
    "crates/plumb-cli/AGENTS.md:60"
    "xtask/AGENTS.md:40"
    "docs/AGENTS.md:50"
    "docs/src/rules/AGENTS.md:40"
    "docs/runbooks/AGENTS.md:45"
    ".agents/AGENTS.md:40"
    ".agents/rules/AGENTS.md:42"
    ".agents/skills/AGENTS.md:40"
)

FORBIDDEN=(
    "omnifol"
    "omniscript"
    "@omnifol"
    "trpc-procedure"
    "trading-domain-expert"
    "omniscript-domain-expert"
    "pnpm typecheck"
    "pnpm lint"
    "pnpm --filter"
    "--base dev"
    "git checkout dev"
)

ALLOWLIST_DIRS=(
    ".agents/skills/humanizer"
)

errors=0
total=0

is_allowlisted() {
    local file="$1"
    for d in "${ALLOWLIST_DIRS[@]}"; do
        case "$file" in "$d"/*) return 0 ;; esac
    done
    return 1
}

for entry in "${SCOPES[@]}"; do
    rel="${entry%%:*}"
    budget="${entry##*:}"
    total=$((total + 1))

    if [ ! -f "$rel" ]; then
        echo "✖ missing AGENTS.md: $rel" >&2
        errors=$((errors + 1))
        continue
    fi

    lines=$(wc -l < "$rel" | tr -d ' ')
    if [ "$lines" -gt "$budget" ]; then
        echo "✖ $rel: $lines lines (max $budget)" >&2
        errors=$((errors + 1))
    fi

    # Sibling CLAUDE.md must be a symlink to AGENTS.md.
    claude_path="$(dirname "$rel")/CLAUDE.md"
    if [ ! -L "$claude_path" ]; then
        echo "✖ $claude_path is not a symlink (should point to AGENTS.md)" >&2
        errors=$((errors + 1))
    else
        target="$(readlink "$claude_path")"
        if [ "$target" != "AGENTS.md" ]; then
            echo "✖ $claude_path → '$target' (expected 'AGENTS.md')" >&2
            errors=$((errors + 1))
        fi
    fi

    # Scoped files must reference the root.
    if [ "$rel" != "AGENTS.md" ]; then
        if ! grep -q "See \`?/AGENTS.md\`?" "$rel" && ! grep -q "/AGENTS.md" "$rel"; then
            echo "✖ $rel must link back to /AGENTS.md" >&2
            errors=$((errors + 1))
        fi
    fi
done

# Forbidden-phrase scan across every AGENTS.md (and their CLAUDE.md
# symlinks, via readlink, but we only check the underlying content
# once through AGENTS.md).
for entry in "${SCOPES[@]}"; do
    rel="${entry%%:*}"
    [ -f "$rel" ] || continue
    if is_allowlisted "$rel"; then continue; fi
    for phrase in "${FORBIDDEN[@]}"; do
        if grep -iq -- "$phrase" "$rel"; then
            echo "✖ $rel contains forbidden phrase '$phrase'" >&2
            errors=$((errors + 1))
        fi
    done
done

if [ "$errors" -ne 0 ]; then
    echo "" >&2
    echo "check-agents-md: $errors error(s) across $total scope(s)." >&2
    exit 1
fi

echo "▸ check-agents-md: $total AGENTS.md scopes OK (line budgets, CLAUDE.md symlinks, no drift)."
