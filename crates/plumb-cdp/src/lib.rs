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
//! ## Supported Chromium versions
//!
//! Plumb accepts Chromium major versions in the inclusive range
//! <code>[MIN_SUPPORTED_CHROMIUM_MAJOR]..=[MAX_SUPPORTED_CHROMIUM_MAJOR]</code>.
//! The lower bound is the oldest major Plumb has validated against; the
//! upper bound is the newest major tested up to. Both are public so
//! callers can introspect the accepted range. Constraining the browser
//! to a known range is part of Plumb's determinism guarantee
//! (`docs/local/prd.md` §9, §16) — DOMSnapshot output stability is
//! re-verified whenever the upper bound moves.
//!
//! ## Behavior
//!
//! [`ChromiumDriver::snapshot_all`] launches Chromium exactly once,
//! validates [`Browser::version`](chromiumoxide::Browser::version),
//! and then loops over the requested targets — for each it opens a
//! fresh page, applies the per-target viewport via CDP
//! `Emulation.setDeviceMetricsOverride`, navigates to the URL, and
//! calls `DOMSnapshot.captureSnapshot` with the
//! [`COMPUTED_STYLE_WHITELIST`] from PRD §10.3. Each CDP response is
//! flattened into a [`PlumbSnapshot`] with deterministic ordering
//! (nodes sorted by `dom_order`, computed styles inserted in
//! whitelist order). [`ChromiumDriver::snapshot`] is a thin wrapper
//! over `snapshot_all` for callers that only want a single target.
//! The `plumb-fake://` URL scheme in `plumb-cli` is handled by
//! [`FakeDriver`] from this crate's `test-fake` wiring.
//!
//! [`PersistentBrowser`] is the long-lived counterpart for callers
//! that lint many URLs in one process (the MCP server). It launches
//! Chromium once, validates the version, and gives each
//! [`PersistentBrowser::snapshot`] call a fresh incognito
//! `BrowserContext` so cookies and localStorage from call N do not
//! leak into call N+1.

#![deny(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use indexmap::IndexMap;
use plumb_core::report::Rect;
use plumb_core::snapshot::SnapshotNode;
use plumb_core::{PlumbSnapshot, ViewportKey};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chromiumoxide::Page;
use chromiumoxide::cdp::browser_protocol::browser::CloseParams as BrowserCloseParams;
use chromiumoxide::cdp::browser_protocol::dom_snapshot::{
    CaptureSnapshotParams, CaptureSnapshotReturns, DocumentSnapshot,
};
use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
use chromiumoxide::cdp::browser_protocol::page::AddScriptToEvaluateOnNewDocumentParams;
use chromiumoxide::cdp::browser_protocol::target::{
    CreateBrowserContextParams, CreateTargetParams,
};
use chromiumoxide::detection::DetectionOptions;
use chromiumoxide::{Browser, BrowserConfig, Handler};
use futures_util::StreamExt;
use tokio::task::JoinHandle;

/// Lowest Chromium major version Plumb has validated against. Booting
/// a Chromium binary with a smaller major refuses to run.
pub const MIN_SUPPORTED_CHROMIUM_MAJOR: u32 = 131;

/// Highest Chromium major version Plumb has tested up to. Booting a
/// Chromium binary with a larger major refuses to run; bump this
/// constant after running the e2e suite against the new major.
pub const MAX_SUPPORTED_CHROMIUM_MAJOR: u32 = 150;

/// CSS property whitelist passed to `DOMSnapshot.captureSnapshot` as the
/// `computedStyles` argument.
///
/// The list is the canonical source of truth for which computed styles
/// flow into [`PlumbSnapshot`] nodes. Order is significant — Chromium
/// returns per-node style values as a parallel array indexed by this
/// list, so silent reordering would mis-label every value.
///
/// Source of truth: PRD §10.3 (`docs/local/prd.md`).
pub const COMPUTED_STYLE_WHITELIST: &[&str; 36] = &[
    "font-size",
    "font-family",
    "font-weight",
    "line-height",
    "color",
    "background-color",
    "border-top-color",
    "border-right-color",
    "border-bottom-color",
    "border-left-color",
    "border-top-width",
    "border-right-width",
    "border-bottom-width",
    "border-left-width",
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-right-radius",
    "border-bottom-left-radius",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "gap",
    "row-gap",
    "column-gap",
    "display",
    "position",
    "box-shadow",
    "opacity",
    "z-index",
    "width",
    "height",
];

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
    /// An unknown URL scheme was passed to the fake driver.
    #[error("FakeDriver does not recognize URL `{0}`")]
    UnknownFakeUrl(String),
    /// No suitable Chromium or Chrome executable could be found.
    #[error("Chromium executable not found. {install_hint}")]
    ChromiumNotFound {
        /// Human-readable installation and override guidance.
        install_hint: String,
    },
    /// The Chromium binary reported a major version outside Plumb's
    /// supported range.
    #[error(
        "Chromium major version {found} is not supported (Plumb supports {min_supported}..={max_supported})"
    )]
    UnsupportedChromium {
        /// Lowest validated major version (see
        /// [`MIN_SUPPORTED_CHROMIUM_MAJOR`]).
        min_supported: u32,
        /// Highest tested major version (see
        /// [`MAX_SUPPORTED_CHROMIUM_MAJOR`]).
        max_supported: u32,
        /// Detected major version.
        found: u32,
    },
    /// The DOMSnapshot CDP response was malformed (missing index,
    /// out-of-range string, empty document list, or any other shape
    /// violation that prevents safe flattening).
    #[error("DOMSnapshot response was malformed: {reason}")]
    MalformedSnapshot {
        /// What was wrong with the response.
        reason: String,
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

    /// Snapshot a list of targets, reusing a single browser session
    /// for the whole batch. The default implementation calls
    /// [`snapshot`](BrowserDriver::snapshot) per target and is suitable
    /// for cheap drivers (e.g. [`FakeDriver`]). Real drivers MUST
    /// override this to launch the browser exactly once per batch.
    ///
    /// Snapshots are returned in the same order as `targets`.
    fn snapshot_all(
        &self,
        targets: Vec<Target>,
    ) -> impl std::future::Future<Output = Result<Vec<PlumbSnapshot>, CdpError>> + Send {
        async move {
            let mut out = Vec::with_capacity(targets.len());
            for target in targets {
                out.push(self.snapshot(target).await?);
            }
            Ok(out)
        }
    }
}

