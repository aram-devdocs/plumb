#!/usr/bin/env python3
"""
Local PR review helper for the Plumb repository.

Generates a structured markdown review body that mirrors the GitHub
review workflow's rules.
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
DEFAULT_REPO = "aram-devdocs/plumb"


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
    """Classify changed files by Plumb crate / area."""
    return {
        "core": any(f.startswith("crates/plumb-core/") for f in files),
        "format": any(f.startswith("crates/plumb-format/") for f in files),
        "cdp": any(f.startswith("crates/plumb-cdp/") for f in files),
        "config": any(f.startswith("crates/plumb-config/") for f in files),
        "mcp": any(f.startswith("crates/plumb-mcp/") for f in files),
        "cli": any(f.startswith("crates/plumb-cli/") for f in files),
        "xtask": any(f.startswith("xtask/") for f in files),
        "docs": any(f.startswith("docs/") for f in files),
        "ci": any(f.startswith(".github/") or f == "lefthook.yml" or f == "justfile" for f in files),
        "deps": any(f.endswith("Cargo.toml") or f == "Cargo.lock" or f == "deny.toml" for f in files),
        "rules": any(f.startswith("crates/plumb-core/src/rules/") for f in files),
        "mcp_tools": any("crates/plumb-mcp/src/lib.rs" == f for f in files),
        "schema": any(f == "schemas/plumb.toml.json" or f.startswith("schemas/") for f in files),
    }


def detect_findings(
    diff_text: str,
    files: list[str],
    categories: dict[str, bool],
) -> tuple[list[Finding], list[Finding], list[tuple[str, str, str]]]:
    blockers: list[Finding] = []
    warnings: list[Finding] = []
    anti_patterns: list[tuple[str, str, str]] = []
    added_lines = added_lines_from_diff(diff_text)

    # Blocker regex patterns against ADDED LINES in .rs files.
    #
    # Each entry: (regex, severity, issue-label, suggestion, predicate_on_file)
    blocker_patterns: list[tuple[str, str, str, str, callable]] = [
        (
            r"^\s*unsafe\s*(\{|fn\b)",
            "P0",
            "New `unsafe` block",
            "Only `plumb-cdp` may use `unsafe`; each block must carry a `// SAFETY:` comment.",
            lambda f: not f.startswith("crates/plumb-cdp/"),
        ),
        (
            r"\.unwrap\(\)|\.expect\(",
            "P0",
            "`unwrap` / `expect` in library crate",
            "Return `Result<_, E>` with a `thiserror`-derived variant. `anyhow` and `expect` are only permitted in `plumb-cli::main` / tests.",
            lambda f: f.startswith("crates/plumb-")
            and not f.startswith("crates/plumb-cli/")
            and "/tests/" not in f,
        ),
        (
            r"\bprintln!\(|\beprintln!\(",
            "P0",
            "`println!`/`eprintln!` outside plumb-cli",
            "Only `plumb-cli` may print to stdout/stderr. Use `tracing` macros elsewhere.",
            lambda f: f.startswith("crates/plumb-")
            and not f.startswith("crates/plumb-cli/")
            and "/tests/" not in f,
        ),
        (
            r"\btodo!\(|\bunimplemented!\(|\bdbg!\(",
            "P0",
            "`todo!`/`unimplemented!`/`dbg!` macro",
            "Remove before merge. Open a tracking issue and return a typed error instead.",
            lambda f: f.endswith(".rs"),
        ),
        (
            r"SystemTime::now|Instant::now",
            "P0",
            "Wall-clock call in plumb-core",
            "Forbidden by `clippy::disallowed-methods`. Replace with a content-hashed derivation or move the call to `plumb-cli`.",
            lambda f: f.startswith("crates/plumb-core/"),
        ),
        (
            r"\bHashMap<|\bHashSet<",
            "P1",
            "`HashMap`/`HashSet` may leak nondeterminism",
            "Use `IndexMap`/`IndexSet` when iteration order is observable.",
            lambda f: f.startswith("crates/plumb-core/"),
        ),
        (
            r"panic!\(",
            "P0",
            "`panic!` in library crate",
            "Return a typed error instead; `panic!` is denied workspace-wide.",
            lambda f: f.startswith("crates/plumb-")
            and not f.startswith("crates/plumb-cli/")
            and "/tests/" not in f,
        ),
    ]

    warning_patterns: list[tuple[str, str, str, str, callable]] = [
        (
            r"^\s*(?://|/\*+|\*)\s*TODO\b(?!.*#\d+)",
            "P2",
            "TODO without issue reference",
            "Reference a tracking issue (`TODO(#42): …`) or remove the TODO.",
            lambda f: f.endswith(".rs"),
        ),
        (
            r"#\[allow\(.+\)\]",
            "P2",
            "Local `#[allow(...)]` without rationale comment",
            "Every suppression needs a one-line comment explaining why it's safe.",
            lambda f: f.endswith(".rs"),
        ),
        (
            r"\b(?:delve|tapestry|landscape|leverage|robust|seamless|comprehensive)\b",
            "P3",
            "AI-flavored wording in docs",
            "Rewrite in plain technical language — run the humanizer skill.",
            lambda f: f.endswith((".md", ".mdx", ".txt")) and f.startswith("docs/"),
        ),
        (
            r"\bIn conclusion\b|\bIt's important to note\b|\bDive in\b",
            "P3",
            "Stilted documentation phrasing",
            "Shorten; run the humanizer skill.",
            lambda f: f.endswith((".md", ".mdx", ".txt")),
        ),
    ]

    for file, line, content in added_lines:
        for pattern, severity, issue, suggestion, predicate in blocker_patterns:
            if not predicate(file):
                continue
            if re.search(pattern, content):
                blockers.append(Finding(severity, file or "-", line, issue, suggestion))
        for pattern, severity, issue, suggestion, predicate in warning_patterns:
            if not predicate(file):
                continue
            flags = re.IGNORECASE if "AI-flavored" in issue or "Stilted" in issue else 0
            if re.search(pattern, content, flags=flags):
                warnings.append(Finding(severity, file or "-", line, issue, suggestion))

    # Rule / MCP tool / schema contract checks.
    if categories["rules"]:
        # New rule? Expect a docs page and golden test in the same diff.
        rule_files = [f for f in files if f.startswith("crates/plumb-core/src/rules/") and f.endswith(".rs")]
        new_rule_files = [f for f in rule_files if "placeholder" not in f and "mod.rs" not in f]
        for rf in new_rule_files:
            slug = Path(rf).stem
            doc_expected = f"docs/src/rules/"
            if not any(f.startswith(doc_expected) and slug.replace("_", "-") in f for f in files):
                blockers.append(
                    Finding(
                        "P0",
                        rf,
                        "-",
                        "Rule added without docs/src/rules/<slug>.md",
                        "Every new rule needs a docs page — `cargo xtask sync-rules-index` enforces this pre-release.",
                    )
                )
            if not any("tests/golden_" in f and slug.replace("_", "-") in f for f in files):
                warnings.append(
                    Finding(
                        "P1",
                        rf,
                        "-",
                        "Rule added without a golden test",
                        "Add `crates/plumb-core/tests/golden_<slug>.rs` with an insta snapshot.",
                    )
                )

    if categories["mcp_tools"] and not any("crates/plumb-cli/tests/mcp_stdio.rs" in f for f in files):
        warnings.append(
            Finding(
                "P1",
                "crates/plumb-mcp/src/lib.rs",
                "-",
                "MCP tool change without protocol test update",
                "Add a case to `crates/plumb-cli/tests/mcp_stdio.rs` that exercises the new tool.",
            )
        )

    if categories["schema"] and not any(f == "schemas/plumb.toml.json" for f in files):
        # Config changed but schema blob not regenerated.
        if any(f.startswith("crates/plumb-config/") for f in files):
            blockers.append(
                Finding(
                    "P0",
                    "crates/plumb-config/",
                    "-",
                    "Config shape changed without schema regeneration",
                    "Run `cargo xtask schema` and commit `schemas/plumb.toml.json`.",
                )
            )

    # Anti-pattern summary for the review table.
    anti_patterns.extend(
        [
            (
                "New `unsafe` outside plumb-cdp",
                "Fail" if any(f.issue == "New `unsafe` block" for f in blockers) else "Pass",
                "Scan of added lines against crate layer.",
            ),
            (
                "`unwrap`/`expect` in library crates",
                "Fail" if any("unwrap" in f.issue for f in blockers) else "Pass",
                "Scan of added lines.",
            ),
            (
                "`println!`/`eprintln!` outside plumb-cli",
                "Fail" if any("println" in f.issue for f in blockers) else "Pass",
                "Scan of added lines.",
            ),
            (
                "Wall-clock in plumb-core",
                "Fail" if any("Wall-clock" in f.issue for f in blockers) else "Pass",
                "`clippy::disallowed-methods` also enforces this.",
            ),
            (
                "`todo!`/`unimplemented!`/`dbg!`",
                "Fail" if any("todo!" in f.issue for f in blockers) else "Pass",
                "Scan of added lines.",
            ),
            (
                "AI-flavored wording in docs",
                "Warn" if any("AI-flavored" in f.issue or "Stilted" in f.issue for f in warnings) else "Pass",
                "Language scan of added doc lines.",
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
        "- This draft mirrors the GitHub review workflow structure; high-risk files still need human inspection before posting.",
    ]
    if instructions:
        notes.append(f"- Extra reviewer focus: {instructions}.")
    if categories["cdp"] or categories["mcp"]:
        notes.append("- `plumb-cdp` / `plumb-mcp` changes present — run security-auditor in parallel.")
    if categories["deps"]:
        notes.append("- Dependency graph changed — confirm `cargo deny check` and `cargo audit` pass.")
    if categories["docs"]:
        notes.append("- Docs changed — run the humanizer skill before approving.")
    if categories["rules"]:
        notes.append("- Rule definition touched — confirm `docs/src/rules/` and golden test accompany the change; `cargo xtask sync-rules-index` must pass.")
    return "\n".join(notes)


def review_verdict(blockers: list[Finding], warnings: list[Finding]) -> str:
    if blockers:
        return "BLOCK" if any(f.severity == "P0" for f in blockers) else "REQUEST_CHANGES"
    if warnings:
        return "REQUEST_CHANGES"
    return "APPROVE"


def compliance_flags(categories: dict[str, bool], blockers: list[Finding]) -> dict[str, str]:
    issues = {finding.issue for finding in blockers}
    def tick(ok: bool) -> str:
        return "x" if ok else " "
    return {
        "LAYER_IMPORTS": tick("New `unsafe` block" not in issues),
        "ERROR_TYPES": tick(not any("unwrap" in i for i in issues)),
        "OUTPUT_DISCIPLINE": tick(not any("println" in i for i in issues)),
        "DETERMINISM": tick(not any("Wall-clock" in i for i in issues) and not any("HashMap" in i for i in issues)),
        "NO_DEBUG_MACROS": tick(not any("todo!" in i for i in issues)),
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
    files = [
        line for line in run(["gh", "pr", "diff", str(pr_number), "--repo", repo, "--name-only"]).splitlines() if line
    ]
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
    parser = argparse.ArgumentParser(description="Generate a local Plumb PR review draft.")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--pr", type=int, help="PR number to review")
    source.add_argument("--local-diff", help="Local git diff range, for example main...HEAD")
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
        "ERROR_TYPES": flags["ERROR_TYPES"],
        "OUTPUT_DISCIPLINE": flags["OUTPUT_DISCIPLINE"],
        "DETERMINISM": flags["DETERMINISM"],
        "NO_DEBUG_MACROS": flags["NO_DEBUG_MACROS"],
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
