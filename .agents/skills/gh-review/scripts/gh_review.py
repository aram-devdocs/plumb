#!/usr/bin/env python3
"""
Local PR review helper for the Omnifol repository.

Generates a structured markdown review body that mirrors the GitHub review workflow.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

SKILL_ROOT = Path(__file__).resolve().parents[1]
WORKSPACE_ROOT = SKILL_ROOT.parents[2]
REVIEW_TEMPLATE = SKILL_ROOT / "assets" / "review-template.md"
DEFAULT_REPO = "aram-devdocs/omnifol"


@dataclass
class Finding:
    severity: str
    file: str
    line: str
    issue: str
    suggestion: str


def run(cmd: list[str], *, check: bool = True) -> str:
    result = subprocess.run(
        cmd,
        cwd=WORKSPACE_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if check and result.returncode != 0:
        raise RuntimeError(f"command failed: {' '.join(cmd)}\n{result.stderr.strip()}")
    return result.stdout


def added_lines_from_diff(diff_text: str) -> list[tuple[str, str, str]]:
    current_file = ""
    current_line = 0
    added: list[tuple[str, str, str]] = []
    hunk_re = re.compile(r"@@ -\d+(?:,\d+)? \+(\d+)")

    for raw_line in diff_text.splitlines():
        if raw_line.startswith("diff --git "):
            parts = raw_line.split()
            if len(parts) >= 4:
                current_file = parts[3].removeprefix("b/")
            current_line = 0
            continue

        if raw_line.startswith("@@"):
            match = hunk_re.search(raw_line)
            if match:
                current_line = int(match.group(1))
            continue

        if raw_line.startswith("+") and not raw_line.startswith("+++"):
            added.append((current_file, str(current_line or "?"), raw_line[1:]))
            if current_line:
                current_line += 1
            continue

        if raw_line.startswith("-") and not raw_line.startswith("---"):
            continue

        if current_line:
            current_line += 1

    return added


def classify_files(files: list[str]) -> dict[str, bool]:
    lowered = [file.lower() for file in files]
    return {
        "schema": any("schema.prisma" in file for file in lowered),
        "ui": any(file.endswith(".tsx") or "packages/web/ui/" in file or "apps/web/" in file for file in lowered),
        "api": any("apps/server/src/trpc/" in file for file in lowered),
        "config": any(".env" in file or "config" in file or ".github/workflows/" in file for file in lowered),
        "migration": any("/migrations/" in file or "prisma/migrations/" in file for file in lowered),
        "strategy": any(
            "packages/shared/omniscript/" in file or "packages/backend/strategy-engine/" in file
            for file in lowered
        ),
        "trading": any(
            token in file
            for file in lowered
            for token in ("trading", "orders", "positions", "balances", "exchange")
        ),
    }


def detect_findings(diff_text: str, files: list[str], categories: dict[str, bool]) -> tuple[list[Finding], list[Finding], list[tuple[str, str, str]]]:
    blockers: list[Finding] = []
    warnings: list[Finding] = []
    anti_patterns: list[tuple[str, str, str]] = []
    added_lines = added_lines_from_diff(diff_text)

    blocker_patterns = [
        (
            r"(?:\bas any\b|:\s*any(?:[\s,)\]>;]|$)|<any>|any\[\]|Promise<any>|Record<[^>]+,\s*any>)",
            "P1",
            "`any` type usage",
            "Replace with a precise type or generic.",
        ),
        (
            r"^\s*(?://|/\*+|\*)\s*@ts-ignore|^\s*(?://|/\*+|\*)\s*@ts-expect-error",
            "P0",
            "Type-check suppression directive",
            "Remove the directive and fix the underlying type issue.",
        ),
        (r"\bconsole\.log\(", "P0", "`console.log` in committed code", "Use `@omnifol/logger` instead."),
        (r"<(?:div|span)[^>]*onClick=", "P1", "Non-semantic clickable element", "Use semantic interactive HTML with keyboard support."),
    ]

    warning_patterns = [
        (
            r"^\s*(?://|/\*+|\*)\s*TODO\b(?!.*#\d+)",
            "P2",
            "TODO without issue reference",
            "Reference a tracking issue or remove the TODO.",
        ),
        (r"\b(?:delve|tapestry|landscape|leverage|robust)\b", "P3", "AI-flavored wording", "Rewrite in plain technical language."),
        (r"\bIn conclusion\b|\bIt's important to note\b", "P3", "Stilted documentation phrasing", "Shorten and rewrite in direct language."),
    ]

    for file, line, content in added_lines:
        is_code = file.endswith((".ts", ".tsx", ".js", ".jsx"))
        is_docs = file.endswith((".md", ".mdx", ".txt"))
        for pattern, severity, issue, suggestion in blocker_patterns:
            if not is_code and issue != "Non-semantic clickable element":
                continue
            if re.search(pattern, content):
                blockers.append(Finding(severity, file or "-", line, issue, suggestion))
        for pattern, severity, issue, suggestion in warning_patterns:
            if issue == "TODO without issue reference" and not is_code:
                continue
            if issue in {"AI-flavored wording", "Stilted documentation phrasing"} and not is_docs:
                continue
            if re.search(pattern, content, flags=re.IGNORECASE):
                warnings.append(Finding(severity, file or "-", line, issue, suggestion))

    if categories["schema"] and not categories["migration"]:
        blockers.append(
            Finding(
                "P0",
                "schema.prisma",
                "-",
                "Schema change without migration files",
                "Add and commit the matching migration.",
            )
        )

    if any(file.startswith("apps/server/src/trpc/") for file in files):
        anti_patterns.append(("Business logic in tRPC procedures", "Manual check", "Review changed procedures for service delegation."))
    else:
        anti_patterns.append(("Business logic in tRPC procedures", "N/A", "No tRPC files changed."))

    anti_patterns.extend(
        [
            ("`any` type usage", "Fail" if any(f.issue == "`any` type usage" for f in blockers) else "Pass", "Diff scan on added lines."),
            (
                "`console.log`",
                "Fail" if any(f.issue == "`console.log` in committed code" for f in blockers) else "Pass",
                "Diff scan on added lines.",
            ),
            (
                "Type-check suppressions",
                "Fail" if any(f.issue == "Type-check suppression directive" for f in blockers) else "Pass",
                "Diff scan on added lines.",
            ),
            (
                "Non-semantic click targets",
                "Fail" if any(f.issue == "Non-semantic clickable element" for f in blockers) else "Pass",
                "Diff scan on added lines.",
            ),
            (
                "AI-written or stilted docs",
                "Warn" if any(f.issue in {"AI-flavored wording", "Stilted documentation phrasing"} for f in warnings) else "Pass",
                "Language scan on added lines.",
            ),
        ]
    )

    return dedupe_findings(blockers), dedupe_findings(warnings), anti_patterns


def dedupe_findings(findings: list[Finding]) -> list[Finding]:
    seen: set[tuple[str, str, str, str]] = set()
    result: list[Finding] = []
    for finding in findings:
        key = (finding.severity, finding.file, finding.line, finding.issue)
        if key in seen:
            continue
        seen.add(key)
        result.append(finding)
    return result


def render_table_rows(findings: list[Finding]) -> str:
    if not findings:
        return "| 1 | - | - | - | None found | - |"
    lines = []
    for index, finding in enumerate(findings, start=1):
        lines.append(
            f"| {index} | {finding.severity} | `{finding.file}` | {finding.line} | {finding.issue} | {finding.suggestion} |"
        )
    return "\n".join(lines)


def render_anti_patterns(rows: list[tuple[str, str, str]]) -> str:
    return "\n".join(f"| {pattern} | {status} | {details} |" for pattern, status, details in rows)


def build_quality_assessment(
    files: list[str],
    categories: dict[str, bool],
    blockers: list[Finding],
    warnings: list[Finding],
    instructions: str | None,
) -> str:
    category_names = [name for name, enabled in categories.items() if enabled]
    notes = [
        f"- Reviewed {len(files)} changed file(s) across: {', '.join(category_names) if category_names else 'uncategorized changes'}.",
        f"- Blocking findings: {len(blockers)}. Warning findings: {len(warnings)}.",
        "- This draft mirrors the workflow structure; high-risk files still need human inspection before posting.",
    ]
    if instructions:
        notes.append(f"- Extra reviewer focus: {instructions}.")
    if categories["trading"] or categories["api"] or categories["schema"]:
        notes.append("- Financial, API, or schema-touching work is present; verify security and migration handling explicitly.")
    if categories["ui"]:
        notes.append("- UI work is present; confirm semantic HTML, keyboard support, and smallest-breakpoint behavior manually.")
    return "\n".join(notes)


def review_verdict(blockers: list[Finding], warnings: list[Finding]) -> str:
    if blockers:
        return "CHANGES REQUESTED"
    if warnings:
        return "NEEDS DISCUSSION"
    return "APPROVED"


def compliance_flags(categories: dict[str, bool], blockers: list[Finding]) -> dict[str, str]:
    issues = {finding.issue for finding in blockers}
    return {
        "LAYER_IMPORTS": " " if not categories["api"] else " ",
        "BOUNDARIES": " " if not categories["ui"] else " ",
        "BUSINESS_LOGIC": " " if "Business logic in tRPC procedures" not in issues else "x",
        "DATABASE_ACCESS": " " if not categories["schema"] else " ",
        "TYPES": " " if "`any` type usage" in issues else "x",
    }


def template_for_review(context: dict[str, str]) -> str:
    template = REVIEW_TEMPLATE.read_text()
    for key, value in context.items():
        template = template.replace(f"{{{{{key}}}}}", value)
    return template


def gather_from_pr(pr_number: int, repo: str) -> tuple[dict[str, str], list[str], str]:
    metadata = json.loads(
        run(
            [
                "gh",
                "pr",
                "view",
                str(pr_number),
                "--repo",
                repo,
                "--json",
                "number,title,author,baseRefName,headRefName,body",
            ]
        )
    )
    files = [line for line in run(["gh", "pr", "diff", str(pr_number), "--repo", repo, "--name-only"]).splitlines() if line]
    diff_text = run(["gh", "pr", "diff", str(pr_number), "--repo", repo])
    header = {
        "PR": f"#{metadata['number']} - {metadata['title']}",
        "AUTHOR": f"@{metadata['author']['login']}",
        "BASE": f"{metadata['baseRefName']} <- {metadata['headRefName']}",
        "DESCRIPTION": metadata.get("body") or "",
    }
    return header, files, diff_text


def gather_from_local_diff(diff_range: str) -> tuple[dict[str, str], list[str], str]:
    files = [line for line in run(["git", "diff", "--name-only", diff_range]).splitlines() if line]
    diff_text = run(["git", "diff", diff_range])
    header = {
        "PR": f"local diff `{diff_range}`",
        "AUTHOR": "@local",
        "BASE": diff_range,
        "DESCRIPTION": "",
    }
    return header, files, diff_text


def write_output(text: str, output: str | None) -> None:
    if output:
        Path(output).write_text(text)
    else:
        print(text)


def post_review(pr_number: int, repo: str, body: str) -> None:
    run(["gh", "pr", "comment", str(pr_number), "--repo", repo, "--body", body])


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate a local Omnifol PR review draft.")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--pr", type=int, help="PR number to review")
    source.add_argument("--local-diff", help="Local git diff range, for example dev...HEAD")
    parser.add_argument("--repo", default=DEFAULT_REPO, help="GitHub repository in owner/name format")
    parser.add_argument("--instructions", help="Additional reviewer instructions")
    parser.add_argument("--output", help="Write markdown output to a file")
    parser.add_argument("--post", action="store_true", help="Post the generated review to the PR")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.pr:
        header, files, diff_text = gather_from_pr(args.pr, args.repo)
    else:
        header, files, diff_text = gather_from_local_diff(args.local_diff)

    categories = classify_files(files)
    blockers, warnings, anti_patterns = detect_findings(diff_text, files, categories)
    verdict = review_verdict(blockers, warnings)
    flags = compliance_flags(categories, blockers)

    context = {
        **header,
        "BLOCKERS": render_table_rows(blockers),
        "WARNINGS": render_table_rows(warnings),
        "ANTI_PATTERNS": render_anti_patterns(anti_patterns),
        "QUALITY_ASSESSMENT": build_quality_assessment(files, categories, blockers, warnings, args.instructions),
        "MATCHES_DESCRIPTION": "Manual check required" if header["DESCRIPTION"] else "Unknown / local diff",
        "SCOPE_CREEP": "Manual check required" if files else "No files changed",
        "VERDICT": verdict,
        "LAYER_IMPORTS": flags["LAYER_IMPORTS"],
        "BOUNDARIES": flags["BOUNDARIES"],
        "BUSINESS_LOGIC": flags["BUSINESS_LOGIC"],
        "DATABASE_ACCESS": flags["DATABASE_ACCESS"],
        "TYPES": flags["TYPES"],
    }

    body = template_for_review(context)
    write_output(body, args.output)

    if args.post:
        if not args.pr:
            raise RuntimeError("--post requires --pr")
        post_review(args.pr, args.repo, body)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        sys.exit(1)