/// Configuration for [`ChromiumDriver`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ChromiumOptions {
    /// Explicit Chrome or Chromium executable path. When unset, Plumb asks
    /// `chromiumoxide` to detect stable Chrome/Chromium installations.
    pub executable_path: Option<PathBuf>,
    /// Override the Chromium profile directory. When unset, `chromiumoxide`
    /// reuses a single temp directory across all launches — which is fine
    /// for sequential CLI invocations but causes profile-singleton lock
    /// contention when multiple drivers run concurrently (e.g. the e2e
    /// test suite). Tests pass per-thread tempdirs here.
    ///
    /// Profile contents do not flow into [`PlumbSnapshot`] output, so
    /// varying this path does not violate the determinism invariant.
    pub user_data_dir: Option<PathBuf>,
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
        // PRD §16: pinning launch args removes a class of nondeterminism
        // (scrollbar overlay differences across DPRs, OS-level scaling).
        let scale_factor_arg = format!("--force-device-scale-factor={}", target.device_pixel_ratio);
        let builder = BrowserConfig::builder()
            .chrome_detection(DetectionOptions {
                msedge: false,
                unstable: false,
            })
            .window_size(target.width, target.height)
            .arg("--hide-scrollbars")
            .arg(scale_factor_arg);

        let builder = if let Some(path) = &self.options.executable_path {
            ensure_executable_path(path)?;
            builder.chrome_executable(path)
        } else {
            builder
        };

        let builder = if let Some(profile) = &self.options.user_data_dir {
            builder.user_data_dir(profile)
        } else {
            builder
        };

        builder.build().map_err(|_| chromium_not_found())
    }
}

impl BrowserDriver for ChromiumDriver {
    async fn snapshot(&self, target: Target) -> Result<PlumbSnapshot, CdpError> {
        let mut snapshots = self.snapshot_all(vec![target]).await?;
        snapshots.pop().ok_or_else(|| {
            // Unreachable in practice: `snapshot_all` returns one snapshot per
            // input target on the success path. Treat a violation of that
            // contract as an internal driver fault rather than panicking.
            CdpError::Driver(Box::new(io::Error::other(
                "ChromiumDriver::snapshot_all returned no snapshot for a single target",
            )))
        })
    }

    async fn snapshot_all(&self, targets: Vec<Target>) -> Result<Vec<PlumbSnapshot>, CdpError> {
        if targets.is_empty() {
            return Ok(Vec::new());
        }

        // Use the first target's dimensions and DPR for the initial
        // launch (the `--force-device-scale-factor` arg is fixed at
        // launch time). Per-target viewport / DPR is then applied via
        // CDP `Emulation.setDeviceMetricsOverride` inside
        // `capture_target`, which overrides the launch-time scale
        // factor for every page after the first.
        let first = &targets[0];
        let config = self.browser_config(first)?;
        let mut session = ChromiumSession::launch(config).await?;

        let result: Result<Vec<PlumbSnapshot>, CdpError> = async {
            validate_browser_version(&session.browser).await?;
            let mut snapshots = Vec::with_capacity(targets.len());
            for target in &targets {
                let snap = capture_target(&session.browser, target).await?;
                snapshots.push(snap);
            }
            Ok(snapshots)
        }
        .await;

        if let Err(cleanup_err) = session.shutdown().await {
            tracing::debug!(error = %cleanup_err, "failed to clean up Chromium session");
            if result.is_ok() {
                return Err(cleanup_err);
            }
        }

        result
    }
}

async fn capture_target(browser: &Browser, target: &Target) -> Result<PlumbSnapshot, CdpError> {
    let page = browser
        .new_page("about:blank")
        .await
        .map_err(driver_error)?;

    capture_on_page(&page, target).await
}

/// Apply viewport / animation hooks, navigate, capture a DOM snapshot.
///
/// Shared between `ChromiumDriver::capture_target` and
/// [`PersistentBrowser::snapshot`] so that the per-target work is
/// expressed in exactly one place.
async fn capture_on_page(page: &Page, target: &Target) -> Result<PlumbSnapshot, CdpError> {
    apply_viewport(page, target).await?;
    inject_animation_killer(page).await?;

    page.goto(target.url.as_str()).await.map_err(driver_error)?;
    page.wait_for_navigation().await.map_err(driver_error)?;

    let params = CaptureSnapshotParams {
        computed_styles: COMPUTED_STYLE_WHITELIST
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        include_paint_order: Some(true),
        include_dom_rects: Some(true),
        include_blended_background_colors: Some(true),
        include_text_color_opacities: None,
    };

    let response = page.execute(params).await.map_err(driver_error)?;
    flatten_snapshot(target, &response.result)
}

