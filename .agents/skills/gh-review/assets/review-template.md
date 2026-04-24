### Code Review Summary

**PR:** {{PR}}
**Author:** {{AUTHOR}}
**Base:** {{BASE}}

---

#### Blockers

| # | Severity | File | Line | Issue | Suggestion |
|---|----------|------|------|-------|------------|
{{BLOCKERS}}

#### Warnings

| # | Severity | File | Line | Issue | Suggestion |
|---|----------|------|------|-------|------------|
{{WARNINGS}}

#### Architecture Compliance

- [{{LAYER_IMPORTS}}] Layer imports follow L1-L6 hierarchy
- [{{BOUNDARIES}}] No cross-app or web/backend boundary violations
- [{{BUSINESS_LOGIC}}] Business logic in correct layer
- [{{DATABASE_ACCESS}}] Database access through repositories
- [{{TYPES}}] Types defined in `@omnifol/types`

#### Anti-Pattern Scan

| Pattern | Status | Details |
|---------|--------|---------|
{{ANTI_PATTERNS}}

#### Quality Assessment

{{QUALITY_ASSESSMENT}}

#### Scope Check

- Changes match PR description: {{MATCHES_DESCRIPTION}}
- Scope creep detected: {{SCOPE_CREEP}}

---

**Verdict:** {{VERDICT}}
