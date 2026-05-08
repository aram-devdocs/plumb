# Security

Found a vulnerability in Plumb? Report it privately.

## Reporting

Open a private report through [GitHub Security Advisories](https://github.com/aram-devdocs/plumb/security/advisories/new). Do not file a public issue or post details in chat.

Please include:

- A description of the issue and its impact.
- Steps to reproduce, or a proof-of-concept.
- Affected versions.
- A suggested fix, if you have one.

## Service-level

- **Acknowledgment:** within 72 hours of the report.
- **Fix:** within 90 days of acknowledgment, with status updates if the fix is going to take longer.
- **Credit:** reporters are named in the advisory unless they ask for anonymity.

## Supported versions

Plumb is pre-1.0. Only the latest `0.x` release line receives security fixes. See the [versioning policy](./versioning.md) for the broader stability story.

## Scope

In scope:

- The `plumb` binary and the `plumb-*` crates.
- The MCP server's tool-call handlers and input validation.
- The rule engine's handling of untrusted URLs and HTML content.
- Install scripts shipped from this repo.

Out of scope:

- Vulnerabilities in Chromium itself — report upstream.
- Vulnerabilities in third-party Rust crates Plumb depends on — report upstream first, then notify us.
- Violations the linter emits about user code; those are by design.
- Social engineering or physical attacks on maintainers.

The full policy lives in [`SECURITY.md`](https://github.com/aram-devdocs/plumb/blob/main/SECURITY.md) at the repo root.
