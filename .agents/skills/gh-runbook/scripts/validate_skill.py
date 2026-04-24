#!/usr/bin/env python3
"""
Validate gh-runbook skill structure.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

SKILL_ROOT = Path(__file__).resolve().parents[1]
REQUIRED_FILES = [
    SKILL_ROOT / "SKILL.md",
    SKILL_ROOT / "assets/child-issue-template.md",
    SKILL_ROOT / "assets/parent-issue-template.md",
    SKILL_ROOT / "references/workstream-topology.md",
    SKILL_ROOT / "scripts/generate_runbook.py",
    SKILL_ROOT / "scripts/validate_skill.py",
]
REQUIRED_BODY_SECTIONS = [
    "## Summary",
    "## Acceptance Criteria",
    "## Dependencies",
    "## Effort Estimate",
    "## Implementation Notes",
]
REQUIRED_PHRASES = [
    "structured-task.yml",
    "--dry-run",
    "manifest.json",
    "create-issues.sh",
    "workstream-topology.md",
]


def main() -> None:
    errors: list[str] = []
    for path in REQUIRED_FILES:
        if not path.exists():
            errors.append(f"missing required file: {path}")

    for template_path in [
        SKILL_ROOT / "assets/child-issue-template.md",
        SKILL_ROOT / "assets/parent-issue-template.md",
    ]:
        if not template_path.exists():
            continue
        content = template_path.read_text()
        for section in REQUIRED_BODY_SECTIONS:
            if section not in content:
                errors.append(f"{template_path.name} missing section: {section}")

    if (SKILL_ROOT / "SKILL.md").exists():
        content = (SKILL_ROOT / "SKILL.md").read_text()
        for phrase in REQUIRED_PHRASES:
            if phrase not in content:
                errors.append(f"SKILL.md missing phrase: {phrase}")

    try:
        result = subprocess.run(
            ["python3", str(SKILL_ROOT / "scripts/generate_runbook.py"), "--help"],
            text=True,
            capture_output=True,
            check=False,
        )
        if result.returncode != 0:
            errors.append(f"generate_runbook.py --help failed: {result.stderr.strip()}")
    except Exception as exc:
        errors.append(f"failed to invoke generate_runbook.py --help: {exc}")

    if errors:
        print("VALIDATION FAILED:")
        for error in errors:
            print(f"  - {error}")
        sys.exit(1)

    print("OK: gh-runbook skill structure is valid")


if __name__ == "__main__":
    main()
