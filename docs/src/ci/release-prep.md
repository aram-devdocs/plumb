# Release Prep

This page records release work that is intentionally staged ahead of
live distribution changes.

## Homebrew tap activation

Issue #51 is wired in this repo state. `dist-workspace.toml` sets
`tap = "aram-devdocs/homebrew-plumb"`, and `cargo dist host` in
`release.yml` pushes the formula to that tap on each release tag using
the `HOMEBREW_TAP_TOKEN` repo secret.

Live verification still depends on the first tag-driven release. Until
that release publishes successfully and `brew install
aram-devdocs/plumb/plumb` is verified end to end on macOS and Linux, the
install-smoke `brew` legs stay gated and the docs MUST NOT claim
`brew install` is a verified install path.

Channel maintenance steps:

1. Confirm `cargo dist plan` still includes the Homebrew artifacts when
   the dist version or installer list changes.
2. Keep the `aram-devdocs/homebrew-plumb` tap repo settings in shape for
   cargo-dist formula updates (default branch, branch protection that
   accepts the tap PR).
3. Rotate `HOMEBREW_TAP_TOKEN` if the publishing identity changes.
4. After the first successful tag-driven publish, ungate the `brew`
   channel in `.github/workflows/install-smoke.yml` and update the
   gating expectations in `tests/install-smoke-validate.sh` in the same
   PR.

## npm activation for `@plumb/cli`

Issue #52 is also prep-only in this repo state. The release workflow and
`dist-workspace.toml` may validate the cargo-dist config shape, but they
do not enable npm publishing yet.

Before anyone uncomments the `npm-scope` setting in
`dist-workspace.toml`, these prerequisites MUST exist:

1. The `@plumb` npm scope.
2. Ownership or publish access for the `@plumb/cli` package under the
   release identity used by this repo.
3. The `NPM_TOKEN` secret and any required repo or environment
   permissions for release publishing.

Activation steps, once the external prerequisites exist:

1. Confirm `cargo dist plan` still passes with the current repo state.
2. Verify the commented config shape still matches the intended package
   name: `npm-scope = "@plumb"` targets `@plumb/cli` for this binary.
3. Add `npm` to the `installers` list when enabling npm publishing.
   In cargo-dist 0.28.0, uncommenting `npm-scope` alone does not emit
   npm artifacts.
4. Add the `NPM_TOKEN` secret and any required GitHub repo or
   environment permissions for cargo-dist publishing.
5. Uncomment `npm-scope = "@plumb"` in `dist-workspace.toml`.
6. Run `cargo dist plan` again and review the generated manifest for npm
   installer entries before tagging a release.
7. Publish from a controlled release tag and verify the live install on
   macOS, Linux, and Windows before updating docs or acceptance claims.

Current blockers this repo does not solve:

- The `@plumb` npm scope may not exist yet.
- Ownership of `@plumb/cli` is external to this repo.
- `NPM_TOKEN` and the required repo or environment permissions are
  external to this repo.

Until those blockers are resolved, this repo MUST NOT claim that
`npm i -g @plumb/cli` has been verified.
