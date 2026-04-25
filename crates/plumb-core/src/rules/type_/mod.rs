//! Type-scale rules.
//!
//! Currently exposes:
//!
//! - [`scale_conformance`] — `font-size` values must be members of
//!   `type.scale`.
//!
//! `type` is a Rust keyword, so the module is `type_`. The rule id
//! retains the canonical `type/scale-conformance` shape.

pub mod scale_conformance;
