//! # plumb-cdp
//!
//! Chromium DevTools Protocol driver for Plumb.
//!
//! This crate owns every interaction with the browser. It is the **only**
//! Plumb crate where `unsafe` is permitted — and only for FFI-adjacent
//! hot spots, each with an explicit `// SAFETY:` comment. The walking
//! skeleton doesn't yet use `unsafe`; the override exists to preempt
//! future friction when snapshot conversion lands.
//!
//! ## Pinned Chromium version
//!
//! [`PINNED_CHROMIUM_MAJOR`] is the canonical Chromium major version
//! Plumb renders against. Pinning the browser is part of Plumb's
//! determinism guarantee (`docs/local/prd.md` §9, §16).
//!
//! ## Current behavior
//!
//! [`ChromiumDriver::snapshot`] launches Chromium and validates
//! [`Browser::version`](chromiumoxide::Browser::version), then returns
//! [`CdpError::NotImplemented`] until DOMSnapshot conversion lands in
//! Issue #15. The `plumb-fake://` URL scheme in `plumb-cli` is handled
//! by [`FakeDriver`] from this crate's `test-fake` wiring.

#![deny(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use plumb_core::{PlumbSnapshot, ViewportKey};
use std::io;
use std::path::{Path, PathBuf};

use chromiumoxide::detection::DetectionOptions;
use chromiumoxide::{Browser, BrowserConfig, Handler};
use futures_util::StreamExt;
use tokio::task::JoinHandle;

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
    /// DOMSnapshot conversion is not implemented yet.
    #[error("DOMSnapshot conversion is not implemented yet (Issue #15)")]
    NotImplemented,
    /// An unknown URL scheme was passed to the fake driver.
    #[error("FakeDriver does not recognize URL `{0}`")]
    UnknownFakeUrl(String),
    /// No suitable Chromium or Chrome executable could be found.
    #[error("Chromium executable not found. {install_hint}")]
    ChromiumNotFound {
        /// Human-readable installation and override guidance.
        install_hint: String,
    },
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

/// Configuration for [`ChromiumDriver`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ChromiumOptions {
    /// Explicit Chrome or Chromium executable path. When unset, Plumb asks
    /// `chromiumoxide` to detect stable Chrome/Chromium installations.
    pub executable_path: Option<PathBuf>,
}

/// Real Chromium-backed driver.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ChromiumDriver {
    options: ChromiumOptions,
}

impl ChromiumDriver {
    /// Build a driver with explicit options.
    #[must_use]
    pub fn new(options: ChromiumOptions) -> Self {
        Self { options }
    }

    fn browser_config(&self, target: &Target) -> Result<BrowserConfig, CdpError> {
        let builder = BrowserConfig::builder()
            .chrome_detection(DetectionOptions {
                msedge: false,
                unstable: false,
            })
            .window_size(target.width, target.height);

        let builder = if let Some(path) = &self.options.executable_path {
            ensure_executable_path(path)?;
            builder.chrome_executable(path)
        } else {
            builder
        };

        builder.build().map_err(|_| chromium_not_found())
    }
}

impl BrowserDriver for ChromiumDriver {
    async fn snapshot(&self, target: Target) -> Result<PlumbSnapshot, CdpError> {
        let config = self.browser_config(&target)?;
        let mut session = ChromiumSession::launch(config).await?;

        let result = match validate_browser_version(&session.browser).await {
            Ok(()) => Err(CdpError::NotImplemented),
            Err(err) => Err(err),
        };

        if let Err(cleanup_err) = session.shutdown().await {
            tracing::debug!(error = %cleanup_err, "failed to clean up Chromium session");
            if result.is_ok() {
                return Err(cleanup_err);
            }
        }

        result
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

fn ensure_executable_path(path: &Path) -> Result<(), CdpError> {
    if path.is_file() {
        Ok(())
    } else {
        Err(chromium_not_found())
    }
}

fn chromium_not_found() -> CdpError {
    CdpError::ChromiumNotFound {
        install_hint: chromium_install_hint(),
    }
}

fn chromium_install_hint() -> String {
    let platform_hint = if cfg!(target_os = "macos") {
        "macOS: install Google Chrome or run `brew install --cask chromium`."
    } else if cfg!(target_os = "windows") {
        "Windows: install Google Chrome or Chromium and pass the `.exe` path if it is not auto-detected."
    } else {
        "Linux: install `google-chrome-stable`, `chromium`, or `chromium-browser` with your package manager."
    };

    format!(
        "Install Chrome/Chromium {PINNED_CHROMIUM_MAJOR} or pass `--executable-path <path>` to a compatible binary. {platform_hint}"
    )
}

struct ChromiumSession {
    browser: Browser,
    handler_task: JoinHandle<()>,
}

impl ChromiumSession {
    async fn launch(config: BrowserConfig) -> Result<Self, CdpError> {
        let (browser, handler) = Browser::launch(config).await.map_err(map_launch_error)?;
        let handler_task = poll_handler(handler);
        Ok(Self {
            browser,
            handler_task,
        })
    }

    async fn shutdown(&mut self) -> Result<(), CdpError> {
        let close_result = self.browser.close().await.map_err(driver_error);
        if let Err(close_err) = close_result {
            if let Err(kill_err) = kill_browser(&mut self.browser).await {
                tracing::debug!(error = %kill_err, "failed to kill Chromium after close error");
            }
            self.abort_handler().await;
            return Err(close_err);
        }

        if let Err(wait_err) = self.browser.wait().await {
            let cleanup_err = io_error(wait_err);
            if let Err(kill_err) = kill_browser(&mut self.browser).await {
                tracing::debug!(error = %kill_err, "failed to kill Chromium after wait error");
            }
            self.abort_handler().await;
            return Err(cleanup_err);
        }

        self.abort_handler().await;
        Ok(())
    }

    async fn abort_handler(&mut self) {
        self.handler_task.abort();
        if let Err(join_err) = (&mut self.handler_task).await {
            if !join_err.is_cancelled() {
                tracing::debug!(error = %join_err, "Chromium handler task failed");
            }
        }
    }
}

fn poll_handler(mut handler: Handler) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(result) = handler.next().await {
            if let Err(err) = result {
                tracing::debug!(error = %err, "Chromium handler error");
            }
        }
    })
}

