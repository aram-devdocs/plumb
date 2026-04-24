# Plan: Issue #<primary> - <title>

## Issue Summary

<one to three sentence summary of what the issue asks for>

## Acceptance Criteria

- [ ] <criterion 1>
- [ ] <criterion 2>
- [ ] <criterion 3>

## Affected Packages

| Package | Layer | Files |
|---------|-------|-------|
| `@omnifol/<pkg>` | L<N> | `path/to/file.ts` |

## Implementation Approach

<describe the approach, referencing layer rules and existing patterns>

## Subagent Dispatch Plan

| Agent | Scope | Files |
|-------|-------|-------|
| `implementer` | <description> | `path/to/file.ts` |

Parallel batches (if applicable):
- Batch 1: <agent A>, <agent B> (independent - different packages/files)
- Batch 2: <agent C> (depends on batch 1 output)

Domain expert consultation required: yes/no
- If yes: consult `trading-domain-expert` or `omniscript-domain-expert` BEFORE dispatch

## Review Gates

- [ ] spec-reviewer (always required)
- [ ] code-quality-reviewer (always required, after spec passes)
- [ ] architecture-validator (always required)
- [ ] security-auditor (required if: auth / exchange API / financial data / encryption)

Security-auditor trigger reason: <explain why or "not required">

## Verification

```bash
pnpm typecheck && pnpm lint && pnpm --filter @omnifol/<pkg> test
```

Additional package-specific tests:
```bash
pnpm --filter @omnifol/<other-pkg> test
```

## Branch

`codex/<primary>-<type>-<slug>`

PR target: `dev`

PR title: `<type>: <imperative description>`

## Notes

<any architectural decisions, risks, or caveats>
