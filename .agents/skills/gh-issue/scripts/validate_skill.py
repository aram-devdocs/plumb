#!/usr/bin/env python3
"""
Validate gh-issue skill structure for Plumb.
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
    "## Issue summary",
    "## Acceptance criteria",
    "## Affected crates",
    "## Implementation approach",
    "## Subagent dispatch plan",
    "## Review gates",
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

STATE_REVIEW_KEYS = ["spec", "quality", "architecture", "test", "security"]

PR_TEMPLATE_SECTIONS = [
    "## Target branch",
    "## Summary",
    "## Spec",
    "## Crates touched",
    "## Test plan",
    "## Breaking change?",
    "## Checklist",
]

# Phrases that must NOT appear — residue from the omnifol port.
FORBIDDEN_PHRASES = [
    "omnifol",
    "omniscript",
    "@omnifol",
    "trpc-procedure",
    "ui-component",
    "hook-query",
    "trading-domain-expert",
    "omniscript-domain-expert",
    "database-migration",
    "pnpm typecheck",
    "pnpm lint",
    "pnpm --filter",
    "--base dev",
    "target dev",
    "git checkout dev",
]


def check_files(errors: list[str]) -> None:
    for path in REQUIRED_FILES:
        if not path.exists():
            errors.append(f"missing required file: {path.relative_to(WORKSPACE_ROOT)}")


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
        "aram-devdocs/plumb",
        "01-implementer",
        "02-spec-reviewer",
        "03-code-quality-reviewer",
        "04-test-runner",
        "05-architecture-validator",
        "06-security-auditor",
        "just validate",
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


def check_forbidden_phrases(errors: list[str]) -> None:
    # Skip this validator itself — FORBIDDEN_PHRASES above literally contains
    # the patterns we're hunting for.
    self_path = Path(__file__).resolve()
    for path in SKILL_ROOT.rglob("*"):
        if not path.is_file():
            continue
        if path.resolve() == self_path:
            continue
        if path.suffix not in {".md", ".py", ".json", ".yml", ".yaml"}:
            continue
        try:
            content = path.read_text()
        except UnicodeDecodeError:
            continue
        for phrase in FORBIDDEN_PHRASES:
            if phrase.lower() in content.lower():
                errors.append(
                    f"omnifol residue: {path.relative_to(WORKSPACE_ROOT)} contains '{phrase}'"
                )


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
    check_forbidden_phrases(errors)
    check_script_help(errors)

    if errors:
        print("VALIDATION FAILED:")
        for error in errors:
            print(f"  - {error}")
        sys.exit(1)

    print("OK: gh-issue skill structure is valid")


if __name__ == "__main__":
    main()
