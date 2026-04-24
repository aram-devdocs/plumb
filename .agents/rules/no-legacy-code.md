# Rule: No legacy, deprecated, or unused code

Plumb is an AI-driven project. Dead weight in the tree — deprecated
items without an exit plan, commented-out blocks, orphaned
compatibility shims, unused functions — costs token budget for every
agent that reads the repo and confuses humans who read it later. The
policy is simple: only keep what is used. When something stops being
used, delete it in the same PR that removed its last caller.

## What's banned

- **Indefinite `#[deprecated]`.** Every `#[deprecated]` attribute MUST
  carry a tracking-issue link and a concrete removal milestone in its
  `note`. `note = "will remove eventually"` is not acceptable. Neither
  is `note = "removed when X lands"` without a GitHub issue number.
- **Commented-out code.** If the code is off, delete it. Git history is
  the archive. `// old impl:` blocks, `/* TODO: restore this later */`,
  and `if false { … }` dead branches are all rejected at review.
- **`// TODO: remove later`** and similar open-ended removal markers.
  Every TODO that promises future deletion MUST reference a tracking
  issue (`// TODO(#123): remove when config v2 lands`).
- **Orphaned compatibility shims.** Type aliases, re-exports, and
  wrapper functions kept "just in case" — if nothing internal uses them
  and no external crate published against them exists, they go.
- **Unused imports, functions, fields, and enum variants.** These are
  caught by the compiler under the lints below and fail CI.
- **Renamed-to-`_` parameters or fields for backwards compatibility.**
  If a parameter is unused, remove it. Don't prefix with `_` to silence
  the warning — fix the call site instead.

## What's required

- **Deprecation plan of record.** The PR that adds a `#[deprecated]`
  item MUST also file or link the tracking issue that owns its removal
  milestone. The removal milestone is concrete: a release (`0.2.0`), a
  ticket (`#123`), or a dependent feature landing (`after #456 merges`).
- **Atomic removal PRs.** When the removal milestone is hit, the PR
  that removes the deprecated item MUST also delete in the same commit
  range:
  - Its tests (unit, integration, golden snapshots).
  - Its documentation (`docs/src/**`, rustdoc examples that reference
    it, any `explain_rule` markdown).
  - Its registration (`register_builtin`, tool descriptor, re-exports).
  - Any `#[allow(deprecated)]` attributes that existed to silence its
    usage.
- **No drive-by deprecations.** Deprecating an item is a scoped change.
  Don't bundle a deprecation with unrelated feature work — it hides the
  removal clock inside an unrelated PR title.

## How it's enforced

- **Workspace `[lints.rust]`** in the root `Cargo.toml`:
  - `dead_code = "deny"` — unused private functions, fields, and enum
    variants fail the build.
  - `unused_imports = "deny"` — orphaned `use` statements fail the
    build.
  - `deprecated = "deny"` — usage of any `#[deprecated]` item fails the
    build unless the consumer has an explicit `#[allow(deprecated)]`
    scoped to the exact registration site.
- **CI `RUSTFLAGS: -Dwarnings`** in `.github/workflows/ci.yml` — every
  remaining warn-level lint (`unused_variables`, `unused_mut`,
  `unused_must_use`, etc.) is promoted to an error on every PR.
- **Clippy `-D warnings`** in the preflight job — the same promotion
  applies to clippy's unused-argument and redundant-clone lints.
- **Review gate.** Human and AI reviewers MUST flag commented-out code,
  open-ended TODOs, and orphaned shims. The 03-code-quality-reviewer
  agent treats any of the above as a blocker.

## The `#[allow(deprecated)]` escape hatch

Registering a deprecated item inside the crate that owns it requires a
local `#[allow(deprecated)]`. That attribute MUST be scoped as tightly
as possible — a single `impl` block or `vec![…]` literal, not a whole
module. Every occurrence of `#[allow(deprecated)]` is a marker for the
removal PR to delete.

## Anti-patterns

- Keeping a "v1" API alongside a "v2" API "for migration." Either
  migrate internal callers in the same PR that introduces v2, or don't
  add v2 yet.
- Leaving a `#[cfg(feature = "legacy")]` gate with no sunset plan.
  Feature gates are for forward-looking optionality, not backwards
  compatibility archaeology.
- Silencing `dead_code` with `#[allow(dead_code)]` on a top-level item.
  If it's truly unused, delete it. If it's used in tests only, gate it
  behind `#[cfg(test)]`.
