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
