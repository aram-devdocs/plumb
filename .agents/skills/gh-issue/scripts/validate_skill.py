#!/usr/bin/env python3
"""
Validate gh-issue skill structure.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

SKILL_ROOT = Path(__file__).resolve().parents[1]
WORKSPACE_ROOT = SKILL_ROOT.parents[2]

REQUIRED_FILES = [
    SKILL_ROOT / "SKILL.md",
    SKILL_ROOT / "scripts/gh_issue_run.py",
    SKILL_ROOT / "scripts/validate_skill.py",
    SKILL_ROOT / "assets/plan-template.md",
    SKILL_ROOT / "assets/pr-body-template.md",
    SKILL_ROOT / "assets/state-template.json",
    SKILL_ROOT / "assets/prompts/lead-dispatch.md",
    SKILL_ROOT / "assets/prompts/review-dispatch.md",
    SKILL_ROOT / "assets/prompts/pr-creation.md",
    SKILL_ROOT / "assets/prompts/feedback-triage.md",
    SKILL_ROOT / "assets/prompts/ci-polling.md",
    SKILL_ROOT / "assets/prompts/cleanup-completion.md",
    SKILL_ROOT / "references/workflow-contract.md",
    SKILL_ROOT / "references/resume-contract.md",
    SKILL_ROOT / "references/evals.md",
]

PLAN_TEMPLATE_SECTIONS = [
    "## Issue Summary",
    "## Acceptance Criteria",
    "## Affected Packages",
    "## Implementation Approach",
    "## Subagent Dispatch Plan",
    "## Review Gates",
    "## Verification",
    "## Branch",
]

STATE_TEMPLATE_KEYS = [
    "primary",
    "issues",
    "slug",
    "phase",
    "branch",
    "pr",
    "commits",
    "reviews",
    "worktree",
    "created",
    "updated",
]

STATE_REVIEW_KEYS = ["spec", "quality", "architecture", "security"]

PR_TEMPLATE_SECTIONS = [
    "## Target Branch",
    "## Type of Change",
    "## Summary",
    "## Related Issues",
    "## Changes",
    "## Affected Layers",
    "## System Impact",
    "## Architectural Compliance",
    "## Testing",
    "## Code Quality",
    "## Documentation",
    "## Breaking Changes",
    "## Deployment Considerations",
    "## Security Implications",
    "## Performance Impact",
    "## Screenshots",
    "## Reviewer Notes",
]


def check_files(errors: list[str]) -> None:
    for path in REQUIRED_FILES:
        if not path.exists():
            errors.append(f"missing required file: {path}")


def check_plan_template(errors: list[str]) -> None:
    path = SKILL_ROOT / "assets/plan-template.md"
    if not path.exists():
        return
    content = path.read_text()
    for section in PLAN_TEMPLATE_SECTIONS:
        if section not in content:
            errors.append(f"plan-template.md missing section: {section}")


def check_state_template(errors: list[str]) -> None:
    path = SKILL_ROOT / "assets/state-template.json"
    if not path.exists():
        return

    try:
        state = json.loads(path.read_text())
    except json.JSONDecodeError as exc:
        errors.append(f"state-template.json invalid JSON: {exc}")
        return

    for key in STATE_TEMPLATE_KEYS:
        if key not in state:
            errors.append(f"state-template.json missing key: {key}")

    reviews = state.get("reviews", {})
    for key in STATE_REVIEW_KEYS:
        if key not in reviews:
            errors.append(f"state-template.json missing reviews.{key}")


def check_skill_md(errors: list[str]) -> None:
    path = SKILL_ROOT / "SKILL.md"
    if not path.exists():
        return

    content = path.read_text()
    required_phrases = [
        "gh_issue_run.py",
        "init-run",
        "update-state",
        "validate-resume",
        "poll-pr",
        "cleanup-worktree",
        "<primary>-<type>-<slug>",
        "security-auditor",
        "spec-reviewer",
        "code-quality-reviewer",
        "architecture-validator",
        "/gh-review",
        "/gh-runbook",
        "humanizer",
    ]
    for phrase in required_phrases:
        if phrase not in content:
            errors.append(f"SKILL.md missing reference to: {phrase}")


def check_pr_template_asset(errors: list[str]) -> None:
    path = SKILL_ROOT / "assets/pr-body-template.md"
    if not path.exists():
        return

    content = path.read_text()
    for section in PR_TEMPLATE_SECTIONS:
        if section not in content:
            errors.append(f"pr-body-template.md missing section: {section}")


def check_script_help(errors: list[str]) -> None:
    result = subprocess.run(
        ["python3", str(SKILL_ROOT / "scripts/gh_issue_run.py"), "--help"],
        cwd=WORKSPACE_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        errors.append(f"gh_issue_run.py --help failed: {result.stderr.strip()}")


def main() -> None:
    errors: list[str] = []
    check_files(errors)
    check_plan_template(errors)
    check_state_template(errors)
    check_skill_md(errors)
    check_pr_template_asset(errors)
    check_script_help(errors)

    if errors:
        print("VALIDATION FAILED:")
        for error in errors:
            print(f"  - {error}")
        sys.exit(1)

    print("OK: gh-issue skill structure is valid")


if __name__ == "__main__":
    main()
