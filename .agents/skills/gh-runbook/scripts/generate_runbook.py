#!/usr/bin/env python3
"""
Generate grouped GitHub issue drafts from a report.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import dataclass, asdict
from datetime import datetime, timezone
from pathlib import Path

SKILL_ROOT = Path(__file__).resolve().parents[1]
WORKSPACE_ROOT = SKILL_ROOT.parents[2]
CHILD_TEMPLATE = (SKILL_ROOT / "assets/child-issue-template.md").read_text()
PARENT_TEMPLATE = (SKILL_ROOT / "assets/parent-issue-template.md").read_text()
DEFAULT_REPO = "aram-devdocs/omnifol"
PLACEHOLDER_PARENT = "{{PARENT_ISSUE}}"


@dataclass
class IssueDraft:
    key: str
    title: str
    summary: str
    acceptance_criteria: list[str]
    dependencies: list[str]
    effort: str
    implementation_notes: list[str]
    labels: list[str]
    milestone: str | None
    related_existing: list[int]
    source_sections: list[str]
    suggested_delivery: list[str]


def run(cmd: list[str]) -> str:
    result = subprocess.run(
        cmd,
        cwd=WORKSPACE_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(f"command failed: {' '.join(cmd)}\n{result.stderr.strip()}")
    return result.stdout


def now_iso() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def slugify(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")


def render_template(template: str, mapping: dict[str, str]) -> str:
    for key, value in mapping.items():
        template = template.replace(f"{{{{{key}}}}}", value)
    return template


def extract_sections(report_text: str) -> dict[str, str]:
    sections: dict[str, str] = {}
    current_heading: str | None = None
    current_lines: list[str] = []

    for line in report_text.splitlines():
        if line.startswith("## ") or line.startswith("### "):
            if current_heading is not None:
                sections[current_heading] = "\n".join(current_lines).strip()
            current_heading = line.lstrip("#").strip()
            current_lines = []
            continue
        if current_heading is not None:
            current_lines.append(line)

    if current_heading is not None:
        sections[current_heading] = "\n".join(current_lines).strip()

    return sections


def compact(text: str, limit: int = 520) -> str:
    text = re.sub(r"\s+", " ", text).strip()
    if len(text) <= limit:
        return text
    return text[: limit - 1].rstrip() + "…"


def extract_table_rows(section_text: str) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    for line in section_text.splitlines():
        if not line.startswith("|") or line.startswith("|---"):
            continue
        parts = [part.strip() for part in line.strip("|").split("|")]
        if len(parts) < 4 or parts[0] == "Sev":
            continue
        rows.append(
            {
                "severity": parts[0],
                "area": parts[1],
                "finding": parts[2],
                "evidence": parts[3],
            }
        )
    return rows


def gh_label_to_effort(effort: str) -> str:
    return {
        "XS": "effort-xs",
        "S": "effort-s",
        "M": "effort-m",
        "L": "effort-l",
        "XL": "effort-xl",
    }[effort]


def milestone_map() -> dict[str, str]:
    return {
        "critical": "sprint_001_phase_1_critical_fixes",
        "ux": "sprint_001_phase_2_ux_polish",
        "auth": "sprint_001_phase_3_auth_onboarding",
        "core": "sprint_001_phase_4_core_features",
    }


def build_drafts(report_path: Path) -> tuple[IssueDraft, list[IssueDraft], dict[str, str]]:
    report_text = report_path.read_text()
    sections = extract_sections(report_text)
    blocker_rows = extract_table_rows(sections.get("Top blockers", ""))
    milestones = milestone_map()

    blocker_lookup = {
        "dlq": next((row for row in blocker_rows if "Dead-letter queue" in row["finding"]), None),
        "orchestration": next((row for row in blocker_rows if "/admin/orchestration" in row["finding"]), None),
        "symbol": next((row for row in blocker_rows if "splits slash-formatted symbols" in row["finding"]), None),
        "export": next((row for row in blocker_rows if "PDF export handling" in row["finding"]), None),
        "trading-safety": next((row for row in blocker_rows if "real trade" in row["finding"]), None),
    }

    parent = IssueDraft(
        key="release-readiness-runbook",
        title="chore: release readiness runbook for 2026-04-23 ui/ux audit",
        summary=compact(
            "The 2026-04-23 release audit found Omnifol is still a no-go for a broad public release. "
            "This runbook coordinates the narrow set of workstreams required to move the app from broad-surface alpha chaos to a reduced, coherent launch scope."
        ),
        acceptance_criteria=[
            "Each child workstream issue is created, sized, and assigned to the correct milestone band.",
            "P0 correctness and operational blockers are resolved or the affected surfaces are hidden from release scope.",
            "Launch-visible onboarding, feedback, help, and admin surfaces have a single clear ownership model.",
            "Release scope is explicitly reduced where product value does not justify launch exposure.",
        ],
        dependencies=["None. This is the coordination issue."],
        effort="L",
        implementation_notes=[
            "Source report: `reports/ui-audit/2026-04-23/omnifol-release-ui-ux-audit.md`.",
            "This replaces the broad, issue-count-driven framing from #454 with audit-driven launch workstreams.",
            "Use `/gh-issue` for implementation and `/gh-review` for local PR review on each child issue or grouped batch.",
        ],
        labels=["alpha-readiness", "tech debt", "full stack", "P0-blocker", gh_label_to_effort("L")],
        milestone=milestones["critical"],
        related_existing=[454],
        source_sections=["Executive summary", "Release recommendation", "Top blockers", "Release-readiness by feature"],
        suggested_delivery=["gh-runbook", "gh-issue", "gh-review"],
    )

    children = [
        IssueDraft(
            key="trading-export-correctness",
            title="fix: trading and export correctness blockers before launch",
            summary=compact(
                "Trading and export currently undermine user trust. The audit found malformed symbol handling in the backend transaction sync path and a broken PDF export path that rewrites PDFs into JSON blobs."
            ),
            acceptance_criteria=[
                "Slash-formatted symbols are parsed and stored correctly across sync, activity, trading, and export surfaces.",
                "Export preserves the requested file type and produces a valid user-facing artifact.",
                "Regression coverage exists for slash-formatted symbols and PDF export handling.",
                "Audit evidence in the report is no longer reproducible in dev.",
            ],
            dependencies=[f"Depends on {PLACEHOLDER_PARENT} - coordinated launch blocker."],
            effort="L",
            implementation_notes=[
                "Likely scope: `packages/backend/services`, `packages/web/hooks`, `packages/web/ui`, `apps/web`.",
                "Audit evidence: "
                + compact(
                    " / ".join(
                        filter(
                            None,
                            [
                                blocker_lookup["symbol"]["finding"] if blocker_lookup["symbol"] else "",
                                blocker_lookup["export"]["finding"] if blocker_lookup["export"] else "",
                            ],
                        )
                    )
                ),
                "Related existing tracking: #409 (manual sync UI), #431 (DLQ monitoring).",
                "Suggested delivery: `/gh-issue` with service-layer tests first; include `/gh-review` before PR.",
            ],
            labels=["alpha-readiness", "back end", "front end", "P0-blocker", gh_label_to_effort("L")],
            milestone=milestones["critical"],
            related_existing=[409, 431],
            source_sections=["Top blockers", "Trading/export symbol corruption", "Export is not ready"],
            suggested_delivery=["gh-issue", "test-runner", "gh-review"],
        ),
        IssueDraft(
            key="trading-safety-order-gating",
            title="fix: harden trading safety framing and real-order gating",
            summary=compact(
                "The current trading flow still feels unsafe. The audit found inconsistent symbol display, incomplete real-trade confirmation wiring, and environment-driven safety behavior that is weaker than the launch framing implies."
            ),
            acceptance_criteria=[
                "Real-order confirmation logic is correct for production and non-production environments.",
                "Trade entry, confirmation, and paper/live framing are coherent across supported viewports.",
                "Trading UI uses normalized symbols without display corruption.",
                "Manual QA documents the safe paths and disabled paths for launch.",
            ],
            dependencies=[
                f"Depends on {PLACEHOLDER_PARENT} - coordinated launch blocker.",
                "Blocked by trading/export correctness if symbol normalization is shared.",
            ],
            effort="M",
            implementation_notes=[
                "Likely scope: `apps/web/src/routes/_app/trading.tsx`, `packages/web/hooks/src/trading`, `packages/web/ui/src/templates`, related UI tests.",
                "Audit evidence: "
                + compact(blocker_lookup["trading-safety"]["finding"] if blocker_lookup["trading-safety"] else "Trading safety regression noted in top blockers."),
                "Related existing tracking: #273, #192, #202.",
                "Suggested delivery: `/gh-issue` with a security reviewer for any live-trade code paths.",
            ],
            labels=["alpha-readiness", "front end", "back end", "P1-launch", gh_label_to_effort("M")],
            milestone=milestones["critical"],
            related_existing=[192, 202],
            source_sections=["Top blockers", "Data trust is not high enough in trading/export", "Release-readiness by feature"],
            suggested_delivery=["gh-issue", "security-auditor", "gh-review"],
        ),
        IssueDraft(
            key="dlq-schema-incident",
            title="fix: resolve dlq schema failures and restore background-job health",
            summary=compact(
                "The audit found a live dead-letter queue full of failed portfolio-sync jobs due to a Prisma schema mismatch. This is an operational launch blocker, not a cosmetic admin issue."
            ),
            acceptance_criteria=[
                "The failing `exchangeAccountId` schema mismatch is resolved in the background-job path.",
                "Failed DLQ jobs are triaged, replayed where safe, and the queue returns near zero for this incident class.",
                "Admin DLQ views show healthy current-state data after the fix.",
                "Follow-up monitoring or alerting covers recurrence of the same job failure mode.",
            ],
            dependencies=[f"Depends on {PLACEHOLDER_PARENT} - coordinated launch blocker."],
            effort="L",
            implementation_notes=[
                "Likely scope: `packages/backend/database`, `packages/backend/repositories`, `packages/backend/services`, `apps/server`, admin views.",
                "Audit evidence: "
                + compact(blocker_lookup["dlq"]["finding"] if blocker_lookup["dlq"] else "DLQ failure cluster documented in top blockers."),
                "Related existing tracking: #431, #418, #423.",
                "Suggested delivery: `/gh-issue` with sequential migration/service work and explicit verification in dev admin.",
            ],
            labels=["alpha-readiness", "audit", "back end", "P0-blocker", gh_label_to_effort("L")],
            milestone=milestones["critical"],
            related_existing=[418, 423, 431],
            source_sections=["Top blockers", "Release recommendation", "Admin"],
            suggested_delivery=["gh-issue", "database-migration", "gh-review"],
        ),
        IssueDraft(
            key="admin-orchestration-health",
            title="fix: restore admin orchestration or remove it from active admin navigation",
            summary=compact(
                "Admin orchestration currently hangs and then fails to load issues from GitHub. The launch decision needs to be explicit: restore a working operator surface or hide it until it is dependable."
            ),
            acceptance_criteria=[
                "The orchestration page either loads successfully with valid data or is removed from active admin navigation.",
                "Error states are explicit and actionable for operator-facing failures.",
                "A clear ownership decision exists for whether orchestration is launch-essential or internal follow-up work.",
            ],
            dependencies=[
                f"Depends on {PLACEHOLDER_PARENT} - coordinated launch blocker.",
                "May depend on DLQ/schema incident work if shared admin data paths are unhealthy.",
            ],
            effort="M",
            implementation_notes=[
                "Likely scope: `apps/server/src/trpc/routers/orchestration.router.ts`, admin route/page/hook layers, GitHub integration handling.",
                "Audit evidence: "
                + compact(blocker_lookup["orchestration"]["finding"] if blocker_lookup["orchestration"] else "Admin orchestration failure documented in top blockers."),
                "Related existing tracking: #454, PR #479.",
                "Suggested delivery: `/gh-issue` with clear keep-or-hide decision captured in the issue body.",
            ],
            labels=["alpha-readiness", "audit", "full stack", "P0-blocker", gh_label_to_effort("M")],
            milestone=milestones["critical"],
            related_existing=[454],
            source_sections=["Top blockers", "Admin", "Release recommendation"],
            suggested_delivery=["gh-issue", "gh-review"],
        ),
        IssueDraft(
            key="onboarding-help-whats-new-consolidation",
            title="refactor: consolidate onboarding, tutorial, help, and changelog surfaces",
            summary=compact(
                "The launch surface currently exposes overlapping onboarding and education systems: dashboard onboarding, sidebar tutorials, docs drawer, help center, and What's New. Users see too many guidance systems at once."
            ),
            acceptance_criteria=[
                "A single primary onboarding and education path is defined for new users.",
                "Tutorial, help, changelog, and contextual guidance surfaces have explicit ownership and non-overlapping jobs.",
                "Launch-visible flows are coherent on desktop, tablet, and mobile.",
                "Deprecated or redundant guidance surfaces are removed, hidden, or feature-gated.",
            ],
            dependencies=[f"Depends on {PLACEHOLDER_PARENT} - launch-scope coordination."],
            effort="L",
            implementation_notes=[
                "Likely scope: `packages/web/ui`, `packages/web/hooks`, `apps/web/src/routes/_app`, tutorial definitions, changelog/help routes.",
                "Related existing tracking: #205, #403, #404, #406.",
                "Suggested delivery: `/gh-issue` plus a stateless-UI review and responsive manual QA.",
            ],
            labels=["alpha-readiness", "saas", "front end", "P1-launch", gh_label_to_effort("L")],
            milestone=milestones["ux"],
            related_existing=[205, 403, 404, 406],
            source_sections=["Product surface is too wide for its current level of finish", "Onboarding / education duplication"],
            suggested_delivery=["gh-issue", "ui-component", "gh-review"],
        ),
        IssueDraft(
            key="feedback-support-bug-report-consolidation",
            title="refactor: consolidate feedback, bug report, and support intake",
            summary=compact(
                "Users and operators currently have too many issue-intake paths: a floating feedback widget, a bug-report route, a support route, and three matching admin queues. The audit found no clear ownership model."
            ),
            acceptance_criteria=[
                "Launch-visible intake paths are reduced to a coherent, intentionally distinct set.",
                "Admin queues align with the reduced user-facing intake model.",
                "CTA copy and routing explain which path a user should choose and why.",
                "Duplicate or low-value intake surfaces are hidden, removed, or feature-gated.",
            ],
            dependencies=[f"Depends on {PLACEHOLDER_PARENT} - launch-scope coordination."],
            effort="M",
            implementation_notes=[
                "Likely scope: feedback widget, `support`, `bug-report`, admin feedback queues, navigation, and copy.",
                "Related existing tracking: #407, #408, #454.",
                "Suggested delivery: `/gh-issue` with a product-scope decision before code changes.",
            ],
            labels=["alpha-readiness", "saas", "front end", "P1-launch", gh_label_to_effort("M")],
            milestone=milestones["ux"],
            related_existing=[407, 408, 454],
            source_sections=["There is too much issue-intake duplication", "Issue-intake duplication"],
            suggested_delivery=["gh-issue", "gh-review"],
        ),
        IssueDraft(
            key="dashboard-analytics-scope",
            title="refactor: rationalize dashboard and analytics launch scope",
            summary=compact(
                "Dashboard and analytics currently feel busy without a strong user-value hierarchy. The audit called out weak daily-use clarity, mixed priorities on the home surface, and analytics that may be demo filler rather than product value."
            ),
            acceptance_criteria=[
                "The default dashboard answers a clear first-question and next-action for the user.",
                "Low-value widgets or analytics surfaces are hidden, cut, or moved behind flags.",
                "Admin analytics and user-facing analytics have distinct goals and information architecture.",
                "Responsive layouts remain stable after scope reduction.",
            ],
            dependencies=[f"Depends on {PLACEHOLDER_PARENT} - launch-scope coordination."],
            effort="L",
            implementation_notes=[
                "Likely scope: dashboard route/page/hook/UI layers, analytics routes, feature-flag gating.",
                "Related existing tracking: #232, #450.",
                "Suggested delivery: `/gh-issue`; use existing typed feature flags instead of a new JSON config.",
            ],
            labels=["alpha-readiness", "analytics", "front end", "P1-launch", gh_label_to_effort("L")],
            milestone=milestones["core"],
            related_existing=[232, 450],
            source_sections=["Value hierarchy is weak on the home/dashboard side", "Dashboard/analytics scope rationalization", "Feature-flag rollout recommendation"],
            suggested_delivery=["gh-issue", "gh-review"],
        ),
        IssueDraft(
            key="auth-invite-token-fallbacks",
            title="fix: replace auth token dead ends with explicit fallback states",
            summary=compact(
                "Auth security fundamentals are strong, but invite-only signup, token-only reset, and verification-pending direct links still fall into harsh or confusing dead-end states when required params are missing."
            ),
            acceptance_criteria=[
                "Signup, reset-password, and verification-pending routes render explicit safe fallback states when required params are missing or expired.",
                "Fallback states preserve security posture and avoid leaking account existence or token validity.",
                "Mobile login and auth entry surfaces remain usable with cookie and system overlays present.",
                "Manual QA covers happy-path and missing-token deep-link cases.",
            ],
            dependencies=[f"Depends on {PLACEHOLDER_PARENT} - launch-scope coordination."],
            effort="M",
            implementation_notes=[
                "Likely scope: `apps/web/src/routes/_auth`, auth UI templates, MFA/login layout interactions.",
                "Related existing tracking: #403, #410, #14, #18, #183.",
                "Suggested delivery: `/gh-issue` with explicit security review because auth flows are touched.",
            ],
            labels=["alpha-readiness", "auth", "front end", "P1-launch", gh_label_to_effort("M")],
            milestone=milestones["auth"],
            related_existing=[14, 18, 183, 403, 410],
            source_sections=["Auth and security fundamentals are materially stronger than the auth UX", "Auth", "Release-readiness by feature"],
            suggested_delivery=["gh-issue", "security-auditor", "gh-review"],
        ),
        IssueDraft(
            key="storybook-runtime-gate",
            title="fix: make storybook a real runtime gate for release surfaces",
            summary=compact(
                "Storybook coverage currently overstates confidence. Several important stories crash at runtime because they no longer match current stateless page contracts, which means the release signal is weaker than the validation output suggests."
            ),
            acceptance_criteria=[
                "Broken page and template stories are updated to current stateless `pageView` contracts.",
                "Release-critical stories used by the audit render successfully in Storybook.",
                "Storybook validation distinguishes runtime health from coverage-only counts.",
                "Storybook remains useful as a state catalog for future release audits.",
            ],
            dependencies=[f"Depends on {PLACEHOLDER_PARENT} - launch-quality gate."],
            effort="M",
            implementation_notes=[
                "Likely scope: `packages/web/ui/src/**/*.stories.tsx`, Storybook validators, affected stateless page contracts.",
                "Related existing tracking: #441, PR #467, PR #479.",
                "Suggested delivery: `/gh-issue` with focused UI/story validation rather than broad product work.",
            ],
            labels=["alpha-readiness", "testing", "front end", "P1-launch", gh_label_to_effort("M")],
            milestone=milestones["ux"],
            related_existing=[441],
            source_sections=["Storybook is present, but not dependable as a release-confidence tool", "Storybook assessment"],
            suggested_delivery=["gh-issue", "test-runner", "gh-review"],
        ),
        IssueDraft(
            key="launch-scope-cleanup-admin-ia",
            title="chore: finalize launch scope and admin information architecture",
            summary=compact(
                "The audit found several visible surfaces that should likely not ship broadly yet, including internal admin tools, low-value strategies exposure, friend features, and other release-scope distractions. The product needs an explicit keep / flag / cut decision."
            ),
            acceptance_criteria=[
                "Each audited feature has a final launch verdict: keep, polish, flag, cut, or internal-only.",
                "Admin navigation reflects internal-only tools and removes launch-inappropriate routes from primary user paths.",
                "Feature-flag rollout decisions use the existing typed feature-flag system and current admin controls.",
                "Release scope documentation matches the actual app shell and route visibility.",
            ],
            dependencies=[
                f"Depends on {PLACEHOLDER_PARENT} - launch coordination.",
                "Should follow onboarding/feedback/dashboard decisions so navigation reflects the reduced surface.",
            ],
            effort="M",
            implementation_notes=[
                "Likely scope: route gating, sidebar/navigation composition, feature-flag definitions and admin controls, release documentation.",
                "Related existing tracking: #454, #432, #410.",
                "Suggested delivery: `/gh-issue` with product and admin IA review before implementation.",
            ],
            labels=["alpha-readiness", "tech debt", "full stack", "P1-launch", gh_label_to_effort("M")],
            milestone=milestones["core"],
            related_existing=[410, 432, 454],
            source_sections=["Release recommendation", "Feature-flag rollout recommendation", "Release-readiness by feature"],
            suggested_delivery=["gh-issue", "gh-review"],
        ),
    ]

    return parent, children, sections


def markdown_list(items: list[str], *, checked: bool = False) -> str:
    if not items:
        return "- None."
    prefix = "- [ ]" if checked else "-"
    return "\n".join(f"{prefix} {item}" for item in items)


def render_issue_body(issue: IssueDraft, *, child_issues: str = "- To be backfilled after issue creation.") -> str:
    template = PARENT_TEMPLATE if issue.key == "release-readiness-runbook" else CHILD_TEMPLATE
    mapping = {
        "SUMMARY": issue.summary,
        "ACCEPTANCE_CRITERIA": markdown_list(issue.acceptance_criteria, checked=True),
        "DEPENDENCIES": markdown_list(issue.dependencies),
        "EFFORT": issue.effort,
        "IMPLEMENTATION_NOTES": markdown_list(issue.implementation_notes),
        "CHILD_ISSUES": child_issues,
    }
    return render_template(template, mapping)


def write_runbook(output_dir: Path, report_path: Path, repo: str) -> dict[str, object]:
    parent, children, sections = build_drafts(report_path)
    output_dir.mkdir(parents=True, exist_ok=True)

    child_paths: list[dict[str, object]] = []
    child_refs = [f"- [ ] {child.title} (`{child.key}`)" for child in children]
    parent_template_path = output_dir / "00-parent-release-readiness-runbook.template.md"
    parent_template_path.write_text(render_issue_body(parent, child_issues="{{CHILD_ISSUES}}"))

    parent_initial_body = render_issue_body(parent, child_issues="\n".join(child_refs))
    parent_body_path = output_dir / "00-parent-release-readiness-runbook.md"
    parent_body_path.write_text(parent_initial_body)

    manifest_children: list[dict[str, object]] = []
    for index, child in enumerate(children, start=1):
        filename = f"{index:02d}-{slugify(child.key)}.md"
        path = output_dir / filename
        path.write_text(render_issue_body(child))
        child_paths.append({"key": child.key, "title": child.title, "path": str(path)})
        manifest_children.append(
            {
                **asdict(child),
                "body_path": str(path),
            }
        )

    summary_lines = [
        f"# Runbook dry run for `{report_path}`",
        "",
        f"- Generated at: `{now_iso()}`",
        f"- Repo: `{repo}`",
        f"- Parent issue: `{parent.title}`",
        "",
        "## Child issues",
    ]
    summary_lines.extend(f"- `{child['key']}` → `{child['title']}`" for child in child_paths)
    (output_dir / "runbook-summary.md").write_text("\n".join(summary_lines) + "\n")

    script_path = output_dir / "create-issues.sh"
    child_specs_path = output_dir / "child-specs.json"
    child_specs = [
        {
            "key": child.key,
            "title": child.title,
            "body_path": str(output_dir / f"{index:02d}-{slugify(child.key)}.md"),
            "labels": child.labels,
            "milestone": child.milestone,
        }
        for index, child in enumerate(children, start=1)
    ]
    child_specs_path.write_text(json.dumps(child_specs, indent=2) + "\n")
    script_path.write_text(
        f"""#!/usr/bin/env bash
