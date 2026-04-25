# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

From the first release onward, this file is maintained automatically by [`release-please`](https://github.com/googleapis/release-please) based on [Conventional Commits](https://www.conventionalcommits.org/) on `main`. Do not edit released sections by hand.

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

### Removed

- Walking-skeleton placeholder rule `placeholder/hello-world` and its docs.
