# gh-review workflow contract

This skill mirrors `.github/workflows/claude-code-review.yml`.

## Inputs

- PR number
- local diff range
- optional reviewer instructions

## Required phases

1. Context gathering
2. Architecture validation
3. Anti-pattern scan
4. Quality assessment
5. AI-writing detection
6. Scope verification
7. Structured output

## File buckets

- schema
- UI
- API
- config
- migration
- strategy / omniscript
- trading / exchange / balances / positions / orders

## Blockers

- `any` type usage
- `console.log` in production code
- business logic in tRPC procedures
- direct Prisma / database access outside repositories
- schema changes without migration files
- magic numbers or strings not extracted to constants
- dead code or commented-out code
- `@ts-ignore` or `@ts-expect-error`
- non-semantic clickable `div` / `span`
- hardcoded configuration values

## Warnings

- missing return types on exported functions
- type-only imports without `type`
- `TODO` comments without issue reference
- missing async error handling
- overly complex functions
- AI-written or stilted documentation / UI copy

## Required output sections

1. `### Code Review Summary`
2. `#### Blockers`
3. `#### Warnings`
4. `#### Architecture Compliance`
5. `#### Anti-Pattern Scan`
6. `#### Quality Assessment`
7. `#### Scope Check`
8. final `**Verdict:** ...`
