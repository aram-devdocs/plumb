## Target Branch

> **IMPORTANT:** All PRs MUST target the `dev` branch, NOT `main`.
> PRs to `main` are only allowed for production releases from `dev`.

- [x] This PR targets the `dev` branch

## Type of Change

- [ ] Bug fix (non-breaking change fixing an issue)
- [ ] New feature (non-breaking change adding functionality)
- [ ] Refactor (non-breaking change improving code structure)
- [ ] Performance (optimization or performance improvement)
- [ ] Security (security fix or enhancement)
- [ ] Infrastructure (CI/CD, build, deployment changes)
- [ ] Documentation
- [ ] Tests
- [ ] Chore (dependency updates, tooling)

## Summary

{{SUMMARY}}

## Related Issues

{{RELATED_ISSUES}}

## Changes

### Added

{{ADDED}}

### Modified

{{MODIFIED}}

### Removed

{{REMOVED}}

## Affected Layers

- [ ] L1 Core (constants, utils, logger)
- [ ] L2 Data (types, brand, feature-flags)
- [ ] L3 Infrastructure (database, repositories, omniscript)
- [ ] L4 Business (services, strategy-engine, emails)
- [ ] L5 Integration (api, hooks, ui, seo)
- [ ] L6 Apps (server, web, web-root)

## System Impact

- [ ] Database schema changes
- [ ] API contract changes
- [ ] Authentication/Authorization
- [ ] UI/UX changes
- [ ] Infrastructure/CI/CD
- [ ] External integrations (exchange APIs, etc.)
- [ ] Omniscript compiler/Strategy engine

## Architectural Compliance

- [ ] Import direction follows L1-L6 layer rules (downward only)
- [ ] No cross-app or web/backend boundary imports
- [ ] Type definitions in `@omnifol/types` (not local)
- [ ] Zod schemas for external data validation
- [ ] Business logic in services, not tRPC procedures or UI
- [ ] Database access through repositories only
- [ ] Logger used instead of console.log

## Testing

### Automated

- [ ] Unit tests added/updated
- [ ] Integration tests added/updated
- [ ] E2E tests added/updated (if applicable)

### Manual

- [ ] Manual testing completed
- [ ] Tested on smallest breakpoint (if UI)

### Security (if applicable)

- [ ] Auth flows tested
- [ ] Input validation verified
- [ ] No sensitive data exposure

## Code Quality

### Pre-commit checks

- [ ] `pnpm typecheck` passes
- [ ] `pnpm lint` passes
- [ ] `pnpm turbo run build` succeeds
- [ ] `pnpm validate:architecture` passes

### Code review checklist

- [ ] No `any` types or `@ts-ignore`
- [ ] No console.log (use logger)
- [ ] No magic numbers/strings (use constants)
- [ ] No commented-out or dead code
- [ ] No hardcoded config values
- [ ] Semantic HTML (no div with onClick)
- [ ] WCAG AA accessible (keyboard, ARIA, contrast)
- [ ] Exhaustive switch statements for unions

## Documentation

- [ ] CLAUDE.md or rules updated (if patterns changed)
- [ ] Inline comments explain "why" not "what"
- [ ] Migration guide provided (if breaking changes)

## Breaking Changes

- [ ] No breaking changes
- [ ] Yes, breaking changes (describe below)

{{BREAKING_CHANGES}}

## Deployment Considerations

- [ ] No special deployment steps
- [ ] Environment variable changes
- [ ] Database migration required (`pnpm db:migrate`)
- [ ] Feature flag changes
- [ ] Other (describe below)

{{DEPLOYMENT_CONSIDERATIONS}}

## Security Implications

- [ ] No security implications
- [ ] Yes (describe below)

{{SECURITY_IMPLICATIONS}}

## Performance Impact

- [ ] No performance impact
- [ ] Performance improvement (describe metrics)
- [ ] Potential performance degradation (describe and justify)

{{PERFORMANCE_IMPACT}}

## Screenshots

{{SCREENSHOTS}}

## Reviewer Notes

{{REVIEWER_NOTES}}
