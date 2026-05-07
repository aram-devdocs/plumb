# Release Prep

This page records release work that is intentionally staged ahead of
live distribution changes.

## Homebrew tap activation

Issue #51 is verified in this repo state. `dist-workspace.toml` sets
`tap = "aram-devdocs/homebrew-plumb"`, and `cargo dist host` in
`release.yml` pushes the formula to that tap on each release tag using
the `HOMEBREW_TAP_TOKEN` repo secret.

The v0.0.11 tag-driven release verified `brew install
aram-devdocs/plumb/plumb` end to end on macOS and Linux, so the
install-smoke `brew` legs run live alongside the other channels.

Channel maintenance steps:

1. Confirm `cargo dist plan` still includes the Homebrew artifacts when
   the dist version or installer list changes.
2. Keep the `aram-devdocs/homebrew-plumb` tap repo settings in shape for
   cargo-dist formula updates (default branch, branch protection that
   accepts the tap PR).
3. Rotate `HOMEBREW_TAP_TOKEN` if the publishing identity changes.
4. Treat brew-channel breakage like any other non-gated install-smoke
   failure.

## npm activation for `plumb-cli`

Issue #52 is wired in this repo state. `dist-workspace.toml` includes
`npm` in the `installers` list with no `npm-scope` set, so cargo-dist
0.28.0 emits an unscoped `plumb-cli` package that publishes to the npm
account that owns the repo's `NPM_TOKEN` secret. The public install
command is `npm i -g plumb-cli`.

The install-smoke `npm` legs run non-manually on ubuntu, macOS, and
windows. Live verification still depends on the first tag-driven
release: until that release publishes successfully and the legs pass
end to end, treat npm-channel breakage like any other non-gated
install-smoke failure.

Channel maintenance steps:

1. Confirm `cargo dist plan` still includes the
   `plumb-cli-npm-package.tar.gz` artifact when the dist version or
   installer list changes.
2. Keep the `NPM_TOKEN` secret valid for the release-publishing
   identity. Rotate when that identity changes.
3. If the package later moves to an org-scoped name, set `npm-scope`
   in `dist-workspace.toml` and update the docs/install-smoke shape in
   the same PR.
