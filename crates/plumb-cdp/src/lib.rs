//! # plumb-cdp
//!
//! Chromium DevTools Protocol driver for Plumb.
//!
//! This crate owns every interaction with the browser. It is the **only**
//! Plumb crate where `unsafe` is permitted — and only for FFI-adjacent
//! hot spots, each with an explicit `// SAFETY:` comment. The walking
//! skeleton doesn't yet use `unsafe`; the override exists to preempt
//! future friction when the real CDP driver lands.
//!
//! ## Pinned Chromium version
//!
//! [`PINNED_CHROMIUM_MAJOR`] is the canonical Chromium major version
//! Plumb renders against. Pinning the browser is part of Plumb's
//! determinism guarantee (`docs/local/prd.md` §9, §16).
//!
//! ## Walking-skeleton behavior
//!
//! [`ChromiumDriver::snapshot`] currently returns [`CdpError::NotImplemented`].
//! The `plumb-fake://` URL scheme in `plumb-cli` is handled by
//! [`FakeDriver`] from this crate's `test-fake` wiring — that scheme is
//! the only way to exercise the full pipeline until the real driver
//! lands.

#![deny(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use plumb_core::{PlumbSnapshot, ViewportKey};

/// Pinned Chromium major version. Any CI or local run that boots a
/// Chromium binary older or newer than this major version refuses to run.
pub const PINNED_CHROMIUM_MAJOR: u32 = 131;

/// A snapshot target: URL + viewport.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    /// URL to navigate to. The `plumb-fake://` scheme is reserved for
    /// deterministic fixtures used by tests and the walking-skeleton CLI.
    pub url: String,
    /// Named viewport.
    pub viewport: ViewportKey,
    /// Viewport width in CSS pixels.
    pub width: u32,
    /// Viewport height in CSS pixels.
    pub height: u32,
    /// Device pixel ratio.
    pub device_pixel_ratio: f32,
}

/// Errors returned by drivers.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CdpError {
    /// The driver isn't implemented yet (walking skeleton).
    #[error("ChromiumDriver is not implemented yet; use `plumb-fake://` URLs until PR #2 lands")]
    NotImplemented,
    /// An unknown URL scheme was passed to the fake driver.
    #[error("FakeDriver does not recognize URL `{0}`")]
    UnknownFakeUrl(String),
    /// The Chromium binary reported a major version we don't support.
    #[error("Chromium major version {found} is not supported (Plumb pins to {expected})")]
    UnsupportedChromium {
        /// Expected major version (see [`PINNED_CHROMIUM_MAJOR`]).
        expected: u32,
        /// Detected major version.
        found: u32,
    },
    /// Any other driver-level failure, carried as a boxed [`std::error::Error`].
    #[error("driver failure: {0}")]
    Driver(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Async trait for browser drivers. Implementations are expected to be
/// cheap to construct and expensive per-call.
pub trait BrowserDriver: Send + Sync {
    /// Snapshot a single target.
    fn snapshot(
        &self,
        target: Target,
    ) -> impl std::future::Future<Output = Result<PlumbSnapshot, CdpError>> + Send;
}

/// Real Chromium-backed driver. Not yet implemented — see PR #2.
#[derive(Debug, Default, Clone, Copy)]
pub struct ChromiumDriver;

impl BrowserDriver for ChromiumDriver {
    #[allow(clippy::unused_async)]
    async fn snapshot(&self, _target: Target) -> Result<PlumbSnapshot, CdpError> {
        Err(CdpError::NotImplemented)
    }
}

/// Deterministic fake driver. Recognizes `plumb-fake://hello` and returns
/// [`PlumbSnapshot::canned`]. Used by the walking-skeleton CLI and by
/// downstream tests.
#[derive(Debug, Default, Clone, Copy)]
pub struct FakeDriver;

impl BrowserDriver for FakeDriver {
    #[allow(clippy::unused_async)]
    async fn snapshot(&self, target: Target) -> Result<PlumbSnapshot, CdpError> {
        if target.url == "plumb-fake://hello" {
            Ok(PlumbSnapshot::canned())
        } else {
            Err(CdpError::UnknownFakeUrl(target.url))
        }
    }
}

/// Whether a URL belongs to the fake-driver scheme.
#[must_use]
pub fn is_fake_url(url: &str) -> bool {
    url.starts_with("plumb-fake://")
}
