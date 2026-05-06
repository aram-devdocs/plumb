//! # plumb-e2e
//!
//! End-to-end harness that drives the locally built `plumb` binary
//! against the framework fixtures under `e2e-sites/`.
//!
//! Each fixture is built (`just build` inside its directory), served
//! over loopback HTTP, and linted via `plumb lint
//! http://127.0.0.1:<port>/ --config e2e-sites/plumb.toml --format
//! json`. The harness then asserts:
//!
//! 1. The violation count for each `target_rule` declared in
//!    `<fixture>/expected.json` matches `by_rule_id` exactly.
//! 2. The total target-rule violation count matches
//!    `total_target_violations`.
//! 3. Three back-to-back runs produce byte-identical JSON output.
//!
//! Non-target rules are tolerated. The fixtures intentionally narrow
//! the assertion surface to a handful of rules so the matrix stays
//! robust against incidental Chromium-side rendering differences.
//!
//! The harness is dev-only (`publish = false`) and depends on nothing
//! upstream of `plumb-cli`. It does not run inside `cargo test
//! --workspace` by default; invoke it via `just test-e2e` or
//! `cargo run -p plumb-e2e -- --all`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod expected;
pub mod runner;
pub mod server;
pub mod sites;
pub mod workspace;

pub use expected::{Expected, WaitFor};
pub use runner::{HarnessConfig, RunReport, run_site};
pub use server::StaticServer;
pub use sites::{SITES, SiteMeta};
pub use workspace::find_workspace_root;

use thiserror::Error;

/// Errors emitted by the e2e harness.
#[derive(Debug, Error)]
pub enum HarnessError {
    /// Failed to read or parse `expected.json`.
    #[error("read or parse expected.json for site `{site}`: {source}")]
    Expected {
        /// Site name.
        site: String,
        /// Underlying I/O or parse error.
        #[source]
        source: anyhow::Error,
    },
    /// The fixture's `Justfile` build failed.
    #[error("build fixture `{site}` (just build): {source}")]
    Build {
        /// Site name.
        site: String,
        /// Underlying error.
        #[source]
        source: anyhow::Error,
    },
    /// Could not bind a loopback port for the static file server.
    #[error("bind loopback HTTP server for site `{site}`: {source}")]
    Bind {
        /// Site name.
        site: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The plumb binary returned an unexpected exit code or non-JSON output.
    #[error("`plumb lint` for site `{site}` failed: {reason}")]
    Lint {
        /// Site name.
        site: String,
        /// Reason string (stderr or parse error).
        reason: String,
    },
    /// Three runs did not produce byte-identical output.
    #[error("non-deterministic output for site `{site}`: runs differ")]
    NonDeterministic {
        /// Site name.
        site: String,
    },
    /// The observed violation breakdown did not match `expected.json`.
    #[error(
        "violation count mismatch for site `{site}`: rule_id `{rule_id}` expected {expected}, got {actual}"
    )]
    CountMismatch {
        /// Site name.
        site: String,
        /// Rule id whose count diverged.
        rule_id: String,
        /// Expected count.
        expected: usize,
        /// Observed count.
        actual: usize,
    },
    /// Workspace layout was not as expected.
    #[error("locate workspace root: {0}")]
    Workspace(String),
}
