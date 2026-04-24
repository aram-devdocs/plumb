//! Spacing rules.
//!
//! Two checks live here:
//!
//! - [`grid_conformance`] — values must be multiples of `spacing.base_unit`.
//! - [`scale_conformance`] — values must be members of the discrete
//!   `spacing.scale` set.
//!
//! Both rules iterate the same set of physical-longhand spacing
//! properties and skip values that do not parse as `<n>px`.

pub mod grid_conformance;
pub mod scale_conformance;

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