set -euo pipefail

REPO="{repo}"
OUT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
PARENT_TEMPLATE="$OUT_DIR/00-parent-release-readiness-runbook.template.md"
PARENT_BODY="$OUT_DIR/00-parent-release-readiness-runbook.md"
CHILD_SPECS="$OUT_DIR/child-specs.json"
PARENT_URL="$(gh issue create --repo "$REPO" --title "{parent.title}" --body-file "$PARENT_BODY" {" ".join(f'--label "{label}"' for label in parent.labels)} --milestone "{parent.milestone}")"
PARENT_NUMBER="${{PARENT_URL##*/}}"

declare -A ISSUE_NUMBERS=()
declare -A ISSUE_TITLES=()

python3 - "$OUT_DIR" "$PARENT_NUMBER" <<'PY'
from pathlib import Path
import sys

out_dir = Path(sys.argv[1])
parent_number = sys.argv[2]
for path in sorted(out_dir.glob("[0-9][0-9]-*.md")):
    text = path.read_text().replace("{PLACEHOLDER_PARENT}", "#" + parent_number)
    path.with_suffix(".rendered.md").write_text(text)
PY

while IFS= read -r entry; do
  key="$(python3 - <<'PY' "$entry"
import json, sys
print(json.loads(sys.argv[1])["key"])
PY
)"
  title="$(python3 - <<'PY' "$entry"
