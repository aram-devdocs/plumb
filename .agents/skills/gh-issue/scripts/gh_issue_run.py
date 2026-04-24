#!/usr/bin/env python3
"""
gh_issue_run.py - Durable state management for /gh-issue skill runs.

Commands:
  init-run <primary> <slug> [--issues N M ...] [--worktree]
  update-state <primary> <slug> [--phase PHASE] [--branch BRANCH] [--pr NUMBER]
                                [--review GATE VERDICT] [--commit SHA]
  validate-resume <primary> <slug>
  poll-pr <primary> <slug>
  cleanup-worktree <primary> <slug>
"""

import argparse
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

REPO = 'aram-devdocs/plumb'
RUNS_DIR = Path('.agents/runs/gh-issue')

VALID_PHASES = [
    'investigating',
    'planning',
    'bootstrapped',
    'implementing',
    'verifying',
    'reviewing',
    'pr',
    'waiting-ci',
    'cleanup',
    'done',
]

REVIEW_GATES = ['spec', 'quality', 'architecture', 'test', 'security']
VALID_VERDICTS = ['pass', 'fail', 'not_required']

BRANCH_PATTERN = 'codex/<primary>-<type>-<slug>'
BRANCH_RE = re.compile(r'^codex/\d+-[a-z]+-[a-z0-9-]+$')
CONVENTIONAL_TITLE_RE = re.compile(
    r'^(feat|fix|refactor|docs|test|chore|perf|ci|build|revert)(\(.+\))?: .+'
)


def run_dir(primary: int, slug: str) -> Path:
    return RUNS_DIR / f'{primary}-{slug}'


def state_path(primary: int, slug: str) -> Path:
    return run_dir(primary, slug) / 'state.json'


def now_iso() -> str:
    return datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')


def load_state(primary: int, slug: str) -> dict:
    p = state_path(primary, slug)
    if not p.exists():
        print(f'ERROR: No state found at {p}', file=sys.stderr)
        sys.exit(1)
    with p.open() as f:
        return json.load(f)


def save_state(primary: int, slug: str, state: dict) -> None:
    p = state_path(primary, slug)
    state['updated'] = now_iso()
    with p.open('w') as f:
        json.dump(state, f, indent=2)
    print(f'State saved: phase={state["phase"]} branch={state.get("branch")}')


def cmd_init_run(primary: int, slug: str, issues: list[int], worktree: bool) -> None:
    d = run_dir(primary, slug)
    d.mkdir(parents=True, exist_ok=True)
    p = state_path(primary, slug)
    if p.exists():
        print(f'WARNING: Run already exists at {d}. Use update-state to modify.')
        existing = load_state(primary, slug)
        print(json.dumps(existing, indent=2))
        return
    state = {
        'primary': primary,
        'issues': issues or [primary],
        'slug': slug,
        'phase': 'investigating',
        'branch': None,
        'pr': None,
        'commits': [],
        'reviews': {
            'spec': None,
            'quality': None,
            'architecture': None,
            'test': None,
            'security': 'not_required',
        },
        'worktree': worktree,
        'created': now_iso(),
        'updated': now_iso(),
    }
    with p.open('w') as f:
        json.dump(state, f, indent=2)
    print(f'Initialized run: {d}')
    print(json.dumps(state, indent=2))


def cmd_update_state(
    primary: int,
    slug: str,
    phase: str | None,
    branch: str | None,
    pr: int | None,
    review: tuple[str, str] | None,
    commit: str | None,
) -> None:
    state = load_state(primary, slug)

    if phase is not None:
        if phase not in VALID_PHASES:
            print(f'ERROR: Invalid phase "{phase}". Valid: {VALID_PHASES}', file=sys.stderr)
            sys.exit(1)
        state['phase'] = phase

    if branch is not None:
        if not BRANCH_RE.match(branch):
            print(
                f'WARNING: Branch "{branch}" does not match pattern {BRANCH_PATTERN}',
                file=sys.stderr,
            )
        state['branch'] = branch

    if pr is not None:
        state['pr'] = pr
        if state['phase'] not in ('pr', 'waiting-ci', 'cleanup', 'done'):
            state['phase'] = 'pr'

    if review is not None:
        gate, verdict = review
        if gate not in REVIEW_GATES:
            print(f'ERROR: Invalid gate "{gate}". Valid: {REVIEW_GATES}', file=sys.stderr)
            sys.exit(1)
        if verdict not in VALID_VERDICTS:
            print(f'ERROR: Invalid verdict "{verdict}". Valid: {VALID_VERDICTS}', file=sys.stderr)
            sys.exit(1)
        state['reviews'][gate] = verdict

    if commit is not None:
        if commit not in state['commits']:
            state['commits'].append(commit)

    save_state(primary, slug, state)


