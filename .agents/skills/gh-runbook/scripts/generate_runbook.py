#!/usr/bin/env python3
"""
Plumb runbook generator — spec-driven.

Reads a YAML spec under docs/runbooks/*.yaml, validates it against
schemas/runbook-spec.json, and emits parent + child issue markdown
drafts + an idempotent create-issues.sh script.

The generator never hits GitHub. Run the emitted script to create
actual issues.
"""

from __future__ import annotations

import argparse
import json
import re
import shlex
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

import yaml

SKILL_ROOT = Path(__file__).resolve().parents[1]
WORKSPACE_ROOT = SKILL_ROOT.parents[2]
SCHEMA_PATH = WORKSPACE_ROOT / "schemas" / "runbook-spec.json"
PARENT_TEMPLATE_PATH = SKILL_ROOT / "assets" / "parent-issue-template.md"
CHILD_TEMPLATE_PATH = SKILL_ROOT / "assets" / "child-issue-template.md"

CRATE_ROLES = {
    "plumb-core": "rule engine, types, determinism (no internal deps)",
    "plumb-format": "output formatters (pretty, JSON, SARIF, MCP-compact)",
    "plumb-cdp": "Chromium DevTools Protocol driver (only crate allowed `unsafe`)",
    "plumb-config": "figment loader + schemars schema emission",
    "plumb-mcp": "rmcp-based stdio MCP server",
    "plumb-cli": "the `plumb` binary (only crate allowed stdout/stderr + anyhow)",
    "xtask": "developer tooling (schema, pre-release, runbook validation)",
    "docs": "mdBook source, rule docs, ADRs, runbook specs",
    "ci": "GitHub workflows, lefthook, justfile",
    "deps": "dependency graph (Cargo.toml, Cargo.lock, deny.toml)",
    "release": "release artifacts (cargo-dist, release-please, installers)",
}


@dataclass
class Issue:
    slug: str
    title: str
    labels: list[str]
    crate: str | None
    effort: str
    prd_refs: list[str]
    summary: str
    acceptance_criteria: list[str]
    dependencies: list[str]
    reviewers: list[str]
    suggested_delivery: list[str]
    batch_id: str = ""
    batch_description: str = ""
    filename: str = ""


@dataclass
class Batch:
    id: str
    description: str
    parallel: bool
    depends_on_batch: list[str]
    issues: list[Issue]


@dataclass
class Spec:
    schema: str
    name: str
    phase_number: int | None
    repo: str
    parent_title: str
    parent_labels: list[str]
    parent_milestone: str | None
    parent_summary: str
    parent_acceptance: list[str]
    parent_prd_refs: list[str]
    batches: list[Batch]
    phase_gate_criterion: str
    phase_gate_unblocks: str


def now_iso() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def load_spec(path: Path) -> Spec:
    with path.open() as f:
        data = yaml.safe_load(f)
    validate_against_schema(data, path)

    parent = data["parent"]
    batches = []
    for b in data["batches"]:
        dep = b.get("depends_on_batch") or []
        if isinstance(dep, str):
            dep = [dep]
        issues = []
        for i in b["issues"]:
            issues.append(
                Issue(
                    slug=i["slug"],
                    title=i["title"],
                    labels=i["labels"],
                    crate=i.get("crate"),
                    effort=i["effort"],
                    prd_refs=i.get("prd_refs", []),
                    summary=i["summary"].rstrip(),
                    acceptance_criteria=i["acceptance_criteria"],
                    dependencies=i.get("dependencies", []),
                    reviewers=i["reviewers"],
                    suggested_delivery=i.get("suggested_delivery", ["gh-issue"]),
                    batch_id=b["id"],
                    batch_description=b["description"],
                )
            )
        batches.append(
            Batch(
                id=b["id"],
                description=b["description"],
                parallel=b["parallel"],
                depends_on_batch=dep,
                issues=issues,
            )
        )

    gate = data["phase_gate"]
    return Spec(
        schema=data["schema"],
        name=data["name"],
        phase_number=data.get("phase_number"),
        repo=data["repo"],
        parent_title=parent["title"],
        parent_labels=parent["labels"],
        parent_milestone=parent.get("milestone"),
        parent_summary=parent["summary"].rstrip(),
        parent_acceptance=parent["acceptance_criteria"],
        parent_prd_refs=parent.get("related_prd_sections", []),
        batches=batches,
        phase_gate_criterion=gate["criterion"].rstrip(),
        phase_gate_unblocks=gate.get("unblocks", ""),
    )


