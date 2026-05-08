# Versioning

Plumb is pre-1.0. The current release line is `0.0.x`. This page documents what callers can rely on now and what the `0.1.0` and `1.0.0` milestones will commit to.

The keywords MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are used per [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) when describing the contract.

## Today: 0.0.x

Treat every minor bump as potentially breaking. Releases follow [release-please](https://github.com/googleapis/release-please) and conventional commits, but there is no SemVer commitment yet.

### What is stable

The following surfaces are stable across `0.0.x` releases. A change that breaks any of them MUST be called out in the release notes.

- **Rule IDs.** A rule keeps its `<category>/<id>` slug for the lifetime of the rule. Renaming a rule is a breaking change and is announced in the changelog.
- **MCP tool names and required arguments.** `lint_url`, `lint_page_html`, `explain_rule`, `list_rules`, `get_config`, `compare_viewports`, and `echo` keep their names and required argument keys. New optional arguments MAY be added.
- **CLI exit codes.** `0` for clean, `1` for violations at the configured fail level, `2` for usage errors, `3` for runtime errors (driver launch failure, bad config). A future change to these values is a breaking change.
- **Output envelope shape.** The top-level keys in `--format json` (`plumb_version`, `run_id`, `stats`, `summary`, `violations`) keep their meaning. Per-violation fields MAY gain new keys; existing keys MUST NOT change type or be removed without a major-line bump.

### What may change

The following are NOT a public API and MAY change in any release.

- **Inter-rule violation ordering.** Violations are sorted deterministically within a `(rule_id, viewport)` group, but the relative order of different rules MAY change as new rules are added.
- **Snapshot internals.** The internal snapshot representation that rules consume is an implementation detail. Do not parse it from outside `plumb-core`.
- **Default config values.** Defaults for tolerances, viewport sets, and severity levels MAY shift on minor bumps. Pin values you depend on in `plumb.toml`.
- **Diagnostic text.** The human-readable `text` block in MCP responses and the pretty CLI output are formatted for humans; their wording MAY change. Parse `structuredContent` or `--format json` instead.

## 0.1.0 milestone

`0.1.0` is the first version that publishes a stability commitment. It will ship when:

- The built-in rule catalog is considered feature-complete for the v1 design-system surface (spacing, color, type, radius, shadow, opacity, z-index, sibling consistency, edge alignment, baseline rhythm, touch target, contrast).
- The MCP protocol surface is considered closed for the same scope: no new required arguments are planned for existing tools.
- The CLI surface (`plumb lint`, `plumb mcp`, `plumb explain`, `plumb init`, `plumb generate-config-schema`) is documented and tested end to end.

From `0.1.0` onward:

- Rule renames or removals MUST go through a deprecation cycle (one minor release with `#[deprecated]` carrying a tracking issue and a removal milestone, per [`.agents/rules/no-legacy-code.md`](https://github.com/aram-devdocs/plumb/blob/main/.agents/rules/no-legacy-code.md)).
- Required MCP tool arguments MUST NOT be added or removed within a `0.1.x` line.
- Exit-code semantics MUST NOT change within a `0.1.x` line.

Defaults and inter-rule ordering remain "may change" between minor versions until `1.0.0`.

## 1.0.0 milestone

`1.0.0` is the SemVer commitment. It will ship when the project has at least two consecutive minor releases without a planned breaking change to the surfaces listed above, and when the determinism CI gate has covered every rule in the catalog for two release cycles.

From `1.0.0` onward Plumb follows [Semantic Versioning 2.0](https://semver.org/spec/v2.0.0.html):

- A breaking change to any rule ID, MCP tool argument, exit code, or `--format json` envelope key requires a major version bump.
- New rules, new MCP tools, and new optional arguments are minor bumps.
- Bug fixes and non-breaking documentation changes are patch bumps.

Until `1.0.0` ships, callers SHOULD pin to an exact version in CI and review the changelog before bumping.

## See also

- [Security policy](./security.md) — supported versions for security fixes.
- [`SECURITY.md`](https://github.com/aram-devdocs/plumb/blob/main/SECURITY.md) — full security policy.
- [Release notes on GitHub](https://github.com/aram-devdocs/plumb/releases) — per-version changes.