/// A persistent Chromium browser kept warm across multiple snapshots.
///
/// Each [`PersistentBrowser::snapshot`] call creates a fresh
/// **incognito browser context** (`Target.createBrowserContext`),
/// opens a page in it, captures the snapshot, and disposes the
/// context — so cookies, localStorage, and any other origin-scoped
/// state from call N never leak into call N+1. The underlying Chromium
/// process stays alive until [`PersistentBrowser::shutdown`] is called
/// or the value is dropped.
///
/// Cheap to clone — clones share the same underlying browser via
/// [`Arc`]. Implements [`BrowserDriver`].
#[derive(Clone, Debug)]
pub struct PersistentBrowser {
    inner: Arc<PersistentBrowserInner>,
}

#[derive(Debug)]
struct PersistentBrowserInner {
    browser: Browser,
    handler_task: Mutex<Option<JoinHandle<()>>>,
}

impl PersistentBrowser {
    /// Launch Chromium and validate its version.
    ///
    /// Per-call viewport and DPR are applied via
    /// `Emulation.setDeviceMetricsOverride` inside [`Self::snapshot`],
    /// so the launch-time defaults here are placeholders sized to a
    /// 1280×800 desktop window.
    ///
    /// # Errors
    ///
    /// Returns [`CdpError::ChromiumNotFound`] when no Chromium binary
    /// can be located, [`CdpError::UnsupportedChromium`] when the
    /// detected Chromium reports a major version outside the supported
    /// range, or [`CdpError::Driver`] for any other launch failure.
    pub async fn launch(options: ChromiumOptions) -> Result<Self, CdpError> {
        let config = persistent_browser_config(&options)?;
        let (browser, handler) = Browser::launch(config).await.map_err(map_launch_error)?;
        let handler_task = poll_handler(handler);

        // Validate the version before stashing the browser in `Arc` —
        // on failure, dropping the browser here causes
        // `Browser::drop` to reap the child synchronously.
        if let Err(err) = validate_browser_version(&browser).await {
            handler_task.abort();
            drop(browser);
            return Err(err);
        }

        Ok(Self {
            inner: Arc::new(PersistentBrowserInner {
                browser,
                handler_task: Mutex::new(Some(handler_task)),
            }),
        })
    }

    /// Snapshot a single target inside a fresh incognito browser context.
    ///
    /// # Errors
    ///
    /// Returns the same error variants as [`ChromiumDriver::snapshot`]:
    /// [`CdpError::Driver`] for CDP failures and
    /// [`CdpError::MalformedSnapshot`] when the response cannot be
    /// flattened.
    pub async fn snapshot(&self, target: Target) -> Result<PlumbSnapshot, CdpError> {
        let ctx_id = self
            .inner
            .browser
            .create_browser_context(CreateBrowserContextParams::default())
            .await
            .map_err(driver_error)?;

        let result: Result<PlumbSnapshot, CdpError> = async {
            let create_params = CreateTargetParams {
                url: "about:blank".to_string(),
                left: None,
                top: None,
                width: None,
                height: None,
                window_state: None,
                browser_context_id: Some(ctx_id.clone()),
                enable_begin_frame_control: None,
                new_window: None,
                background: None,
                for_tab: None,
                hidden: None,
            };
            let page = self
                .inner
                .browser
                .new_page(create_params)
                .await
                .map_err(driver_error)?;
            capture_on_page(&page, &target).await
        }
        .await;

        // Always dispose the incognito context, even on failure. Mirror
        // the swallow-and-log pattern from `ChromiumSession::shutdown`
        // so cleanup errors never mask the underlying snapshot result.
        if let Err(err) = self
            .inner
            .browser
            .dispose_browser_context(ctx_id)
            .await
            .map_err(driver_error)
        {
            tracing::debug!(error = %err, "failed to dispose incognito browser context");
        }

        result
    }

    /// Gracefully close the underlying browser and abort the handler
    /// task.
    ///
    /// Idempotent — safe to call more than once. The first call sends
    /// `Browser.close` over CDP and aborts the handler task; subsequent
    /// calls observe the absent handle and return `Ok(())`.
    ///
    /// # Errors
    ///
    /// Currently never returns an error: cleanup failures are logged
    /// at `debug` and swallowed so callers can use `shutdown` as a
    /// best-effort hook on MCP exit. The signature retains `Result`
    /// for forward-compatibility.
    pub async fn shutdown(&self) -> Result<(), CdpError> {
        let handler_task = match self.inner.handler_task.lock() {
            Ok(mut guard) => guard.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        };

        if handler_task.is_none() {
            // Already shut down — preserve idempotence.
            return Ok(());
        }

        if let Err(err) = self
            .inner
            .browser
            .execute(BrowserCloseParams::default())
            .await
        {
            tracing::debug!(error = %err, "failed to send Browser.close on shutdown");
        }

        if let Some(task) = handler_task {
            task.abort();
        }

        Ok(())
    }
}

