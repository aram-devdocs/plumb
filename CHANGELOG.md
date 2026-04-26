# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

From the first release onward, this file is maintained automatically by [`release-please`](https://github.com/googleapis/release-please) based on [Conventional Commits](https://www.conventionalcommits.org/) on `main`. Do not edit released sections by hand.

## [0.0.2](https://github.com/aram-devdocs/plumb/compare/v0.0.1...v0.0.2) (2026-04-26)


### Features

* bootstrap workspace, walking skeleton, and tooling ([0ad9924](https://github.com/aram-devdocs/plumb/commit/0ad9924aa27f4d00c78bc36fb634c4057060adaa))
* **cdp:** add Chromium launch detection ([#94](https://github.com/aram-devdocs/plumb/issues/94)) ([ce9afb9](https://github.com/aram-devdocs/plumb/commit/ce9afb91eb954716a368d48078d158535e4572da))
* **cdp:** wire DOMSnapshot.captureSnapshot with style whitelist ([#104](https://github.com/aram-devdocs/plumb/issues/104)) ([d062a21](https://github.com/aram-devdocs/plumb/commit/d062a21f8cd218f720596d2126c6161c5b2adb18))
* **cli:** --selector flag scoping lint to subtree ([#127](https://github.com/aram-devdocs/plumb/issues/127)) ([f61428b](https://github.com/aram-devdocs/plumb/commit/f61428b794df6c8deac8b87f4b9f4ae66d1df9db))
* **cli,core,cdp:** --viewport flag + viewport multi-run orchestration ([#103](https://github.com/aram-devdocs/plumb/issues/103)) ([0b7ee6e](https://github.com/aram-devdocs/plumb/commit/0b7ee6e7bb16c39444b77c7f3a761614d3712622))
* **cli,format:** deterministic JSON envelope with plumb_version, run_id, summary ([#123](https://github.com/aram-devdocs/plumb/issues/123)) ([8f2d9c3](https://github.com/aram-devdocs/plumb/commit/8f2d9c3bd8a4a531411a5b14712975ba84b8c6c0))
* **cli:** plumb init with Tailwind detection ([#33](https://github.com/aram-devdocs/plumb/issues/33)) ([#128](https://github.com/aram-devdocs/plumb/issues/128)) ([90fdd50](https://github.com/aram-devdocs/plumb/commit/90fdd5017f6c008d0abb56cdac03c6278cdb8a19))
* **config:** [color], [radius], [alignment], [a11y] sections + schema ([#109](https://github.com/aram-devdocs/plumb/issues/109)) ([f5b0047](https://github.com/aram-devdocs/plumb/commit/f5b004773dd40cf21f00e81f8a690b85ca70ee6a)), closes [#21](https://github.com/aram-devdocs/plumb/issues/21)
* **config:** add CSS custom-properties scraper ([#114](https://github.com/aram-devdocs/plumb/issues/114)) ([b6dbde1](https://github.com/aram-devdocs/plumb/commit/b6dbde10ed18cfa559ddba5d8931309da1b5bb36))
* **config:** add DTCG 2025.10 token adapter ([#111](https://github.com/aram-devdocs/plumb/issues/111)) ([778e212](https://github.com/aram-devdocs/plumb/commit/778e212318181d65edb3124827c6ad8004703128))
* **config:** add spacing and type schema sections ([#91](https://github.com/aram-devdocs/plumb/issues/91)) ([a649708](https://github.com/aram-devdocs/plumb/commit/a649708f4d9e7ad46e9f3ba1886a0c6c32bcd7fc))
* **config:** span-annotated validation errors via miette ([#110](https://github.com/aram-devdocs/plumb/issues/110)) ([6ea352d](https://github.com/aram-devdocs/plumb/commit/6ea352d4d04cc856663b13631294998b3c13ecf0))
* **config:** tailwind config adapter ([#115](https://github.com/aram-devdocs/plumb/issues/115)) ([36d2c54](https://github.com/aram-devdocs/plumb/commit/36d2c5420a7a51ee598c33d6c987c4b7be6b1594))
* **core:** bundle MVP rules (radius/a11y/sibling/edge) ([#113](https://github.com/aram-devdocs/plumb/issues/113)) ([2c54e94](https://github.com/aram-devdocs/plumb/commit/2c54e947f64230d59254e6d40cc36f7b18428e18))
* **core:** enrich snapshot context ([#93](https://github.com/aram-devdocs/plumb/issues/93)) ([b0a592d](https://github.com/aram-devdocs/plumb/commit/b0a592dd522aed156a0e5c05eb28e867eb8b42f4))
* **core:** rule color/palette-conformance ([#112](https://github.com/aram-devdocs/plumb/issues/112)) ([0220e55](https://github.com/aram-devdocs/plumb/commit/0220e55e7edb6b6273571b6acd3a62ce255f803e))
* **core:** rules spacing/grid + spacing/scale + type/scale ([#108](https://github.com/aram-devdocs/plumb/issues/108)) ([b00b5e6](https://github.com/aram-devdocs/plumb/commit/b00b5e661941e7dba4fc0c21d6d2cd01ca921287))
* **gh-issue:** Phase 7/8 — mandatory PR + converge on CI green + Claude review approve ([d2ff65e](https://github.com/aram-devdocs/plumb/commit/d2ff65e32203779729eee1b5b5ca6de0172fb701))
* **gh-runbook:** adaptive dispatch — recommend split/bundle/cluster per batch ([1e3f9fd](https://github.com/aram-devdocs/plumb/commit/1e3f9fd322fc30f764154731851b7c7cf49e455f))
* **gh-runbook:** gate-based progression with parallel batches + per-ticket dispatch ([ccc3fb8](https://github.com/aram-devdocs/plumb/commit/ccc3fb8f4e8d36f210c7cbc3c0e363306bc97bfd))
* **gh-runbook:** GoudEngine-style parent bodies — batch table + copy-pasteable /gh-issue commands ([2ff501b](https://github.com/aram-devdocs/plumb/commit/2ff501b8b6c89e2a496ef55afbfe5a4ae5fb87d3))
* **mcp:** add explain_rule tool ([#130](https://github.com/aram-devdocs/plumb/issues/130)) ([4bf8237](https://github.com/aram-devdocs/plumb/commit/4bf8237f4663b8e78de545c8890edb1c0d52503c)), closes [#35](https://github.com/aram-devdocs/plumb/issues/35)
* **mcp:** lint_url accepts real http(s) URLs via ChromiumDriver ([#131](https://github.com/aram-devdocs/plumb/issues/131)) ([2924d5d](https://github.com/aram-devdocs/plumb/commit/2924d5d7486e371457f3eff9443787056f1c42ba))
* ship every deferred pattern — skills, hooks, workflows, rmcp, xtask ([561bd45](https://github.com/aram-devdocs/plumb/commit/561bd454a1eb29b023e8d43d076c2e583f1e5ccd))
* **skills:** rewrite gh-runbook as spec-driven generator + JSON Schema ([91c4180](https://github.com/aram-devdocs/plumb/commit/91c41808097b71dc508959c6805a3d174d6b048e))
* **xtask:** validate-runbooks subcommand ([7ca7bd5](https://github.com/aram-devdocs/plumb/commit/7ca7bd5afc4b34fdb84b64b0a536d512ec40b364))


### Bug Fixes

* **cdp:** accept Chromium major-version range; e2e tests fail loud ([#126](https://github.com/aram-devdocs/plumb/issues/126)) ([2a5d1e2](https://github.com/aram-devdocs/plumb/commit/2a5d1e2a5a29f83f06c383e4c7b78f3153c28320))
* **cdp:** scale FakeDriver viewport rects to configured target ([#125](https://github.com/aram-devdocs/plumb/issues/125)) ([c0a853a](https://github.com/aram-devdocs/plumb/commit/c0a853ab65c4b40ed277e5cde7cb27f570c48c82)), closes [#121](https://github.com/aram-devdocs/plumb/issues/121)
* **ci:** drop cargo-workspace plugin from release-please ([#99](https://github.com/aram-devdocs/plumb/issues/99)) ([a15f910](https://github.com/aram-devdocs/plumb/commit/a15f9103c8ccad18c3ce8e4cbaf56cf66ffa422a))
* **ci:** hide MSRV version from dependabot's action-tag scanner ([#102](https://github.com/aram-devdocs/plumb/issues/102)) ([2261152](https://github.com/aram-devdocs/plumb/commit/22611521f5af567f1ff417975a2224a2468954d6))
* **ci:** post Claude review verdict as sticky PR comment ([#106](https://github.com/aram-devdocs/plumb/issues/106)) ([ecd1f3f](https://github.com/aram-devdocs/plumb/commit/ecd1f3ff23a602c0e4b8640528181928e2a74871))
* **ci:** use simple release-type with TOML extra-file for workspace version ([#100](https://github.com/aram-devdocs/plumb/issues/100)) ([547aeab](https://github.com/aram-devdocs/plumb/commit/547aeabb58af3a264545d6c93f1a4aef66279a14))
* **cli:** error when --viewport is passed without configured [viewports] ([#119](https://github.com/aram-devdocs/plumb/issues/119)) ([#124](https://github.com/aram-devdocs/plumb/issues/124)) ([5849499](https://github.com/aram-devdocs/plumb/commit/58494990d48dd9e9b4d6a465840b662067f5e136))
* **gh-runbook:** file-backed state + substitute {{PARENT_ISSUE}} before child creation ([1304a83](https://github.com/aram-devdocs/plumb/commit/1304a83c295399a514a28d12da4606790f82375d))
* **hooks:** detect subagent context at PreToolUse, not SessionStart ([#116](https://github.com/aram-devdocs/plumb/issues/116)) ([b39a55b](https://github.com/aram-devdocs/plumb/commit/b39a55bc6e0fed140ba48babae26cdc97c0761cb))
* **hooks:** pin HOOK_SESSION_ID to stdin session_id so review gates compose across hook invocations ([#105](https://github.com/aram-devdocs/plumb/issues/105)) ([dd0cd18](https://github.com/aram-devdocs/plumb/commit/dd0cd18e993bace545d607d411f58946b4a779c5))


### Documentation

* **agents:** hierarchical AGENTS.md + CLAUDE.md symlinks + size validator ([25e7719](https://github.com/aram-devdocs/plumb/commit/25e7719edbe9b9e4b633b2d5d677c8ed9a26d5d1))
* retarget phase 1 acceptance criterion to checked-in fixture ([#122](https://github.com/aram-devdocs/plumb/issues/122)) ([fae2b39](https://github.com/aram-devdocs/plumb/commit/fae2b394b7c53c44645e6f27dbe85c4b1a8fd7dd)), closes [#120](https://github.com/aram-devdocs/plumb/issues/120)
* **rules:** add no-legacy-code policy + workspace lints ([#107](https://github.com/aram-devdocs/plumb/issues/107)) ([2f280e0](https://github.com/aram-devdocs/plumb/commit/2f280e06c8783e9c36c81419cfa5c3f2574652ab))
* **runbooks:** V0→V1 delivery specs for all 7 phases + roadmap umbrella + phase labels ([6e47ea7](https://github.com/aram-devdocs/plumb/commit/6e47ea733ea9e9c36a3bdbbef0f5adb0dd539be4))

## [Unreleased]

### Added

- Initial workspace scaffold, tooling, and walking skeleton.
- PRD-style `[spacing]` and `[type]` config sections with schema validation.
- Rule `spacing/grid-conformance`: flags `margin-*`, `padding-*`, `gap`, `row-gap`, and `column-gap` values that aren't multiples of `spacing.base_unit`.
- Rule `spacing/scale-conformance`: flags the same property set when values aren't members of `spacing.scale`.
- Rule `type/scale-conformance`: flags `font-size` values that aren't members of `type.scale`.
- PRD §12.2 `[color]`, `[radius]`, `[alignment]`, `[a11y]` config sections fleshed out: `color.delta_e_tolerance` (default 2.0), `alignment.tolerance_px` (default 3), `a11y.touch_target.{min_width_px, min_height_px}` (default 24×24 per WCAG 2.5.8).
- DTCG 2025.10 token adapter in `plumb-config`: `merge_dtcg(&mut Config, &DtcgSource)` imports a Design Tokens Community Group JSON file into a `Config`. Maps `color`, `dimension` (spacing or typography by namespace heuristic), `fontFamily`, `fontWeight`, and `radius` / `borderRadius`; resolves `{path.to.token}` brace aliases and `{ "$ref": "#/..." }` pointers with cycle detection; caps nesting at 64 levels.

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
