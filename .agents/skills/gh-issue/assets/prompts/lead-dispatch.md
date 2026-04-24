# Lead Dispatch Prompt

You are the lead implementer for GitHub issue #{{PRIMARY}} in the Omnifol TypeScript/pnpm monorepo.

## Context

- Issue(s): {{ISSUES}}
- Branch: `{{BRANCH}}` (targeting `dev`)
- Run state: `.agents/runs/gh-issue/{{PRIMARY}}-{{SLUG}}/`
- Plan: `.agents/runs/gh-issue/{{PRIMARY}}-{{SLUG}}/plan.md`

## Your Responsibilities

1. Read the plan at the path above before doing anything
2. Follow the layer architecture (L1 Core -> L2 Data -> L3 Infra -> L4 Business -> L5 Integration -> L6 Apps)
3. Apply TDD: write failing tests FIRST, then implement
4. Commit atomically with conventional commit messages: `<type>: <description>`
5. Update run state after each commit:
   ```bash
   python3 .agents/skills/gh-issue/scripts/gh_issue_run.py update-state {{PRIMARY}} {{SLUG}} --commit <sha>
   ```

## Implementation Rules

- No `any` types, no `@ts-ignore`, no `console.log` (use `@omnifol/logger`)
- Business logic in `@omnifol/services`, not tRPC procedures
- Zod schemas in `@omnifol/types`, use `.parse()` for external data
- UI components must be stateless (props in, callbacks out)
- If user-facing copy changed, run a humanizer pass before handoff
- Use existing typed feature flags instead of hardcoded config
- Run `pnpm typecheck && pnpm lint` before committing

## Build Commands

```bash
pnpm typecheck                              # TypeScript check
pnpm lint                                   # Biome lint + format
pnpm --filter @omnifol/<pkg> test           # Package tests
pnpm turbo run build                        # Full build
```

## Parallel Dispatch Rules

If the plan calls for parallel subagents:
- Analyze task independence BEFORE dispatching
- Dispatch all batch agents in a SINGLE message (multiple Task calls)
- Each agent gets explicit file scope
- Never parallelize database migrations, compiler changes, shared type changes

## Deliverable

After implementation:
- All tests pass
- TypeScript compiles without errors
- Biome lint passes
- Each logical change is a separate commit
- Run state updated with all commit SHAs