async fn kill_browser(browser: &mut Browser) -> Result<(), CdpError> {
    if let Some(result) = browser.kill().await {
        result.map_err(io_error)?;
    }
    Ok(())
}

async fn validate_browser_version(browser: &Browser) -> Result<(), CdpError> {
    let version = browser.version().await.map_err(driver_error)?;
    validate_chromium_product_major(&version.product)
}

fn validate_chromium_product_major(product: &str) -> Result<(), CdpError> {
    let found = chromium_major_from_product(product).ok_or_else(|| {
        CdpError::Driver(Box::new(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("could not parse Chromium product version `{product}`"),
        )))
    })?;

    if found == PINNED_CHROMIUM_MAJOR {
        Ok(())
    } else {
        Err(CdpError::UnsupportedChromium {
            expected: PINNED_CHROMIUM_MAJOR,
            found,
        })
    }
}

fn chromium_major_from_product(product: &str) -> Option<u32> {
    let (_, version) = product.split_once('/')?;
    let major = version.split('.').next()?;
    major.parse().ok()
}

fn map_launch_error(err: chromiumoxide::error::CdpError) -> CdpError {
    match err {
        chromiumoxide::error::CdpError::Io(io_err) => {
            if io_err.kind() == io::ErrorKind::NotFound {
                chromium_not_found()
            } else {
                io_error(io_err)
            }
        }
        chromiumoxide::error::CdpError::LaunchIo(io_err, stderr) => {
            if io_err.kind() == io::ErrorKind::NotFound {
                chromium_not_found()
            } else {
                CdpError::Driver(Box::new(chromiumoxide::error::CdpError::LaunchIo(
                    io_err, stderr,
                )))
            }
        }
        other => driver_error(other),
    }
}

fn driver_error(err: chromiumoxide::error::CdpError) -> CdpError {
    CdpError::Driver(Box::new(err))
}

fn io_error(err: io::Error) -> CdpError {
    CdpError::Driver(Box::new(err))
}

#[cfg(test)]
mod tests {
    use super::{CdpError, PINNED_CHROMIUM_MAJOR};

    #[test]
    fn parses_product_major_versions() {
        assert_eq!(
            super::chromium_major_from_product("Chrome/131.0.6778.204"),
            Some(131)
        );
        assert_eq!(
            super::chromium_major_from_product("HeadlessChrome/131.0.6778.204"),
            Some(131)
        );
        assert_eq!(
            super::chromium_major_from_product("Chromium/131.0.6778.204"),
            Some(131)
        );
        assert_eq!(super::chromium_major_from_product("Chrome"), None);
        assert_eq!(
            super::chromium_major_from_product("Chrome/not-a-version"),
            None
        );
    }

    #[test]
    fn detects_unsupported_chromium_major() {
        let result = super::validate_chromium_product_major("Chrome/132.0.0.0");

        assert!(matches!(
            result,
            Err(CdpError::UnsupportedChromium {
                expected: PINNED_CHROMIUM_MAJOR,
                found: 132,
            })
        ));
    }

    #[test]
    fn accepts_pinned_chromium_major() {
        let product = format!("HeadlessChrome/{PINNED_CHROMIUM_MAJOR}.0.0.0");

        assert!(super::validate_chromium_product_major(&product).is_ok());
    }
}
