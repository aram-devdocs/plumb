#!/usr/bin/env python3
"""
Validate gh-runbook skill structure for Plumb.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

SKILL_ROOT = Path(__file__).resolve().parents[1]
WORKSPACE_ROOT = SKILL_ROOT.parents[2]
SCHEMA_PATH = WORKSPACE_ROOT / "schemas" / "runbook-spec.json"

REQUIRED_FILES = [
    SKILL_ROOT / "SKILL.md",
    SKILL_ROOT / "assets/child-issue-template.md",
    SKILL_ROOT / "assets/parent-issue-template.md",
    SKILL_ROOT / "references/spec-format.md",
    SKILL_ROOT / "scripts/generate_runbook.py",
    SKILL_ROOT / "scripts/validate_skill.py",
    SCHEMA_PATH,
]

PARENT_TEMPLATE_TOKENS = [
    "{{SUMMARY}}",
    "{{ACCEPTANCE_CRITERIA}}",
    "{{BATCHES}}",
    "{{PHASE_GATE_CRITERION}}",
    "{{UNBLOCKS}}",
]

CHILD_TEMPLATE_TOKENS = [
    "{{SUMMARY}}",
    "{{CRATE}}",
    "{{BATCH_ID}}",
    "{{EFFORT}}",
    "{{PRD_REFS}}",
    "{{ACCEPTANCE_CRITERIA}}",
    "{{DEPENDENCIES}}",
    "{{REVIEWERS}}",
    "{{PARENT_ISSUE}}",
]

REQUIRED_SKILL_PHRASES = [
    "spec-format.md",
    "--dry-run",
    "manifest.json",
    "create-issues.sh",
    "aram-devdocs/plumb",
    "runbook-spec.json",
]

FORBIDDEN_PHRASES = [
    "omnifol",
    "omniscript",
    "@omnifol",
    "trpc",
    "pnpm",
    "aram-devdocs/omnifol",
    "workstream-topology.md",
]


def main() -> None:
    errors: list[str] = []

    for path in REQUIRED_FILES:
        if not path.exists():
            errors.append(f"missing required file: {path.relative_to(WORKSPACE_ROOT)}")

    for template_path, tokens in [
        (SKILL_ROOT / "assets/parent-issue-template.md", PARENT_TEMPLATE_TOKENS),
        (SKILL_ROOT / "assets/child-issue-template.md", CHILD_TEMPLATE_TOKENS),
    ]:
        if not template_path.exists():
            continue
        content = template_path.read_text()
        for token in tokens:
            if token not in content:
                errors.append(f"{template_path.name} missing token: {token}")

    if (SKILL_ROOT / "SKILL.md").exists():
        content = (SKILL_ROOT / "SKILL.md").read_text()
        for phrase in REQUIRED_SKILL_PHRASES:
            if phrase not in content:
                errors.append(f"SKILL.md missing phrase: {phrase}")

    if SCHEMA_PATH.exists():
        try:
            schema = json.loads(SCHEMA_PATH.read_text())
        except json.JSONDecodeError as exc:
            errors.append(f"runbook-spec.json invalid JSON: {exc}")
        else:
            if schema.get("$id") != "https://plumb.aramhammoudeh.com/schemas/runbook-spec.json":
                errors.append("runbook-spec.json $id mismatch")

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
                    f"residue: {path.relative_to(WORKSPACE_ROOT)} contains '{phrase}'"
                )

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