import json, sys
print(json.loads(sys.argv[1])["title"])
PY
)"
  body_path="$(python3 - <<'PY' "$entry"
import json, sys
print(json.loads(sys.argv[1])["body_path"])
PY
)"
  milestone="$(python3 - <<'PY' "$entry"
import json, sys
value = json.loads(sys.argv[1])["milestone"]
print(value or "")
PY
)"
  labels=$(python3 - <<'PY' "$entry"
import json, sys
print(" ".join('--label "' + label + '"' for label in json.loads(sys.argv[1])["labels"]))
PY
)
  rendered_path="${{body_path%.md}}.rendered.md"
  if [ -n "$milestone" ]; then
    url="$(eval gh issue create --repo "$REPO" --title '"$title"' --body-file '"$rendered_path"' $labels --milestone '"$milestone"')"
  else
    url="$(eval gh issue create --repo "$REPO" --title '"$title"' --body-file '"$rendered_path"' $labels)"
  fi
  ISSUE_NUMBERS["$key"]="${{url##*/}}"
  ISSUE_TITLES["$key"]="$title"
done < <(python3 - "$CHILD_SPECS" <<'PY'
import json
import sys

items = json.loads(open(sys.argv[1]).read())
print("\\n".join(json.dumps(item) for item in items))
PY
)

