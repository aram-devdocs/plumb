#!/usr/bin/env bash
# SessionStart hook — print a short repo-state summary so Claude can
# pick up where the last session left off without re-running `git status`.

set -euo pipefail

cd "$(git rev-parse --show-toplevel 2>/dev/null || echo .)"

branch="$(git branch --show-current 2>/dev/null || echo '(detached)')"
ahead="$(git rev-list --count origin/main..HEAD 2>/dev/null || echo '?')"
behind="$(git rev-list --count HEAD..origin/main 2>/dev/null || echo '?')"
dirty="$(git status --porcelain 2>/dev/null | wc -l | tr -d ' ')"
runs="$(ls -1 .agents/runs 2>/dev/null | grep -v '^README.md$' | wc -l | tr -d ' ')"

cat <<EOF
Plumb context:
- Branch: $branch (ahead $ahead, behind $behind vs origin/main)
- Uncommitted files: $dirty
- Active agent runs: $runs
- Read order: /AGENTS.md → docs/local/prd.md → .agents/rules/ → .agents/skills/
EOF
