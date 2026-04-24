//! # plumb-core
//!
//! Core types and the deterministic rule engine for Plumb.
//!
//! This crate is the foundation every other Plumb crate builds on. It
//! defines the public [`Violation`] shape, the [`Rule`] trait, the
//! [`Config`] schema, and the in-memory [`PlumbSnapshot`] type that rule
//! engines evaluate.
//!
//! ## Determinism invariants
//!
//! `plumb-core` must produce byte-identical output across runs. The crate
//! forbids `std::time::SystemTime::now` and `std::time::Instant::now` via
//! `clippy::disallowed_methods`, never logs to stdout or stderr, and uses
//! deterministic iteration order everywhere ([`indexmap`] instead of
//! [`std::collections::HashMap`] for any observable output).
//!
//! See `docs/local/prd.md` §9 for the full invariant list.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::print_stdout, clippy::print_stderr)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod config;
pub mod engine;
pub mod report;
pub mod rules;
pub mod snapshot;
pub mod telemetry;

pub use config::Config;
pub use engine::run;
pub use report::{
    Confidence, Fix, FixKind, Rect, RunId, Severity, ViewportKey, Violation, ViolationSink,
};
pub use rules::{Rule, register_builtin};
pub use snapshot::{PlumbSnapshot, SnapshotCtx};