def cmd_validate_resume(primary: int, slug: str) -> None:
    p = state_path(primary, slug)
    if not p.exists():
        print(f'FAIL: No state at {p}', file=sys.stderr)
        sys.exit(1)
    state = load_state(primary, slug)
    errors = []

    if state.get('phase') == 'done':
        errors.append('Run is already done')

    branch = state.get('branch')
    if branch and not BRANCH_RE.match(branch):
        errors.append(f'Branch "{branch}" does not match pattern {BRANCH_PATTERN}')

    reviews = state.get('reviews', {})
    # Review-gate ordering invariant: spec → quality → architecture → test.
    if reviews.get('quality') == 'pass' and reviews.get('spec') != 'pass':
        errors.append('code-quality-reviewer passed but spec-reviewer did not — invalid order')
    if reviews.get('architecture') == 'pass' and reviews.get('quality') != 'pass':
        errors.append('architecture-validator passed but code-quality-reviewer did not — invalid order')
    if reviews.get('test') == 'pass' and reviews.get('architecture') != 'pass':
        errors.append('test-runner passed but architecture-validator did not — invalid order')

    plan_file = run_dir(primary, slug) / 'plan.md'
    if not plan_file.exists() and state.get('phase') not in ('investigating',):
        errors.append('plan.md missing - required after planning phase')

    if errors:
        for e in errors:
            print(f'VALIDATION ERROR: {e}', file=sys.stderr)
        sys.exit(1)

    print('OK: Run state is valid, can resume')
    print(f'  phase: {state["phase"]}')
    print(f'  branch: {state.get("branch")}')
    print(f'  pr: {state.get("pr")}')
    print(f'  reviews: {json.dumps(state.get("reviews", {}))}')


def cmd_poll_pr(primary: int, slug: str) -> None:
    """Poll the PR for combined CI + Claude-code-review status.

    Prints a summary line:
        status: ci=<pass|fail|pending> review=<approve|request_changes|block|pending|none>

    Then the raw CI table and the latest Claude review comment. Exit code:
        0 — both green (ci=pass review=approve): advance to cleanup
        1 — anything else: fix loop or wait
    """
    state = load_state(primary, slug)
    pr = state.get('pr')
    if not pr:
        print('ERROR: No PR number in state. Create PR first.', file=sys.stderr)
        sys.exit(1)

    ci_state = _poll_ci(pr)
    review_verdict, review_body = _poll_claude_review(pr)

    print(f'status: ci={ci_state} review={review_verdict}')
    print('')
    print('--- CI checks ---')
    _print_ci_checks(pr)
    if review_body:
        print('')
        print('--- latest Claude review comment ---')
        print(review_body[:4000])
        if len(review_body) > 4000:
            print(f'... ({len(review_body) - 4000} more chars — `gh pr view {pr} --comments` for full)')

    if ci_state == 'pass' and review_verdict == 'approve':
        sys.exit(0)
    sys.exit(1)


def _poll_ci(pr: int) -> str:
    """Return 'pass' | 'fail' | 'pending' for the PR's CI rollup."""
    try:
        result = subprocess.run(
            ['gh', 'pr', 'view', str(pr), '--repo', REPO, '--json', 'statusCheckRollup'],
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired) as exc:
        print(f'WARN: polling CI failed: {exc}', file=sys.stderr)
        return 'pending'

    if result.returncode != 0:
        return 'pending'

    try:
        data = json.loads(result.stdout)
    except json.JSONDecodeError:
        return 'pending'

    checks = data.get('statusCheckRollup') or []
    if not checks:
        return 'pending'

    any_pending = False
    any_failed = False
    for check in checks:
        status = (check.get('status') or '').upper()
        conclusion = (check.get('conclusion') or '').upper()
        if conclusion in ('FAILURE', 'CANCELLED', 'TIMED_OUT', 'ACTION_REQUIRED'):
            any_failed = True
        elif conclusion in ('SUCCESS', 'NEUTRAL', 'SKIPPED'):
            continue
        elif status in ('IN_PROGRESS', 'QUEUED', 'PENDING', 'WAITING') or not conclusion:
            any_pending = True

    if any_failed:
        return 'fail'
    if any_pending:
        return 'pending'
    return 'pass'


