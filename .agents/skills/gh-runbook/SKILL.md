---
name: gh-runbook
description: Convert an Omnifol audit or research report into a parent runbook issue, grouped workstream issue drafts, a manifest, and a dry-run gh CLI creation script. Uses the repo structured task format and reuses issue-enrichment conventions for acceptance criteria, dependencies, effort, and milestone suggestions.
user_invocable: true
---

# /gh-runbook - Audit to Issue Runbook

Turn a report into a release runbook with grouped GitHub issues instead of one issue per finding.

## Usage

```bash
/gh-runbook reports/ui-audit/2026-04-23/omnifol-release-ui-ux-audit.md --dry-run
/gh-runbook /absolute/path/to/report.md --dry-run
```

## Workflow

1. Read the source report.
2. Read `.github/ISSUE_TEMPLATE/structured-task.yml`.
3. Reuse Omnifol issue-enrichment conventions:
   - acceptance criteria
   - dependency notes
   - effort labels
   - milestone suggestions
4. Generate drafts with:
   ```bash
   python3 .agents/skills/gh-runbook/scripts/generate_runbook.py <report> --dry-run --output-dir /tmp/gh-runbook
   ```
5. Review the output:
   - `manifest.json`
   - `runbook-summary.md`
   - parent issue draft
   - child issue drafts
   - `create-issues.sh`
6. Present the drafts and script to the user. Do not create issues without approval.

## Default topology

The first-pass Omnifol audit uses grouped workstreams defined in `references/workstream-topology.md`:

1. parent release-readiness runbook
2. trading / export correctness
3. trading safety and order-entry gating
4. DLQ / schema incident resolution
5. admin orchestration health or hide
6. onboarding / help / what's-new consolidation
7. feedback / support / bug-report consolidation
8. dashboard / analytics scope rationalization
9. auth invite / token fallback states
10. Storybook runtime gate and page-state coverage
11. launch-scope cleanup and admin IA follow-up

## Rules

- Dry run is the default; only generate drafts and scripts.
- Group findings into workstreams with clear ownership and dependencies.
- Use the structured issue body shape from `.github/ISSUE_TEMPLATE/structured-task.yml`.
- Prefer existing labels and milestones when they already exist in GitHub.
- If the report cites prior issues or PRs, note whether the new issue is follow-up work or a net-new tracking item.
- Keep generated output outside git unless the user explicitly asks to save it in the repo.
