//! Opt-in telemetry stub.
//!
//! Shipped as a no-op in the walking skeleton so future telemetry work
//! doesn't need a schema migration. The default is [`TelemetryMode::Off`]
//! and [`emit`] is a no-op.

use serde::{Deserialize, Serialize};

/// Telemetry mode — controlled by the user via config and `PLUMB_TELEMETRY`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryMode {
    /// No telemetry sent.
    #[default]
    Off,
    /// Anonymous counts only — no URLs, no violation content.
    Anon,
}

/// Record a telemetry event. No-op in the walking skeleton.
#[inline]
pub fn emit(_event: &str) {
    // Intentionally empty.
}
