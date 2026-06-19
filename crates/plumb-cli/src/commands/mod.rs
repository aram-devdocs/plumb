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
pub mod watch;

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

/// `--min-severity` threshold shared by `lint` and `watch`.
///
/// Ordering is `Error` > `Warning` > `Info` (mirroring
/// [`plumb_core::Severity`]'s own ordering). A violation is kept when its
/// severity is at or above the selected level. `Off` keeps nothing, which
/// forces a clean (exit 0) run regardless of findings.
#[derive(Debug, Clone, Copy)]
pub enum SeverityFilter {
    /// Keep info, warnings, and errors.
    Info,
    /// Keep warnings and errors; drop info.
    Warn,
    /// Keep only errors.
    Error,
    /// Keep nothing.
    Off,
}

impl SeverityFilter {
    /// Whether a violation at `severity` is shown — and counts toward the
    /// exit code — under this threshold.
    #[must_use]
    pub fn keeps(self, severity: plumb_core::Severity) -> bool {
        use plumb_core::Severity;
        match self {
            Self::Info => true,
            Self::Warn => severity >= Severity::Warning,
            Self::Error => severity >= Severity::Error,
            Self::Off => false,
        }
    }

    /// Lowercase label used in the pretty footer.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Off => "off",
        }
    }
}

impl From<crate::MinSeverity> for SeverityFilter {
    fn from(value: crate::MinSeverity) -> Self {
        match value {
            crate::MinSeverity::Info => Self::Info,
            crate::MinSeverity::Warn => Self::Warn,
            crate::MinSeverity::Error => Self::Error,
            crate::MinSeverity::Off => Self::Off,
        }
    }
}
