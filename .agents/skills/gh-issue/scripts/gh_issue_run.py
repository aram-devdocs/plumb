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

REVIEW_GATES = ['spec', 'quality', 'architecture', 'security']
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
    spec = reviews.get('spec')
    quality = reviews.get('quality')
    if quality == 'pass' and spec != 'pass':
        errors.append('code-quality-reviewer passed but spec-reviewer did not - invalid order')

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
    state = load_state(primary, slug)
    pr = state.get('pr')
    if not pr:
        print('ERROR: No PR number in state. Create PR first.', file=sys.stderr)
        sys.exit(1)

    print(f'Polling CI for PR #{pr}...')
    try:
        result = subprocess.run(
            ['gh', 'pr', 'checks', str(pr), '--repo', 'aram-devdocs/omnifol'],
            capture_output=True,
            text=True,
            timeout=30,
        )
        print(result.stdout)
        if result.returncode != 0:
            print(result.stderr, file=sys.stderr)
    except FileNotFoundError:
        print('ERROR: gh CLI not found', file=sys.stderr)
        sys.exit(1)
    except subprocess.TimeoutExpired:
        print('ERROR: gh pr checks timed out', file=sys.stderr)
        sys.exit(1)


def cmd_cleanup_worktree(primary: int, slug: str) -> None:
    state = load_state(primary, slug)
    if not state.get('worktree'):
        print('No worktree to clean up (worktree=false)')
        return
    branch = state.get('branch')
    if not branch:
        print('ERROR: No branch in state', file=sys.stderr)
        sys.exit(1)
    worktree_path = Path('..') / f'omnifol-{branch}'
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
