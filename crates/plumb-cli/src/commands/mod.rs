//! CLI subcommand implementations.
//!
//! Each subcommand lives in its own module and exposes a `run` function
//! that returns `anyhow::Result<ExitCode>`.

pub mod explain;
pub mod init;
pub mod lint;
pub mod mcp;
pub mod schema;
pub mod selector;

use std::fmt;

/// The output format flag shared between `lint` and `mcp`.
#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    /// Pretty-printed TTY output.
    Pretty,
    /// Canonical JSON.
    Json,
    /// SARIF 2.1.0.
    Sarif,
}

impl From<crate::Format> for OutputFormat {
    fn from(f: crate::Format) -> Self {
        match f {
            crate::Format::Pretty => Self::Pretty,
            crate::Format::Json => Self::Json,
            crate::Format::Sarif => Self::Sarif,
        }
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Pretty => "pretty",
            Self::Json => "json",
            Self::Sarif => "sarif",
        })
    }
}
