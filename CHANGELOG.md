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

### Removed

- Walking-skeleton placeholder rule `placeholder/hello-world` and its docs.
