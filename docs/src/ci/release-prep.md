# Release Prep

This page records release work that is intentionally staged ahead of
live distribution changes.

## Homebrew tap activation

Issue #51 is prep-only in this repo state. The release workflow and
`dist-workspace.toml` already validate the cargo-dist setup, but they do
not enable Homebrew publishing yet.

Before anyone uncomments the `tap` setting in `dist-workspace.toml`,
these prerequisites MUST exist:

1. The `plumb-dev` GitHub organization.
2. The `plumb-dev/homebrew-tap` repository.
3. The token and GitHub permissions cargo-dist needs to update the tap
   on release.

Activation steps, once the external prerequisites exist:

1. Confirm `cargo dist plan` still passes with the current repo state.
2. Create or verify the `plumb-dev/homebrew-tap` repo settings that
   cargo-dist expects for formula updates.
3. Add the release token or permissions required for cargo-dist to push
   Homebrew formula changes.
4. Uncomment `tap = "plumb-dev/homebrew-tap"` in
   `dist-workspace.toml`.
5. Run `cargo dist plan` again and review the generated manifest for the
   Homebrew artifacts.
6. Cut a tag-driven dry run in GitHub Actions before claiming
   `brew install` support.

Current blockers this repo does not solve:

- `plumb-dev` org does not exist here.
- `plumb-dev/homebrew-tap` is not available here.
- cargo-dist publishing credentials and permissions are external to this
  repo.

Until those blockers are resolved, the install docs describe the
Homebrew command shape, but this repo MUST NOT claim `brew install`
verification.

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