def validate_against_schema(data: dict, path: Path) -> None:
    try:
        import jsonschema
    except ImportError:
        _structural_validate(data, path)
        return

    with SCHEMA_PATH.open() as f:
        schema = json.load(f)
    try:
        jsonschema.validate(instance=data, schema=schema)
    except jsonschema.ValidationError as exc:
        raise RuntimeError(f"spec {path} failed schema validation: {exc.message}") from exc

    _cross_reference_validate(data, path)


def _structural_validate(data: dict, path: Path) -> None:
    required_top = ["schema", "name", "repo", "parent", "batches", "phase_gate"]
    for key in required_top:
        if key not in data:
            raise RuntimeError(f"spec {path} missing required field: {key}")
    if data["schema"] != "https://plumb.dev/schemas/runbook-spec.json":
        raise RuntimeError(f"spec {path} has wrong schema URL")
    for batch in data["batches"]:
        for i in batch["issues"]:
            required_issue = [
                "slug",
                "title",
                "labels",
                "effort",
                "summary",
                "acceptance_criteria",
                "reviewers",
            ]
            for key in required_issue:
                if key not in i:
                    raise RuntimeError(
                        f"spec {path} issue missing {key}: {i.get('slug', '?')}"
                    )
    _cross_reference_validate(data, path)


def _cross_reference_validate(data: dict, path: Path) -> None:
    batch_ids = [b["id"] for b in data["batches"]]
    for batch in data["batches"]:
        dep = batch.get("depends_on_batch") or []
        if isinstance(dep, str):
            dep = [dep]
        for d in dep:
            if d not in batch_ids:
                raise RuntimeError(
                    f"spec {path}: batch {batch['id']} depends on unknown batch {d}"
                )

    all_slugs = []
    for batch in data["batches"]:
        for i in batch["issues"]:
            all_slugs.append(i["slug"])
    duplicates = {s for s in all_slugs if all_slugs.count(s) > 1}
    if duplicates:
        raise RuntimeError(f"spec {path}: duplicate slugs: {sorted(duplicates)}")


def md_list(items: list[str], *, checked: bool = False) -> str:
    if not items:
        return "- None."
    prefix = "- [ ]" if checked else "-"
    return "\n".join(f"{prefix} {item}" for item in items)


def _display_spec_path(spec_path: Path) -> str:
    try:
        return str(spec_path.resolve().relative_to(WORKSPACE_ROOT))
    except ValueError:
        return str(spec_path.resolve())


def render_batches_section(spec: Spec) -> str:
    lines = []
    for batch in spec.batches:
        lines.append(f"### Batch `{batch.id}` — {batch.description}")
        lines.append("")
        lines.append(f"- **Parallel:** {'yes' if batch.parallel else 'no'}")
        if batch.depends_on_batch:
            deps = ", ".join(f"`{b}`" for b in batch.depends_on_batch)
            lines.append(f"- **Depends on:** {deps}")
        lines.append("")
        lines.append("Child issues:")
        lines.append(f"{{{{CHILD_ISSUE_LINES_{batch.id}}}}}")
        lines.append("")
    return "\n".join(lines).rstrip()


def render_template(template: str, mapping: dict[str, str]) -> str:
    for key, value in mapping.items():
        template = template.replace(f"{{{{{key}}}}}", value)
    return template


