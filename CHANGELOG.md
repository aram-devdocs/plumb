# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

From the first release onward, this file is maintained automatically by [`release-please`](https://github.com/googleapis/release-please) based on [Conventional Commits](https://www.conventionalcommits.org/) on `main`. Do not edit released sections by hand.

## [Unreleased]

### Added

- SARIF output now includes built-in rule metadata, canonical rule `helpUri` links, and Code Scanning-compatible result locations.
- Pretty and JSON formatter output now include deterministic stats with severity counts, viewport count, rule count, and a content-hashed run id. Pretty output now groups violations by viewport, then rule, then selector.
- Repository hooks now enforce the talking-stick workflow for agent-driven changes.
- Initial workspace scaffold, tooling, and walking skeleton.
- PRD-style `[spacing]` and `[type]` config sections with schema validation.
- `plumb mcp` `lint_url` now accepts an optional `detail` argument. The default `compact` mode preserves the existing MCP payload, while `detail: "full"` returns the canonical full JSON envelope and rejects structured payloads above 50 KB.
- `plumb mcp` now exposes a `plumb://config` resource that returns the resolved `plumb.toml` for the server working directory as JSON.
- Rule `spacing/grid-conformance`: flags `margin-*`, `padding-*`, `gap`, `row-gap`, and `column-gap` values that aren't multiples of `spacing.base_unit`.
- Rule `spacing/scale-conformance`: flags the same property set when values aren't members of `spacing.scale`.
- Rule `type/scale-conformance`: flags `font-size` values that aren't members of `type.scale`.
- PRD §12.2 `[color]`, `[radius]`, `[alignment]`, `[a11y]` config sections fleshed out: `color.delta_e_tolerance` (default 2.0), `alignment.tolerance_px` (default 3), `a11y.touch_target.{min_width_px, min_height_px}` (default 24×24 per WCAG 2.5.8).
- DTCG 2025.10 token adapter in `plumb-config`: `merge_dtcg(&mut Config, &DtcgSource)` imports a Design Tokens Community Group JSON file into a `Config`. Maps `color`, `dimension` (spacing or typography by namespace heuristic), `fontFamily`, `fontWeight`, and `radius` / `borderRadius`; resolves `{path.to.token}` brace aliases and `{ "$ref": "#/..." }` pointers with cycle detection; caps nesting at 64 levels.
- `plumb-cdp::PersistentBrowser`: long-lived Chromium handle that warms once per MCP session and gives every `snapshot` call a fresh incognito `BrowserContext`. The MCP server's `lint_url` tool uses it for real `http(s)://` URLs so back-to-back lints reuse the warm browser without leaking cookies or storage between calls. The `plumb-fake://` fast path stays browser-free.

### Changed

- Renamed `radius.allowed_px` to `radius.scale` for naming consistency with `spacing.scale` and `type.scale`. The old name is rejected; update any pre-existing `plumb.toml`.
- `plumb-cdp` accepts a Chromium major-version range
  (`MIN_SUPPORTED_CHROMIUM_MAJOR..=MAX_SUPPORTED_CHROMIUM_MAJOR`,
  currently `131..=150`) instead of an exact pin on `131`. The
  `CdpError::UnsupportedChromium` variant now carries `min_supported`,
  `max_supported`, and `found` fields, and the install hint reflects the
  range. This unblocks `plumb lint <real-url>` on any host running a
  recent Chrome / Chromium build.
- `plumb-cdp`'s `e2e-chromium` tests no longer silently skip when
  Chromium is missing or out-of-range. They now hard-fail unless the
  user opts in via `PLUMB_E2E_CHROMIUM_SKIP=1`, in which case the skip
  is logged via `tracing::warn!`.

### Removed

- Walking-skeleton placeholder rule `placeholder/hello-world` and its docs.
