//! Type-scale rules.
//!
//! Currently exposes:
//!
//! - [`family_conformance`] — `font-family` values must be members of
//!   `type.families`.
//! - [`scale_conformance`] — `font-size` values must be members of
//!   `type.scale`.
//! - [`weight_conformance`] — `font-weight` values must be members of
//!   `type.weights`.
//!
//! `type` is a Rust keyword, so the module is `type_`. The rule id
//! retains the canonical `type/<id>` shape.

pub mod family_conformance;
pub mod scale_conformance;
pub mod weight_conformance;