def render_parent_body(spec: Spec, spec_path: Path) -> str:
    template = PARENT_TEMPLATE_PATH.read_text()
    mapping = {
        "SUMMARY": spec.parent_summary,
        "PRD_REFS": md_list([f"PRD {r}" for r in spec.parent_prd_refs])
        if spec.parent_prd_refs
        else "- None.",
        "ACCEPTANCE_CRITERIA": md_list(spec.parent_acceptance, checked=True),
        "BATCHES": render_batches_section(spec),
        "PHASE_GATE_CRITERION": spec.phase_gate_criterion,
        "UNBLOCKS": spec.phase_gate_unblocks or "(terminal phase)",
        "MILESTONE": spec.parent_milestone or "(none)",
        "GENERATED_AT": now_iso(),
        "SPEC_PATH": _display_spec_path(spec_path),
        "REPO": spec.repo,
    }
    return render_template(template, mapping)


def render_child_body(spec: Spec, issue: Issue) -> str:
    template = CHILD_TEMPLATE_PATH.read_text()
    crate = issue.crate or "(multi-crate)"
    crate_role = CRATE_ROLES.get(crate, "see scoped AGENTS.md")
    mapping = {
        "SUMMARY": issue.summary,
        "CRATE": crate,
        "CRATE_ROLE": crate_role,
        "PARENT_ISSUE": "{{PARENT_ISSUE}}",
        "BATCH_ID": issue.batch_id,
        "BATCH_DESCRIPTION": issue.batch_description,
        "EFFORT": issue.effort,
        "PRD_REFS": md_list([f"PRD {r}" for r in issue.prd_refs])
        if issue.prd_refs
        else "- None.",
        "ACCEPTANCE_CRITERIA": md_list(issue.acceptance_criteria, checked=True),
        "DEPENDENCIES": md_list(issue.dependencies)
        if issue.dependencies
        else "- None (first in batch; batch-level deps gate this).",
        "REVIEWERS": md_list([f"`{r}`" for r in issue.reviewers]),
        "SUGGESTED_DELIVERY": ", ".join(issue.suggested_delivery) or "gh-issue",
    }
    return render_template(template, mapping)


def _parent_slug(spec: Spec) -> str:
    if spec.phase_number:
        return f"phase-{spec.phase_number}"
    return re.sub(r"[^a-z0-9]+", "-", spec.name.lower()).strip("-") or "runbook"