CHILD_LINES_FILE="$OUT_DIR/child-issues.rendered.md"
: > "$CHILD_LINES_FILE"
while IFS= read -r entry; do
  key="$(python3 - <<'PY' "$entry"
import json, sys
print(json.loads(sys.argv[1])["key"])
PY
)"
  title="${{ISSUE_TITLES[$key]}}"
  number="${{ISSUE_NUMBERS[$key]}}"
  echo "- [ ] $title (#$number)" >> "$CHILD_LINES_FILE"
done < <(python3 - "$CHILD_SPECS" <<'PY'
import json
import sys

items = json.loads(open(sys.argv[1]).read())
print("\\n".join(json.dumps(item) for item in items))
PY
)

python3 - "$PARENT_TEMPLATE" "$OUT_DIR/00-parent-release-readiness-runbook.final.md" "$CHILD_LINES_FILE" <<'PY'
from pathlib import Path
import sys

template = Path(sys.argv[1]).read_text()
target = Path(sys.argv[2])
child_lines = Path(sys.argv[3]).read_text().rstrip() or "- None."
target.write_text(template.replace("{{CHILD_ISSUES}}", child_lines))
PY

gh issue edit "$PARENT_NUMBER" --repo "$REPO" --body-file "$OUT_DIR/00-parent-release-readiness-runbook.final.md"

