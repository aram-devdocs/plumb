#!/usr/bin/env python3
"""
Validate gh-review skill structure.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

SKILL_ROOT = Path(__file__).resolve().parents[1]
REQUIRED_FILES = [
    SKILL_ROOT / "SKILL.md",
    SKILL_ROOT / "assets/review-template.md",
    SKILL_ROOT / "references/workflow-contract.md",
    SKILL_ROOT / "scripts/gh_review.py",
    SKILL_ROOT / "scripts/validate_skill.py",
]
REQUIRED_SECTIONS = [
    "### Code Review Summary",
    "#### Blockers",
    "#### Warnings",
    "#### Architecture Compliance",
    "#### Anti-Pattern Scan",
    "#### Quality Assessment",
    "#### Scope Check",
    "**Verdict:** {{VERDICT}}",
]
REQUIRED_PHRASES = [
    "claude-code-review.yml",
    "--pr <number>",
    "--local-diff",
    "APPROVED",
    "CHANGES REQUESTED",
    "NEEDS DISCUSSION",
]


def main() -> None:
    errors: list[str] = []
    for path in REQUIRED_FILES:
        if not path.exists():
            errors.append(f"missing required file: {path}")

    if (SKILL_ROOT / "assets/review-template.md").exists():
        content = (SKILL_ROOT / "assets/review-template.md").read_text()
        for section in REQUIRED_SECTIONS:
            if section not in content:
                errors.append(f"review-template.md missing section: {section}")

    if (SKILL_ROOT / "SKILL.md").exists():
        content = (SKILL_ROOT / "SKILL.md").read_text()
        for phrase in REQUIRED_PHRASES:
            if phrase not in content:
                errors.append(f"SKILL.md missing phrase: {phrase}")

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