impl Drop for PersistentBrowserInner {
    fn drop(&mut self) {
        // Best-effort sync abort of the handler task. Sending CDP
        // commands here would require a runtime; `Browser::drop`
        // already reaps the child synchronously, so we only stop the
        // event loop.
        let task = match self.handler_task.lock() {
            Ok(mut guard) => guard.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        };
        if let Some(task) = task {
            task.abort();
        }
    }
}

impl BrowserDriver for PersistentBrowser {
    async fn snapshot(&self, target: Target) -> Result<PlumbSnapshot, CdpError> {
        Self::snapshot(self, target).await
    }
}

fn persistent_browser_config(options: &ChromiumOptions) -> Result<BrowserConfig, CdpError> {
    // PRD §16: pinning launch args removes a class of nondeterminism
    // (scrollbar overlay differences across DPRs, OS-level scaling).
    // `PersistentBrowser` does not fix a launch-time DPR — every
    // snapshot calls `Emulation.setDeviceMetricsOverride` to drive
    // both viewport and DPR per-call.
    let builder = BrowserConfig::builder()
        .chrome_detection(DetectionOptions {
            msedge: false,
            unstable: false,
        })
        .window_size(1280, 800)
        .arg("--hide-scrollbars");

    let builder = if let Some(path) = &options.executable_path {
        ensure_executable_path(path)?;
        builder.chrome_executable(path)
    } else {
        builder
    };

    let builder = if let Some(profile) = &options.user_data_dir {
        builder.user_data_dir(profile)
    } else {
        builder
    };

    builder.build().map_err(|_| chromium_not_found())
}

async fn apply_viewport(page: &Page, target: &Target) -> Result<(), CdpError> {
    let params = SetDeviceMetricsOverrideParams {
        width: i64::from(target.width),
        height: i64::from(target.height),
        device_scale_factor: f64::from(target.device_pixel_ratio),
        mobile: false,
        scale: None,
        screen_width: None,
        screen_height: None,
        position_x: None,
        position_y: None,
        dont_set_visible_size: None,
        screen_orientation: None,
        viewport: None,
    };
    page.execute(params).await.map_err(driver_error)?;
    Ok(())
}

async fn inject_animation_killer(page: &Page) -> Result<(), CdpError> {
    // PRD §16 determinism mitigation: install a CSS-injection script that
    // runs before any page script, so transitions/animations don't race
    // with `captureSnapshot` and produce different bounds across runs.
    let source = "(() => { \
        const style = document.createElement('style'); \
        style.textContent = '*, *::before, *::after { \
            animation-duration: 0s !important; \
            animation-delay: 0s !important; \
            transition-duration: 0s !important; \
            transition-delay: 0s !important; \
            caret-color: transparent !important; \
        }'; \
        (document.head || document.documentElement).appendChild(style); \
    })();";
    let params = AddScriptToEvaluateOnNewDocumentParams {
        source: source.to_string(),
        world_name: None,
        include_command_line_api: None,
        run_immediately: Some(true),
    };
    page.execute(params).await.map_err(driver_error)?;
    Ok(())
}

/// Deterministic fake driver. Recognizes `plumb-fake://hello` and returns
/// [`PlumbSnapshot::canned`]. Used by the walking-skeleton CLI and by
/// downstream tests.
///
/// Viewport-aware end-to-end: the returned snapshot's viewport name,
/// width, and height match the target, and any per-node rect that
/// covered the canned viewport is rescaled to the target dimensions
/// so that hand-testing multi-viewport behavior produces the expected
/// rects rather than the canned 1280x800 ones.
#[derive(Debug, Default, Clone, Copy)]
pub struct FakeDriver;