echo "Parent issue: #$PARENT_NUMBER"
for key in "${{!ISSUE_NUMBERS[@]}}"; do
  echo "$key -> #${{ISSUE_NUMBERS[$key]}}"
done
"""
    )
    script_path.chmod(0o755)

    manifest = {
        "generated_at": now_iso(),
        "source_report": str(report_path),
        "repo": repo,
        "parent": {
            **asdict(parent),
            "body_path": str(parent_body_path),
            "template_path": str(parent_template_path),
        },
        "children": manifest_children,
        "source_sections": list(sections.keys()),
        "script_path": str(script_path),
        "child_specs_path": str(child_specs_path),
    }
    (output_dir / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def parse_args() -> argparse.Namespace:
    if len(sys.argv) > 1 and not sys.argv[1].startswith("-") and sys.argv[1] not in {"generate", "render-template"}:
        legacy = argparse.ArgumentParser(
            description="Generate grouped Omnifol runbook issue drafts from a report."
        )
        legacy.add_argument("report")
        legacy.add_argument("--repo", default=DEFAULT_REPO)
        legacy.add_argument("--output-dir")
        legacy.add_argument("--dry-run", action="store_true")
        parsed = legacy.parse_args()
        parsed.command = "generate"
        return parsed

    parser = argparse.ArgumentParser(description="Generate grouped Omnifol runbook issue drafts from a report.")
    subparsers = parser.add_subparsers(dest="command")

    generate = subparsers.add_parser("generate", help="Generate issue drafts")
    generate.add_argument("report")
    generate.add_argument("--repo", default=DEFAULT_REPO)
    generate.add_argument("--output-dir")
    generate.add_argument("--dry-run", action="store_true")

    render = subparsers.add_parser("render-template", help="Render a template by replacing {{TOKENS}}.")
    render.add_argument("template")
    render.add_argument("--set", action="append", default=[])
    return parser.parse_args()


def handle_render(template_path: str, replacements: list[str]) -> None:
    mapping: dict[str, str] = {}
    for pair in replacements:
        if "=" not in pair:
            raise RuntimeError(f"invalid --set value: {pair}")
        key, value = pair.split("=", 1)
        mapping[key] = value
    print(render_template(Path(template_path).read_text(), mapping))


def main() -> None:
    args = parse_args()

    if args.command == "render-template":
        handle_render(args.template, args.set)
        return

    if args.command == "generate":
        report = Path(args.report)
        repo = args.repo
        output_dir = Path(args.output_dir) if args.output_dir else Path("/tmp") / f"gh-runbook-{slugify(report.stem)}"
    else:
        raise RuntimeError("report path is required")

    manifest = write_runbook(output_dir, report, repo)
    print(json.dumps(manifest, indent=2))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        sys.exit(1)
