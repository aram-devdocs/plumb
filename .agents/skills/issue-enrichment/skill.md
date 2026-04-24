---
name: issue-enrichment
description: Orchestrates bulk GitHub issue enrichment. Fetches issues, dispatches the github-issue-enricher agent for analysis, collects enrichment plans, and generates a gh CLI script for applying changes.
user_invocable: true
---

# Issue Enrichment Skill

Bulk-enrich GitHub issues with structured metadata: milestones, effort labels, dependency annotations, acceptance criteria, and structured bodies.

## Usage

- `/issue-enrichment` - Enrich all open issues
- `/issue-enrichment #375` - Enrich a single issue
- `/issue-enrichment #375-#450` - Enrich a range of issues

## How It Works

### Step 1: Fetch Issues

```bash
gh issue list --repo aram-devdocs/omnifol --state open --limit 500 --json number,title,body,labels,milestone,assignees
```

If a range is specified (e.g., `#375-#450`), filter to that range.
If a single issue is specified (e.g., `#375`), fetch just that issue.

### Step 2: Analyze Each Issue

For each issue, dispatch the `github-issue-enricher` agent with:
- Issue number, title, body, current labels, milestone, assignees
- Request: produce an enrichment plan JSON

Process issues in batches of 4 (parallel agent dispatches per batch).

### Step 3: Collect Enrichment Plans

Aggregate all enrichment plans into a combined report. Display summary:
- Issues analyzed: N
- Milestones assigned: N
- Dependencies found: N
- Effort labels assigned: N

### Step 4: Generate Enrichment Script

Generate a `scripts/enrich-issues.sh` bash script that applies all enrichments via `gh` CLI:

```bash
#!/usr/bin/env bash
set -euo pipefail

OWNER="aram-devdocs"
REPO="omnifol"

# Create effort labels (idempotent)
gh label create "effort-xs" --repo "$OWNER/$REPO" --color "bfdbfe" --description "Effort: <1 hour" 2>/dev/null || true
gh label create "effort-s"  --repo "$OWNER/$REPO" --color "93c5fd" --description "Effort: 1-4 hours" 2>/dev/null || true
gh label create "effort-m"  --repo "$OWNER/$REPO" --color "60a5fa" --description "Effort: 4-8 hours" 2>/dev/null || true
gh label create "effort-l"  --repo "$OWNER/$REPO" --color "3b82f6" --description "Effort: 1-3 days" 2>/dev/null || true
gh label create "effort-xl" --repo "$OWNER/$REPO" --color "2563eb" --description "Effort: 3+ days" 2>/dev/null || true

# Per-issue enrichment
gh issue edit N --repo "$OWNER/$REPO" --milestone "milestone_title"
gh issue edit N --repo "$OWNER/$REPO" --add-label "effort-m"
gh issue edit N --repo "$OWNER/$REPO" --body "structured body..."
```

### Step 5: Present for Approval

Display the generated script to the user. Do NOT execute automatically.

The user reviews and either:
- Approves execution (run the script)
- Requests modifications
- Saves the script for later

## When to Use

- **Bulk enrichment**: After creating a batch of issues that need structured metadata
- **New issue triage**: When new issues are created without full metadata
- **Pre-sprint planning**: Before starting a sprint, enrich all issues in scope
- **Data cleanup**: When the orchestration dashboard shows issues without milestones or dependencies

## Safety

- Always presents the enrichment script for user approval before execution
- Never auto-applies changes to GitHub issues
- Preserves original issue body content in an "Original Description" section
- Idempotent: skips issues that already have the target milestone/labels/body structure
- Uses `2>/dev/null || true` for label creation to handle already-exists gracefully
