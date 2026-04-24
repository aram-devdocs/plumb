#!/usr/bin/env python3
"""
Validate gh-review skill structure for Plumb.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

SKILL_ROOT = Path(__file__).resolve().parents[1]
WORKSPACE_ROOT = SKILL_ROOT.parents[2]

REQUIRED_FILES = [
    SKILL_ROOT / "SKILL.md",
    SKILL_ROOT / "assets/review-template.md",
    SKILL_ROOT / "references/workflow-contract.md",
    SKILL_ROOT / "scripts/gh_review.py",
    SKILL_ROOT / "scripts/validate_skill.py",
]

REQUIRED_TEMPLATE_SECTIONS = [
    "### Code review summary",
    "#### Blockers",
    "#### Warnings",
    "#### Architecture compliance",
    "#### Anti-pattern scan",
    "#### Quality assessment",
    "#### Scope check",
    "**Verdict:** {{VERDICT}}",
]

REQUIRED_SKILL_PHRASES = [
    "claude-code-review.yml",
    "--pr",
    "--local-diff",
    "aram-devdocs/plumb",
    "APPROVE",
    "REQUEST_CHANGES",
    "BLOCK",
    "plumb-cdp",
    "plumb-mcp",
]

FORBIDDEN_PHRASES = [
    "omnifol",
    "omniscript",
    "@omnifol",
    "trpc",
    "pnpm",
    "--base dev",
    "target dev",
    "git checkout dev",
]


def main() -> None:
    errors: list[str] = []

    for path in REQUIRED_FILES:
        if not path.exists():
            errors.append(f"missing required file: {path.relative_to(WORKSPACE_ROOT)}")

    template_path = SKILL_ROOT / "assets/review-template.md"
    if template_path.exists():
        content = template_path.read_text()
        for section in REQUIRED_TEMPLATE_SECTIONS:
            if section not in content:
                errors.append(f"review-template.md missing section: {section}")

    skill_path = SKILL_ROOT / "SKILL.md"
    if skill_path.exists():
        content = skill_path.read_text()
        for phrase in REQUIRED_SKILL_PHRASES:
            if phrase not in content:
                errors.append(f"SKILL.md missing phrase: {phrase}")

    # Forbidden-phrase scan across skill files. Skip this validator itself.
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

    try:
        result = subprocess.run(
            ["python3", str(SKILL_ROOT / "scripts/gh_review.py"), "--help"],
            text=True,
            capture_output=True,
            check=False,
        )
        if result.returncode != 0:
            errors.append(f"gh_review.py --help failed: {result.stderr.strip()}")
    except Exception as exc:
        errors.append(f"failed to invoke gh_review.py --help: {exc}")

    if errors:
        print("VALIDATION FAILED:")
        for error in errors:
            print(f"  - {error}")
        sys.exit(1)

    print("OK: gh-review skill structure is valid")


if __name__ == "__main__":
    main()