def _poll_claude_review(pr: int) -> tuple[str, str]:
    """Return (verdict, raw_body) from the latest Claude code review comment.

    verdict ∈ {'approve', 'request_changes', 'block', 'pending', 'none'}.
    The Claude review workflow (.github/workflows/claude-code-review.yml)
    posts PR comments via `anthropics/claude-code-action`. We match by
    author (github-actions / claude-code) OR by a line matching
    `Verdict: APPROVE | REQUEST_CHANGES | BLOCK`.
    """
    try:
        result = subprocess.run(
            ['gh', 'pr', 'view', str(pr), '--repo', REPO, '--json', 'comments'],
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return ('pending', '')

    if result.returncode != 0:
        return ('pending', '')

    try:
        data = json.loads(result.stdout)
    except json.JSONDecodeError:
        return ('pending', '')

    comments = data.get('comments') or []
    for c in reversed(comments):
        body = c.get('body', '') or ''
        author = (c.get('author') or {}).get('login', '')
        if 'Verdict:' in body or 'claude' in author.lower() or 'github-actions' in author.lower():
            for line in reversed(body.splitlines()):
                line = line.strip()
                for prefix in ('Verdict:', '**Verdict:**', '*Verdict:*'):
                    if line.lower().startswith(prefix.lower()):
                        tail = line[len(prefix):].strip().strip('*').strip()
                        verdict = tail.split()[0].upper() if tail else ''
                        if verdict == 'APPROVE':
                            return ('approve', body)
                        if verdict in ('REQUEST_CHANGES', 'REQUEST-CHANGES', 'CHANGES'):
                            return ('request_changes', body)
                        if verdict == 'BLOCK':
                            return ('block', body)
            return ('pending', body)
    return ('none', '')


def _print_ci_checks(pr: int) -> None:
    try:
        result = subprocess.run(
            ['gh', 'pr', 'checks', str(pr), '--repo', REPO],
            capture_output=True,
            text=True,
            timeout=30,
        )
        print(result.stdout, end='')
        if result.returncode != 0 and result.stderr:
            print(result.stderr, file=sys.stderr)
    except (FileNotFoundError, subprocess.TimeoutExpired) as exc:
        print(f'(ci check fetch failed: {exc})', file=sys.stderr)


def cmd_cleanup_worktree(primary: int, slug: str) -> None:
    state = load_state(primary, slug)
    if not state.get('worktree'):
        print('No worktree to clean up (worktree=false)')
        return
    branch = state.get('branch')
    if not branch:
        print('ERROR: No branch in state', file=sys.stderr)
        sys.exit(1)
    # Worktrees live next to the repo: `../plumb-<branch-slugified>`.
    wt_name = 'plumb-' + branch.replace('/', '-')
    worktree_path = Path('..') / wt_name
    if worktree_path.exists():
        print(f'Removing worktree at {worktree_path}')
        subprocess.run(['git', 'worktree', 'remove', str(worktree_path), '--force'], check=False)
        subprocess.run(['git', 'worktree', 'prune'], check=False)
    else:
        print(f'Worktree path {worktree_path} not found, skipping removal')


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description='gh-issue run state manager')
    sub = parser.add_subparsers(dest='command')

    p_init = sub.add_parser('init-run', help='Initialize a new run')
    p_init.add_argument('primary', type=int)
    p_init.add_argument('slug')
    p_init.add_argument('--issues', nargs='+', type=int, default=[])
    p_init.add_argument('--worktree', action='store_true')

    p_update = sub.add_parser('update-state', help='Update run state')
    p_update.add_argument('primary', type=int)
    p_update.add_argument('slug')
    p_update.add_argument('--phase', choices=VALID_PHASES)
    p_update.add_argument('--branch')
    p_update.add_argument('--pr', type=int)
    p_update.add_argument('--review', nargs=2, metavar=('GATE', 'VERDICT'))
    p_update.add_argument('--commit')

    p_validate = sub.add_parser('validate-resume', help='Validate a run can be resumed')
    p_validate.add_argument('primary', type=int)
    p_validate.add_argument('slug')

    p_poll = sub.add_parser('poll-pr', help='Poll CI status for PR')
    p_poll.add_argument('primary', type=int)
    p_poll.add_argument('slug')

    p_cleanup = sub.add_parser('cleanup-worktree', help='Remove worktree')
    p_cleanup.add_argument('primary', type=int)
    p_cleanup.add_argument('slug')

    return parser


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        sys.exit(1)

    if args.command == 'init-run':
        cmd_init_run(args.primary, args.slug, args.issues, args.worktree)
    elif args.command == 'update-state':
        review = tuple(args.review) if args.review else None
        cmd_update_state(
            args.primary, args.slug, args.phase, args.branch, args.pr, review, args.commit
        )
    elif args.command == 'validate-resume':
        cmd_validate_resume(args.primary, args.slug)
    elif args.command == 'poll-pr':
        cmd_poll_pr(args.primary, args.slug)
    elif args.command == 'cleanup-worktree':
        cmd_cleanup_worktree(args.primary, args.slug)
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == '__main__':
    main()