impl BrowserDriver for FakeDriver {
    #[allow(clippy::unused_async)]
    async fn snapshot(&self, target: Target) -> Result<PlumbSnapshot, CdpError> {
        if target.url == "plumb-fake://hello" {
            let mut snap = PlumbSnapshot::canned();
            // Capture the canned viewport bounds before overwriting so
            // we can rewrite any node rect that covered the full
            // canned viewport to the target's dimensions.
            let canned_w = snap.viewport_width;
            let canned_h = snap.viewport_height;
            snap.viewport = target.viewport.clone();
            snap.viewport_width = target.width;
            snap.viewport_height = target.height;
            for node in &mut snap.nodes {
                if let Some(rect) = node.rect.as_mut()
                    && rect.x == 0
                    && rect.y == 0
                    && rect.width == canned_w
                    && rect.height == canned_h
                {
                    rect.width = target.width;
                    rect.height = target.height;
                }
            }
            Ok(snap)
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

    // The `--executable-path` mention here is for the not-found case:
    // pointing at a binary auto-detect missed. It does NOT bypass the
    // version check — the supplied binary still has to fall in the
    // supported range.
    format!(
        "Install Chrome/Chromium between major {MIN_SUPPORTED_CHROMIUM_MAJOR} and {MAX_SUPPORTED_CHROMIUM_MAJOR} (inclusive), or pass `--executable-path <path>` to a Chromium binary in that range that auto-detect missed. {platform_hint}"
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
        if let Err(join_err) = (&mut self.handler_task).await
            && !join_err.is_cancelled()
        {
            tracing::debug!(error = %join_err, "Chromium handler task failed");
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

    // PRD §16: Plumb accepts a contiguous range of Chromium majors,
    // re-validated whenever the upper bound moves.
    if (MIN_SUPPORTED_CHROMIUM_MAJOR..=MAX_SUPPORTED_CHROMIUM_MAJOR).contains(&found) {
        Ok(())
    } else {
        Err(CdpError::UnsupportedChromium {
            min_supported: MIN_SUPPORTED_CHROMIUM_MAJOR,
            max_supported: MAX_SUPPORTED_CHROMIUM_MAJOR,
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

fn malformed(reason: impl Into<String>) -> CdpError {
    CdpError::MalformedSnapshot {
        reason: reason.into(),
    }
}

/// DOM `nodeType` for an element node — the only kind Plumb keeps in the
/// flattened snapshot. Text/comment/doctype nodes are skipped.
const ELEMENT_NODE_TYPE: i64 = 1;

/// Flatten the CDP `DOMSnapshot.captureSnapshot` response into a
/// deterministic [`PlumbSnapshot`].
///
/// The flattening is a pure function of `(target, response)`. It walks
/// `documents[0]` in source order, keeps element nodes, and resolves
/// every string index through the shared `strings` table. Children
/// lists are sorted by `dom_order` and the final node vector is sorted
/// by `dom_order` before return — these two sorts keep the snapshot
/// byte-identical across runs against the same page.
fn flatten_snapshot(
    target: &Target,
    response: &CaptureSnapshotReturns,
) -> Result<PlumbSnapshot, CdpError> {
    let strings = response.strings.as_slice();
    let document = response
        .documents
        .first()
        .ok_or_else(|| malformed("documents array is empty"))?;

    let nodes_view = NodesView::from_document(document)?;
    let layout_view = LayoutView::from_document(document)?;
    let node_to_dom_order = build_dom_order_map(&nodes_view);

    let FlattenedNodes {
        mut nodes,
        tags,
        parents,
    } = build_nodes(&nodes_view, &node_to_dom_order, strings)?;

    apply_layout(&mut nodes, &layout_view, &node_to_dom_order, strings)?;
    finalize_nodes(&mut nodes, &tags, &parents);
    nodes.sort_by_key(|n| n.dom_order);

    Ok(PlumbSnapshot {
        url: target.url.clone(),
        viewport: target.viewport.clone(),
        viewport_width: target.width,
        viewport_height: target.height,
        nodes,
    })
}

/// Result of the first flatten pass — element nodes with bookkeeping
/// indexes for the layout/selector passes.
struct FlattenedNodes {
    nodes: Vec<SnapshotNode>,
    tags: IndexMap<u64, String>,
    parents: IndexMap<u64, Option<u64>>,
}

/// Map every CDP node index → kept element's `dom_order`. Non-element
/// nodes get `None`. Element nodes get a 0-based, gap-free order.
fn build_dom_order_map(nodes_view: &NodesView<'_>) -> Vec<Option<u64>> {
    let mut map: Vec<Option<u64>> = vec![None; nodes_view.len()];
    let mut next_order: u64 = 0;
    for (idx, slot) in map.iter_mut().enumerate() {
        if nodes_view.is_element(idx) {
            *slot = Some(next_order);
            next_order += 1;
        }
    }
    map
}

fn build_nodes(
    nodes_view: &NodesView<'_>,
    node_to_dom_order: &[Option<u64>],
    strings: &[String],
) -> Result<FlattenedNodes, CdpError> {
    let mut nodes: Vec<SnapshotNode> = Vec::new();
    let mut tags: IndexMap<u64, String> = IndexMap::new();
    let mut parents: IndexMap<u64, Option<u64>> = IndexMap::new();

    for (idx, dom_order) in node_to_dom_order.iter().enumerate() {
        let Some(dom_order) = dom_order else { continue };
        let tag = lookup_string(strings, nodes_view.node_name(idx)?)?.to_lowercase();
        let attrs = nodes_view.attributes_for(idx, strings)?;
        let parent_dom_order =
            resolve_parent_dom_order(nodes_view.parent_index(idx), idx, node_to_dom_order)?;

        tags.insert(*dom_order, tag.clone());
        parents.insert(*dom_order, parent_dom_order);

        nodes.push(SnapshotNode {
            dom_order: *dom_order,
            selector: String::new(),
            tag,
            attrs,
            computed_styles: IndexMap::new(),
            rect: None,
            parent: parent_dom_order,
            children: Vec::new(),
        });
    }

    Ok(FlattenedNodes {
        nodes,
        tags,
        parents,
    })
}

fn resolve_parent_dom_order(
    parent_index: Option<i64>,
    idx: usize,
    node_to_dom_order: &[Option<u64>],
) -> Result<Option<u64>, CdpError> {
    let Some(parent_idx) = parent_index else {
        return Ok(None);
    };
    let parent_idx_usize = usize::try_from(parent_idx).map_err(|_| {
        malformed(format!(
            "negative parent index `{parent_idx}` for node {idx}"
        ))
    })?;
    if parent_idx_usize >= node_to_dom_order.len() {
        return Err(malformed(format!(
            "parent index `{parent_idx}` out of range for node {idx}"
        )));
    }
    Ok(node_to_dom_order[parent_idx_usize])
}

fn apply_layout(
    nodes: &mut [SnapshotNode],
    layout_view: &LayoutView<'_>,
    node_to_dom_order: &[Option<u64>],
    strings: &[String],
) -> Result<(), CdpError> {
    for layout_idx in 0..layout_view.len() {
        let cdp_node_idx = layout_view.node_index(layout_idx)?;
        let cdp_node_idx_usize = usize::try_from(cdp_node_idx).map_err(|_| {
            malformed(format!(
                "negative layout node index `{cdp_node_idx}` at layout slot {layout_idx}"
            ))
        })?;
        if cdp_node_idx_usize >= node_to_dom_order.len() {
            return Err(malformed(format!(
                "layout node index `{cdp_node_idx}` out of range at layout slot {layout_idx}"
            )));
        }
        let Some(dom_order) = node_to_dom_order[cdp_node_idx_usize] else {
            // Layout entry refers to a non-element node — skip.
            continue;
        };
        let Ok(dom_order_usize) = usize::try_from(dom_order) else {
            continue;
        };
        if dom_order_usize >= nodes.len() {
            continue;
        }
        if let Some(rect) = layout_view.rect_at(layout_idx)? {
            nodes[dom_order_usize].rect = Some(rect);
        }
        if let Some(styles) = layout_view.styles_at(layout_idx, strings)? {
            nodes[dom_order_usize].computed_styles = styles;
        }
    }
    Ok(())
}

fn finalize_nodes(
    nodes: &mut [SnapshotNode],
    tags: &IndexMap<u64, String>,
    parents: &IndexMap<u64, Option<u64>>,
) {
    let mut children_index: IndexMap<u64, Vec<u64>> = IndexMap::new();
    for node in nodes.iter() {
        if let Some(parent) = node.parent {
            children_index
                .entry(parent)
                .or_default()
                .push(node.dom_order);
        }
    }
    for kids in children_index.values_mut() {
        kids.sort_unstable();
    }
    for node in nodes {
        if let Some(kids) = children_index.swap_remove(&node.dom_order) {
            node.children = kids;
        }
        node.selector = build_selector(node.dom_order, tags, parents);
    }
}

fn lookup_string(strings: &[String], idx: i64) -> Result<&str, CdpError> {
    let idx_usize =
        usize::try_from(idx).map_err(|_| malformed(format!("negative string index `{idx}`")))?;
    strings
        .get(idx_usize)
        .map(String::as_str)
        .ok_or_else(|| malformed(format!("string index `{idx}` out of range")))
}

/// Like [`lookup_string`] but treats negative indices as `None` instead of
/// an error.
///
/// Chrome uses `-1` as a sentinel in optional DOMSnapshot string slots
/// (e.g. attribute values, computed-style values) to signal "no value."
/// Required slots (node names, attribute names) must still go through
/// [`lookup_string`] so that a negative index there remains a hard error.
fn lookup_optional_string(strings: &[String], idx: i64) -> Result<Option<&str>, CdpError> {
    if idx < 0 {
        return Ok(None);
    }
    lookup_string(strings, idx).map(Some)
}

fn build_selector(
    dom_order: u64,
    tags: &IndexMap<u64, String>,
    parents: &IndexMap<u64, Option<u64>>,
) -> String {
    let mut chain: Vec<&str> = Vec::new();
    let mut cursor = Some(dom_order);
    while let Some(current) = cursor {
        if let Some(tag) = tags.get(&current) {
            chain.push(tag.as_str());
        }
        cursor = parents.get(&current).copied().flatten();
    }
    chain.reverse();
    chain.join(" > ")
}

/// Borrowed view over a `NodeTreeSnapshot` that resolves the parallel
/// arrays (`parent_index`, `node_type`, `node_name`, `attributes`)
/// without copying.
struct NodesView<'a> {
    node_count: usize,
    parent_index: &'a [i64],
    node_type: &'a [i64],
    node_name: &'a [chromiumoxide::cdp::browser_protocol::dom_snapshot::StringIndex],
    attributes: Option<&'a [chromiumoxide::cdp::browser_protocol::dom_snapshot::ArrayOfStrings]>,
}

impl<'a> NodesView<'a> {
    fn from_document(document: &'a DocumentSnapshot) -> Result<Self, CdpError> {
        let node_name = document
            .nodes
            .node_name
            .as_deref()
            .ok_or_else(|| malformed("nodes.nodeName missing"))?;
        let parent_index = document
            .nodes
            .parent_index
            .as_deref()
            .ok_or_else(|| malformed("nodes.parentIndex missing"))?;
        let node_type = document
            .nodes
            .node_type
            .as_deref()
            .ok_or_else(|| malformed("nodes.nodeType missing"))?;

        let node_count = node_name.len();
        if parent_index.len() != node_count || node_type.len() != node_count {
            return Err(malformed(format!(
                "parallel node arrays disagree on length: \
                 nodeName={}, parentIndex={}, nodeType={}",
                node_name.len(),
                parent_index.len(),
                node_type.len()
            )));
        }

        let attributes = document.nodes.attributes.as_deref();
        if let Some(attrs) = attributes
            && attrs.len() != node_count
        {
            return Err(malformed(format!(
                "nodes.attributes length {} disagrees with nodeName length {node_count}",
                attrs.len()
            )));
        }

        Ok(Self {
            node_count,
            parent_index,
            node_type,
            node_name,
            attributes,
        })
    }

    fn len(&self) -> usize {
        self.node_count
    }

    fn is_element(&self, idx: usize) -> bool {
        self.node_type
            .get(idx)
            .copied()
            .is_some_and(|t| t == ELEMENT_NODE_TYPE)
    }

    fn node_name(&self, idx: usize) -> Result<i64, CdpError> {
        self.node_name
            .get(idx)
            .map(|s| *s.inner())
            .ok_or_else(|| malformed(format!("nodeName missing for node {idx}")))
    }

    fn parent_index(&self, idx: usize) -> Option<i64> {
        match self.parent_index.get(idx).copied() {
            Some(p) if p >= 0 => Some(p),
            _ => None,
        }
    }

    fn attributes_for(
        &self,
        idx: usize,
        strings: &[String],
    ) -> Result<IndexMap<String, String>, CdpError> {
        let Some(attrs) = self.attributes else {
            return Ok(IndexMap::new());
        };
        let Some(entry) = attrs.get(idx) else {
            return Ok(IndexMap::new());
        };
        let pairs = entry.inner();
        if pairs.len() % 2 != 0 {
            return Err(malformed(format!(
                "attributes for node {idx} has odd length {}",
                pairs.len()
            )));
        }
        let mut out = IndexMap::with_capacity(pairs.len() / 2);
        for chunk in pairs.chunks_exact(2) {
            let name = lookup_string(strings, *chunk[0].inner())?.to_string();
            let value = lookup_optional_string(strings, *chunk[1].inner())?
                .unwrap_or("")
                .to_string();
            out.insert(name, value);
        }
        Ok(out)
    }
}

/// Borrowed view over a `LayoutTreeSnapshot` with bounds checks against
/// the parallel `node_index`/`bounds`/`styles` arrays.
struct LayoutView<'a> {
    node_index: &'a [i64],
    bounds: &'a [chromiumoxide::cdp::browser_protocol::dom_snapshot::Rectangle],
    styles: &'a [chromiumoxide::cdp::browser_protocol::dom_snapshot::ArrayOfStrings],
}

impl<'a> LayoutView<'a> {
    fn from_document(document: &'a DocumentSnapshot) -> Result<Self, CdpError> {
        let node_index = document.layout.node_index.as_slice();
        let bounds = document.layout.bounds.as_slice();
        let styles = document.layout.styles.as_slice();
        if node_index.len() != bounds.len() {
            return Err(malformed(format!(
                "layout.nodeIndex length {} disagrees with layout.bounds length {}",
                node_index.len(),
                bounds.len()
            )));
        }
        if !styles.is_empty() && styles.len() != node_index.len() {
            return Err(malformed(format!(
                "layout.styles length {} disagrees with layout.nodeIndex length {}",
                styles.len(),
                node_index.len()
            )));
        }
        Ok(Self {
            node_index,
            bounds,
            styles,
        })
    }

    fn len(&self) -> usize {
        self.node_index.len()
    }

    fn node_index(&self, idx: usize) -> Result<i64, CdpError> {
        self.node_index
            .get(idx)
            .copied()
            .ok_or_else(|| malformed(format!("layout.nodeIndex missing slot {idx}")))
    }

    fn rect_at(&self, idx: usize) -> Result<Option<Rect>, CdpError> {
        let Some(rectangle) = self.bounds.get(idx) else {
            return Ok(None);
        };
        let inner = rectangle.inner();
        if inner.is_empty() {
            return Ok(None);
        }
        if inner.len() != 4 {
            return Err(malformed(format!(
                "layout.bounds slot {idx} has length {} (expected 4)",
                inner.len()
            )));
        }
        Ok(Some(rect_from_bounds(inner)))
    }

    fn styles_at(
        &self,
        idx: usize,
        strings: &[String],
    ) -> Result<Option<IndexMap<String, String>>, CdpError> {
        let Some(entry) = self.styles.get(idx) else {
            return Ok(None);
        };
        let style_indices = entry.inner();
        if style_indices.is_empty() {
            return Ok(Some(IndexMap::new()));
        }
        if style_indices.len() != COMPUTED_STYLE_WHITELIST.len() {
            return Err(malformed(format!(
                "layout.styles[{idx}] length {} disagrees with whitelist length {}",
                style_indices.len(),
                COMPUTED_STYLE_WHITELIST.len()
            )));
        }
        let mut out = IndexMap::with_capacity(style_indices.len());
        for (slot, prop) in style_indices.iter().zip(COMPUTED_STYLE_WHITELIST.iter()) {
            let raw = *slot.inner();
            let Some(value) = lookup_optional_string(strings, raw)? else {
                // CDP uses `-1` to indicate "no value" for this property on
                // this node — skip rather than insert empty strings.
                continue;
            };
            if value.is_empty() {
                continue;
            }
            out.insert((*prop).to_string(), value.to_string());
        }
        Ok(Some(out))
    }
}

fn rect_from_bounds(inner: &[f64]) -> Rect {
    // CDP returns CSS pixel floats. Round to the nearest integer for a
    // stable representation; clamp width/height at zero to satisfy the
    // `u32` shape on collapsed boxes (Chromium occasionally emits tiny
    // negative floats around -0.0).
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    // SAFETY (cast lints): values are bounded by viewport dimensions
    // (i32 fits viewport widths/heights up to ~2.1B px) and are clamped
    // non-negative before unsigned cast.
    let x = inner[0].round() as i32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let y = inner[1].round() as i32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let width = inner[2].round().max(0.0) as u32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let height = inner[3].round().max(0.0) as u32;
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        COMPUTED_STYLE_WHITELIST, CdpError, MAX_SUPPORTED_CHROMIUM_MAJOR,
        MIN_SUPPORTED_CHROMIUM_MAJOR,
    };

    #[test]
    fn style_whitelist_has_36_properties() {
        assert_eq!(
            COMPUTED_STYLE_WHITELIST.len(),
            36,
            "PRD §10.3 pins exactly 36 computed-style properties"
        );
    }

    #[test]
    fn style_whitelist_pins_canonical_order() {
        // Locks the exact order from PRD §10.3. If the list grows or the
        // order changes, the rule engine's interpretation of the parallel
        // style indices coming back from Chromium silently breaks.
        let expected: [&str; 36] = [
            "font-size",
            "font-family",
            "font-weight",
            "line-height",
            "color",
            "background-color",
            "border-top-color",
            "border-right-color",
            "border-bottom-color",
            "border-left-color",
            "border-top-width",
            "border-right-width",
            "border-bottom-width",
            "border-left-width",
            "border-top-left-radius",
            "border-top-right-radius",
            "border-bottom-right-radius",
            "border-bottom-left-radius",
            "margin-top",
            "margin-right",
            "margin-bottom",
            "margin-left",
            "padding-top",
            "padding-right",
            "padding-bottom",
            "padding-left",
            "gap",
            "row-gap",
            "column-gap",
            "display",
            "position",
            "box-shadow",
            "opacity",
            "z-index",
            "width",
            "height",
        ];
        assert_eq!(COMPUTED_STYLE_WHITELIST, &expected);
    }

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
        // Below the minimum is rejected.
        let below = MIN_SUPPORTED_CHROMIUM_MAJOR - 1;
        let below_product = format!("Chrome/{below}.0.0.0");
        let below_result = super::validate_chromium_product_major(&below_product);
        assert!(matches!(
            below_result,
            Err(CdpError::UnsupportedChromium {
                min_supported: MIN_SUPPORTED_CHROMIUM_MAJOR,
                max_supported: MAX_SUPPORTED_CHROMIUM_MAJOR,
                found,
            }) if found == below
        ));

        // Above the maximum is rejected.
        let above = MAX_SUPPORTED_CHROMIUM_MAJOR + 1;
        let above_product = format!("Chrome/{above}.0.0.0");
        let above_result = super::validate_chromium_product_major(&above_product);
        assert!(matches!(
            above_result,
            Err(CdpError::UnsupportedChromium {
                min_supported: MIN_SUPPORTED_CHROMIUM_MAJOR,
                max_supported: MAX_SUPPORTED_CHROMIUM_MAJOR,
                found,
            }) if found == above
        ));
    }

    #[test]
    fn accepts_supported_chromium_majors() {
        // Min, max, and an in-between value (140) all pass.
        let lower_bound = format!("HeadlessChrome/{MIN_SUPPORTED_CHROMIUM_MAJOR}.0.0.0");
        assert!(super::validate_chromium_product_major(&lower_bound).is_ok());

        let upper_bound = format!("HeadlessChrome/{MAX_SUPPORTED_CHROMIUM_MAJOR}.0.0.0");
        assert!(super::validate_chromium_product_major(&upper_bound).is_ok());

        let in_range = "HeadlessChrome/140.0.0.0";
        assert!(super::validate_chromium_product_major(in_range).is_ok());
    }

    #[test]
    fn lookup_string_rejects_negative_index() {
        let strings = vec!["hello".to_string()];
        let err = super::lookup_string(&strings, -1).unwrap_err();
        assert!(
            matches!(err, CdpError::MalformedSnapshot { ref reason } if reason.contains("negative string index")),
            "expected MalformedSnapshot for negative index, got {err:?}"
        );
    }

    #[test]
    fn lookup_string_rejects_out_of_range() {
        let strings = vec!["hello".to_string()];
        let err = super::lookup_string(&strings, 5).unwrap_err();
        assert!(
            matches!(err, CdpError::MalformedSnapshot { ref reason } if reason.contains("out of range")),
            "expected MalformedSnapshot for OOB index, got {err:?}"
        );
    }

    #[test]
    fn lookup_string_resolves_valid_index() {
        let strings = vec!["hello".to_string(), "world".to_string()];
        assert_eq!(super::lookup_string(&strings, 0).unwrap(), "hello");
        assert_eq!(super::lookup_string(&strings, 1).unwrap(), "world");
    }

    #[test]
    fn lookup_optional_string_returns_none_for_sentinel() {
        let strings = vec!["hello".to_string()];
        assert_eq!(super::lookup_optional_string(&strings, -1).unwrap(), None);
        // Other negative values also map to None.
        assert_eq!(super::lookup_optional_string(&strings, -42).unwrap(), None);
    }

    #[test]
    fn lookup_optional_string_resolves_valid_index() {
        let strings = vec!["hello".to_string(), "world".to_string()];
        assert_eq!(
            super::lookup_optional_string(&strings, 0).unwrap(),
            Some("hello")
        );
        assert_eq!(
            super::lookup_optional_string(&strings, 1).unwrap(),
            Some("world")
        );
    }

    #[test]
    fn lookup_optional_string_rejects_out_of_range() {
        let strings = vec!["hello".to_string()];
        let err = super::lookup_optional_string(&strings, 5).unwrap_err();
        assert!(
            matches!(err, CdpError::MalformedSnapshot { ref reason } if reason.contains("out of range")),
            "expected MalformedSnapshot for OOB index, got {err:?}"
        );
    }
}
