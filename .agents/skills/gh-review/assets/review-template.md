### Code review summary

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

#### Architecture compliance

- [{{LAYER_IMPORTS}}] No new `unsafe` outside `plumb-cdp`
- [{{ERROR_TYPES}}] No new `unwrap` / `expect` / `panic!` in library crates (thiserror in libs, anyhow only in `plumb-cli::main`)
- [{{OUTPUT_DISCIPLINE}}] No new `println!` / `eprintln!` outside `plumb-cli`
- [{{DETERMINISM}}] No new wall-clock or `HashMap` in observable-output paths
- [{{NO_DEBUG_MACROS}}] No new `todo!` / `unimplemented!` / `dbg!`

#### Anti-pattern scan

| Pattern | Status | Details |
|---------|--------|---------|
{{ANTI_PATTERNS}}

#### Quality assessment

{{QUALITY_ASSESSMENT}}

#### Scope check

- Changes match PR description: {{MATCHES_DESCRIPTION}}
- Scope creep detected: {{SCOPE_CREEP}}

---

**Verdict:** {{VERDICT}}