def write_outputs(spec: Spec, spec_path: Path, output_dir: Path) -> dict:
    output_dir.mkdir(parents=True, exist_ok=True)

    parent_slug = _parent_slug(spec)
    parent_filename = f"00-parent-{parent_slug}.md"
    (output_dir / parent_filename).write_text(render_parent_body(spec, spec_path))

    children_manifest = []
    sequence = 1
    for batch in spec.batches:
        for issue in batch.issues:
            filename = f"{sequence:02d}-{batch.id}-{issue.slug}.md"
            issue.filename = filename
            (output_dir / filename).write_text(render_child_body(spec, issue))
            children_manifest.append(
                {
                    "slug": issue.slug,
                    "title": issue.title,
                    "labels": issue.labels,
                    "milestone": spec.parent_milestone,
                    "crate": issue.crate,
                    "effort": issue.effort,
                    "prd_refs": issue.prd_refs,
                    "batch": issue.batch_id,
                    "dependencies": issue.dependencies,
                    "reviewers": issue.reviewers,
                    "body_path": filename,
                }
            )
            sequence += 1

    manifest = {
        "generated_at": now_iso(),
        "source_spec": _display_spec_path(spec_path),
        "repo": spec.repo,
        "name": spec.name,
        "phase_number": spec.phase_number,
        "parent": {
            "title": spec.parent_title,
            "labels": spec.parent_labels,
            "milestone": spec.parent_milestone,
            "body_path": parent_filename,
        },
        "phase_gate": {
            "criterion": spec.phase_gate_criterion,
            "unblocks": spec.phase_gate_unblocks,
        },
        "children": children_manifest,
    }
    (output_dir / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")

    lines = [
        f"# {spec.name}",
        "",
        f"- Generated at: `{now_iso()}`",
        f"- Source spec: `{manifest['source_spec']}`",
        f"- Repo: `{spec.repo}`",
        f"- Milestone: `{spec.parent_milestone or '(none)'}`",
        "",
        "## Parent",
        f"- [{spec.parent_title}]({parent_filename})",
        "",
        "## Children by batch",
        "",
    ]
    for batch in spec.batches:
        deps = (
            f" (after {', '.join(batch.depends_on_batch)})"
            if batch.depends_on_batch
            else ""
        )
        lines.append(f"### Batch `{batch.id}` — {batch.description}{deps}")
        for issue in batch.issues:
            lines.append(
                f"- [{issue.title}]({issue.filename}) "
                + f"— `{issue.slug}` · {issue.effort} · `{issue.crate or '-'}`"
            )
        lines.append("")
    lines.append("## Phase gate")
    lines.append("")
    lines.append(spec.phase_gate_criterion)
    lines.append("")
    lines.append(f"**Unblocks:** {spec.phase_gate_unblocks or '(terminal)'}")
    lines.append("")
    lines.append("## Next step")
    lines.append("")
    lines.append("1. Review this summary and the individual issue drafts.")
    lines.append(
        f"2. Create the milestone `{spec.parent_milestone}` in GitHub if it doesn't exist."
    )
    lines.append(
        "3. Run `bash create-issues.sh` from this directory to create the parent + children."
    )
    lines.append("")
    (output_dir / "summary.md").write_text("\n".join(lines))

    script = render_create_issues_script(spec, children_manifest, parent_filename)
    script_path = output_dir / "create-issues.sh"
    script_path.write_text(script)
    script_path.chmod(0o755)

    return manifest


def render_create_issues_script(
    spec: Spec, children: list[dict], parent_filename: str
) -> str:
    parent_labels = " ".join(
        f"--label {shlex.quote(l)}" for l in spec.parent_labels
    )

    lines = [
        "#!/usr/bin/env bash",
        "# Auto-generated by gh-runbook. Idempotent — re-running reads .issue-numbers.json.",
        "set -euo pipefail",
        'SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"',
        'cd "$SCRIPT_DIR"',
        "",
        "DRY_RUN=0",
        'if [ "${1:-}" = "--dry-run" ]; then DRY_RUN=1; fi',
        "",
        f"REPO={shlex.quote(spec.repo)}",
        "NUMBERS_FILE=.issue-numbers.json",
        "",
        'if [ -f "$NUMBERS_FILE" ]; then',
        '  NUMBERS=$(cat "$NUMBERS_FILE")',
        "else",
        "  NUMBERS='{}'",
        "fi",
        "",
        "lookup_number() {",
        "  python3 -c 'import json,sys; d=json.loads(sys.argv[1]); print(d.get(sys.argv[2], \"\"))' \"$NUMBERS\" \"$1\"",
        "}",
        "",
        "save_number() {",
        "  NUMBERS=$(python3 -c 'import json,sys; d=json.loads(sys.argv[1]); d[sys.argv[2]]=sys.argv[3]; print(json.dumps(d))' \"$NUMBERS\" \"$1\" \"$2\")",
        '  echo "$NUMBERS" > "$NUMBERS_FILE"',
        "}",
        "",
        "create_issue() {",
        '  local slug="$1"; shift',
        '  local title="$1"; shift',
        '  local body_file="$1"; shift',
        '  local labels="$1"; shift',
        '  local milestone="$1"; shift',
        '  local existing',
        '  existing=$(lookup_number "$slug")',
        '  if [ -n "$existing" ]; then',
        '    echo "▸ $slug already created: #$existing" >&2',
        '    printf "%s" "$existing"',
        "    return",
        "  fi",
        '  if [ "$DRY_RUN" -eq 1 ]; then',
        '    echo "▸ DRY-RUN would create: $title ($slug)" >&2',
        '    printf "DRY-%s" "$slug"',
        "    return",
        "  fi",
        "  local url",
        '  if [ -n "$milestone" ]; then',
        '    url=$(eval gh issue create --repo "$REPO" --title "\\"$title\\"" --body-file "\\"$body_file\\"" $labels --milestone "\\"$milestone\\"")',
        "  else",
        '    url=$(eval gh issue create --repo "$REPO" --title "\\"$title\\"" --body-file "\\"$body_file\\"" $labels)',
        "  fi",
        '  local number="${url##*/}"',
        '  save_number "$slug" "$number"',
        '  printf "%s" "$number"',
        "}",
        "",
        f"PARENT_NUMBER=$(create_issue parent {shlex.quote(spec.parent_title)} {shlex.quote(parent_filename)} {shlex.quote(parent_labels)} {shlex.quote(spec.parent_milestone or '')})",
        'echo "Parent: #$PARENT_NUMBER"',
        "",
    ]

    for child in children:
        slug = child["slug"]
        title = child["title"]
        body_path = child["body_path"]
        labels_arg = " ".join(f"--label {shlex.quote(l)}" for l in child["labels"])
        milestone = child["milestone"] or ""
        var = f"CHILD_{slug.upper().replace('-', '_')}"
        lines.append(
            f"{var}=$(create_issue {shlex.quote(slug)} {shlex.quote(title)} {shlex.quote(body_path)} {shlex.quote(labels_arg)} {shlex.quote(milestone)})"
        )

    lines += [
        "",
        'if [ "$DRY_RUN" -eq 0 ]; then',
        "  for f in ??-*-*.md; do",
        '    sed -i.bak "s|{{PARENT_ISSUE}}|#$PARENT_NUMBER|g" "$f"',
        '    rm -f "$f.bak"',
        "  done",
        "fi",
        "",
        'if [ "$DRY_RUN" -eq 0 ]; then',
        f'  PARENT_BODY=$(cat {shlex.quote(parent_filename)})',
    ]

    for batch in spec.batches:
        batch_children = [c for c in children if c["batch"] == batch.id]
        lines.append(f'  LINES_{batch.id}=""')
        for c in batch_children:
            var = f"CHILD_{c['slug'].upper().replace('-', '_')}"
            title = c["title"].replace('"', '\\"')
            lines.append(
                f'  LINES_{batch.id}+="- [ ] {title} (#${var})"$\'\\n\''
            )
        lines.append(
            f'  PARENT_BODY="${{PARENT_BODY//\\{{\\{{CHILD_ISSUE_LINES_{batch.id}\\}}\\}}/$LINES_{batch.id}}}"'
        )

    lines += [
        '  printf "%s" "$PARENT_BODY" > parent-body.rendered.md',
        f'  gh issue edit "$PARENT_NUMBER" --repo "$REPO" --body-file parent-body.rendered.md',
        "fi",
        "",
        'echo',
        'echo "Issue summary:"',
        'echo "Parent #$PARENT_NUMBER"',
        "python3 -c 'import json; d=json.load(open(\".issue-numbers.json\")); [print(f\"{k}: #{v}\") for k,v in d.items() if k != \"parent\"]' 2>/dev/null || true",
    ]

    return "\n".join(lines) + "\n"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate a Plumb runbook (parent issue + children + create-issues.sh) from a YAML spec."
    )
    parser.add_argument("spec", type=Path, help="Path to a runbook spec YAML file")
    parser.add_argument("--output-dir", type=Path, help="Directory to write drafts into")
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Retained for compatibility. The generator never creates issues directly.",
    )
    parser.add_argument(
        "--force", action="store_true", help="Overwrite an existing output directory"
    )
    parser.add_argument(
        "--validate-only",
        action="store_true",
        help="Validate the spec against the schema and exit",
    )
    return parser.parse_args()


def default_output_dir(spec_path: Path) -> Path:
    stem = spec_path.stem.replace("-spec", "")
    return WORKSPACE_ROOT / ".agents" / "runs" / "gh-runbook" / stem


def main() -> None:
    args = parse_args()

    if not args.spec.exists():
        raise RuntimeError(f"spec not found: {args.spec}")

    spec = load_spec(args.spec)

    if args.validate_only:
        print(f"OK: {args.spec} is valid")
        return

    output_dir = args.output_dir or default_output_dir(args.spec)
    if output_dir.exists() and any(output_dir.iterdir()) and not args.force:
        raise RuntimeError(
            f"output directory {output_dir} already exists and is non-empty — pass --force to overwrite"
        )

    manifest = write_outputs(spec, args.spec, output_dir)
    print(
        json.dumps(
            {
                "output_dir": str(output_dir),
                "issue_count": 1 + len(manifest["children"]),
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        sys.exit(1)
