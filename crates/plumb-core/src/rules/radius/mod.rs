//! Border-radius rules.
//!
//! Currently exposes:
//!
//! - [`scale_conformance`] — `border-*-radius` values must be members of
//!   `radius.scale`.

pub mod scale_conformance;

/// Physical-longhand border-radius properties the rules in this category
/// inspect.
///
/// The `border-radius` shorthand is deliberately omitted — Chromium's
/// `getComputedStyle` returns longhands per PRD §10.3, and checking both
/// shapes would double-count every offense.
pub(crate) const RADIUS_PROPERTIES: &[&str] = &[
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-right-radius",
    "border-bottom-left-radius",
];
