//! Spacing rules.
//!
//! Currently exposes:
//!
//! - [`grid_conformance`] — values must be multiples of `spacing.base_unit`.
//!
//! Every rule in this category iterates the same physical-longhand
//! spacing properties and skips values that do not parse as `<n>px`.

pub mod grid_conformance;

/// Physical-longhand spacing properties the rules in this category
/// inspect.
///
/// Shorthands (`margin`, `padding`) are deliberately omitted — Chromium's
/// `getComputedStyle` returns longhands by default (PRD §10.3), so real
/// snapshots never carry the shorthand and checking it would double-count
/// every offense.
pub(crate) const SPACING_PROPERTIES: &[&str] = &[
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "gap",
    "row-gap",
    "column-gap",
];
