# ADR 0006 — SLSA attestation verification path

**Status:** Accepted
**Date:** 2026-05-07
**Deciders:** Aram Hammoudeh

## Context

A pre-share audit of the public install story raised a flag: the
endpoint `GET /repos/aram-devdocs/plumb/attestations` returns
`404 Not Found`. The release workflow uses
[`actions/attest-build-provenance@v4`](https://github.com/actions/attest-build-provenance)
twice (in the `build` and `installers` jobs) with both
`id-token: write` and `attestations: write` permissions, so the audit
finding implied a configuration gap.

Investigation against the v0.0.11 release showed the opposite: the
attestations are generated, signed, indexed, and verifiable. The
audit's 404 came from querying a path that GitHub does not expose as
a list endpoint. This ADR records the actual verification path so
future audits, runbooks, and install docs target the right shape of
the API.

## What the investigation found

For the v0.0.11 release:

- `GET /repos/aram-devdocs/plumb/attestations` — `404`.
  This is not a list endpoint and never was. There is no public way to
  enumerate every attestation a repository owns.
- `GET /repos/aram-devdocs/plumb/attestations/sha256:<digest>` — works.
  Returns the full sigstore bundle(s) for a given artifact. For
  binaries this returns 2 attestations (one from the `build` job, one
  from the `installers` job that re-attests `plumb-cli*`); for the
  npm package it returns 1.
- `gh attestation verify <asset> --repo aram-devdocs/plumb` — succeeds
  for every release asset tested (darwin tar.xz, linux gnu tar.xz,
  installer.sh, npm-package.tar.gz). Output prints
  `Verification succeeded!` and lists the matching attestations.
- `gh attestation download <asset> --repo aram-devdocs/plumb` — writes
  the bundle(s) to `sha256:<digest>.jsonl` in the current directory.
  The file path is fixed by `gh`; there is no `--output-file` flag.
- `gh attestation verify <asset> --bundle <digest>.jsonl --repo …` —
  the canonical offline verification path, using a previously
  downloaded bundle.

## Decisions

### 1. Document by-digest verification, not list enumeration

Public install docs (`docs/src/install.md`) describe the verification
path as `gh attestation verify <asset> --repo aram-devdocs/plumb`.
There is no documented list-endpoint workflow because GitHub does not
expose one. Treat any future audit finding of "the bare
`/attestations` endpoint 404s" as expected behavior, not a gap.

### 2. Keep both attestation jobs

The release workflow attests artifacts in both the `build` job
(per-target binaries via `subject-path: target/distrib/*`) and the
`installers` job (installer scripts, Homebrew formula, npm package
via `subject-path: target/distrib/plumb-cli*`). This produces 1–2
attestations per asset depending on which job emitted it. We keep
both: the build-job attestation pins the binary to its build runner,
the installers-job attestation pins the wrapping script that consumers
download to its build runner.

### 3. `gh attestation verify --bundle` is the offline path

When users want network-isolated verification, the documented path is
`gh attestation download` followed by `gh attestation verify --bundle`.
We keep a `cosign` reference in the docs but treat `gh` as primary
because (a) the JSONL file may contain multiple bundles, which `gh`
handles natively and `cosign` does not, and (b) users who already
trust the `gh` binary do not need to install a second tool.

### 4. Asset name corrections in install.md

The install docs previously referenced `plumb-x86_64-…` filenames.
The actual cargo-dist output is `plumb-cli-x86_64-…`. The docs are
corrected to match what users will see on the release page.

## Consequences

- The audit-style query `gh api repos/<owner>/<repo>/attestations`
  is not a meaningful health check. Future readiness checks must
  query by digest (against a known release asset) or run
  `gh attestation verify` end-to-end.
- Coverage table in `install.md` now lists every asset class that
  ships with an attestation: platform archives, installer scripts,
  Homebrew formula, npm package.
- Offline verification is a `gh`-only path in the documented flow;
  cosign remains a footnote for users who prefer it.

## See also

- `.github/workflows/release.yml` — `build` and `installers` jobs
  invoke `actions/attest-build-provenance@v4`.
- `docs/src/install.md` — user-facing verification instructions.
- [GitHub Docs — Using artifact attestations to establish provenance for builds](https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations/using-artifact-attestations-to-establish-provenance-for-builds).
