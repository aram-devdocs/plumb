# Security Policy

## Supported versions

Plumb is pre-1.0; only the latest `0.x` release line receives security fixes.

| Version | Supported |
|---------|-----------|
| latest  | yes       |
| older   | no        |

## Reporting a vulnerability

**Do not open a public issue for security reports.**

Please use [GitHub Security Advisories](https://github.com/aram-devdocs/plumb/security/advisories/new) to submit a private report. Include:

- A description of the issue and its impact.
- Steps to reproduce, or a proof-of-concept.
- Affected versions.
- Suggested fix, if known.

We aim to acknowledge reports within 72 hours and issue a fix within 90 days of acknowledgment. Reporters are credited in the advisory unless they request anonymity.

## Scope

In scope:

- The `plumb` binary and the `plumb-*` crates.
- The MCP server's tool-call handlers and input validation.
- The rule engine's handling of untrusted URLs and HTML content.

Out of scope:

- Vulnerabilities in Chromium (report upstream).
- Vulnerabilities in third-party Rust crates Plumb depends on (report upstream, then notify us).
- Social engineering or physical attacks on maintainers.
