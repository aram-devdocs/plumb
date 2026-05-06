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
use plumb_core::snapshot::{SnapshotNode, TextBox};
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
use chromiumoxide::cdp::browser_protocol::network::{
    CookieParam, Headers, SetCookiesParams, SetExtraHttpHeadersParams,
};
use chromiumoxide::cdp::browser_protocol::page::AddScriptToEvaluateOnNewDocumentParams;
use chromiumoxide::cdp::browser_protocol::target::{
    CreateBrowserContextParams, CreateTargetParams,
};
use chromiumoxide::detection::DetectionOptions;
use chromiumoxide::{Browser, BrowserConfig, Handler};
use futures_util::StreamExt;
use serde::Deserialize;
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

/// A snapshot target: URL + viewport + per-target capture knobs.
///
/// The capture knobs (`wait_for_selector`, `wait_ms`,
/// `disable_animations`, `hide_scrollbars`, `pin_dpr`) are documented
/// in PRD §15. They control browser-side behavior between navigation
/// and `DOMSnapshot.captureSnapshot` and never flow into snapshot
/// content — they only affect *when* the snapshot is captured and what
/// CSS state the page is in at that moment.
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
    /// Optional CSS selector to wait for before capturing the snapshot.
    /// When set, the driver polls the page until at least one matching
    /// element exists. Compatible with [`Self::wait_ms`] — both fire,
    /// in order: selector first, then the additional sleep.
    pub wait_for_selector: Option<String>,
    /// Optional additional milliseconds to sleep before capturing the
    /// snapshot, after navigation (and after [`Self::wait_for_selector`]).
    pub wait_ms: Option<u64>,
    /// Inject CSS that disables animations and transitions before the
    /// page renders. Defaults to `true` — the historical Plumb behavior
    /// (PRD §16) — and the CLI exposes a flag that flips this value.
    pub disable_animations: bool,
    /// Inject CSS that hides page-level scrollbars. Defaults to `true`
    /// to match the Chromium launch arg `--hide-scrollbars`. The CSS
    /// belt-and-suspenders covers cases where the launch arg alone is
    /// not honored (e.g. older Chromium majors on certain platforms).
    pub hide_scrollbars: bool,
    /// Optional explicit device-pixel ratio override applied via
    /// `Emulation.setDeviceMetricsOverride.deviceScaleFactor` instead of
    /// using [`Self::device_pixel_ratio`]. When `None`, the existing
    /// `device_pixel_ratio` is used. The CLI exposes this as `--dpr`.
    pub pin_dpr: Option<f64>,
}

impl Target {
    /// Effective device-scale factor for `Emulation.setDeviceMetricsOverride`.
    ///
    /// Prefers [`Self::pin_dpr`] when set, otherwise falls back to
    /// [`Self::device_pixel_ratio`]. Centralizing the choice keeps the
    /// "pin overrides default" rule in one place.
    #[must_use]
    pub fn effective_dpr(&self) -> f64 {
        self.pin_dpr
            .unwrap_or_else(|| f64::from(self.device_pixel_ratio))
    }
}

impl Default for Target {
    fn default() -> Self {
        Self {
            url: String::new(),
            viewport: ViewportKey::new("desktop"),
            width: 1280,
            height: 800,
            device_pixel_ratio: 1.0,
            wait_for_selector: None,
            wait_ms: None,
            disable_animations: true,
            hide_scrollbars: true,
            pin_dpr: None,
        }
    }
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
    /// A user-supplied cookie name/value contained illegal characters
    /// (header injection guard — newlines are refused before reaching
    /// the browser).
    #[error("invalid cookie {field} `{input}`: {reason}")]
    InvalidCookie {
        /// Which cookie field failed validation (`name` or `value`).
        field: &'static str,
        /// The offending input.
        input: String,
        /// Reason the input was rejected.
        reason: &'static str,
    },
    /// A user-supplied HTTP header name/value contained illegal
    /// characters (header injection guard — newlines and `:` in names
    /// are refused before reaching the browser).
    #[error("invalid header {field} `{input}`: {reason}")]
    InvalidHeader {
        /// Which header field failed validation (`name` or `value`).
        field: &'static str,
        /// The offending input.
        input: String,
        /// Reason the input was rejected.
        reason: &'static str,
    },
    /// A user-supplied path (auth-script or storage-state) failed the
    /// safe-path check.
    #[error("invalid path `{path}`: {reason}")]
    InvalidPath {
        /// The offending path.
        path: PathBuf,
        /// Reason the path was rejected.
        reason: String,
    },
    /// Failed to parse a Playwright storage-state JSON file.
    #[error("malformed storage-state file `{path}`: {reason}")]
    MalformedStorageState {
        /// The file the driver was reading.
        path: PathBuf,
        /// What went wrong.
        reason: String,
    },
    /// Any other driver-level failure, carried as a boxed [`std::error::Error`].
    #[error("driver failure: {0}")]
    Driver(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// A cookie to install before navigation.
///
/// User-supplied cookies are validated for header-injection-style
/// payloads (newlines, NULs) before flowing into a CDP `Network.setCookies`
/// request. A `None` `url` means the cookie is bound to whatever URL the
/// target ends up navigating to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cookie {
    /// Cookie name.
    pub name: String,
    /// Cookie value.
    pub value: String,
    /// Optional explicit URL the cookie is associated with. When `None`,
    /// the cookie is associated with the target URL on injection.
    pub url: Option<String>,
    /// Optional cookie domain.
    pub domain: Option<String>,
    /// Optional cookie path (defaults to `/`).
    pub path: Option<String>,
    /// Optional `Secure` flag.
    pub secure: Option<bool>,
    /// Optional `HttpOnly` flag.
    pub http_only: Option<bool>,
}

impl Cookie {
    /// Construct a cookie from a `name=value` token. The pre-navigation
    /// helper attaches the target URL on injection.
    ///
    /// # Errors
    ///
    /// Returns [`CdpError::InvalidCookie`] when:
    /// - The token has no `=` separator.
    /// - The name is empty or contains whitespace / control bytes.
    /// - The value contains control bytes (header injection).
    pub fn parse_kv(token: &str) -> Result<Self, CdpError> {
        let (name, value) = token
            .split_once('=')
            .ok_or_else(|| CdpError::InvalidCookie {
                field: "name",
                input: token.to_owned(),
                reason: "expected `name=value`",
            })?;
        let name = name.trim().to_owned();
        let value = value.to_owned();
        validate_cookie_name(&name)?;
        validate_cookie_value(&value)?;
        Ok(Self {
            name,
            value,
            ..Self::default()
        })
    }

    fn into_cdp_param(self, default_url: Option<&str>) -> CookieParam {
        let mut param = CookieParam::new(self.name, self.value);
        param.url = self.url.or_else(|| default_url.map(str::to_owned));
        param.domain = self.domain;
        param.path = self.path;
        param.secure = self.secure;
        param.http_only = self.http_only;
        param
    }
}

/// Reject any byte that is a C0 control character (`< 0x20`) or DEL
/// (`0x7F`). Plumb chooses to reject every C0 byte rather than only the
/// HTTP-specific CR/LF/NUL trio because a cookie or header value with
/// any control byte is almost certainly a smuggling attempt and never
/// a legitimate input. Tab (`\t`, `0x09`) is also rejected; HTTP
/// whitespace folding has been deprecated in RFC 7230 §3.2.4 and Plumb
/// has no compatibility need for it on inputs the user types into a
/// shell flag.
fn is_disallowed_ctl(byte: u8) -> bool {
    byte < 0x20 || byte == 0x7F
}

fn validate_no_ctl(input: &str, field: &'static str, kind: &'static str) -> Result<(), CdpError> {
    if input.bytes().any(is_disallowed_ctl) {
        return match kind {
            "cookie" => Err(CdpError::InvalidCookie {
                field,
                input: input.to_owned(),
                reason: "control characters (C0 / DEL) are not allowed",
            }),
            _ => Err(CdpError::InvalidHeader {
                field,
                input: input.to_owned(),
                reason: "control characters (C0 / DEL) are not allowed",
            }),
        };
    }
    Ok(())
}

/// Validate an HTTP header name. Rejects empty names, names containing
/// `:` (the field-line separator), whitespace, or control bytes.
///
/// Shared between [`parse_header_kv`] (CLI input parser) and the
/// pre-injection sweep in `install_extra_headers` (library boundary).
fn validate_header_name(name: &str) -> Result<(), CdpError> {
    if name.is_empty() {
        return Err(CdpError::InvalidHeader {
            field: "name",
            input: name.to_owned(),
            reason: "name must not be empty",
        });
    }
    if name
        .bytes()
        .any(|b| b == b':' || b == b' ' || b == b'\t' || is_disallowed_ctl(b))
    {
        return Err(CdpError::InvalidHeader {
            field: "name",
            input: name.to_owned(),
            reason: "name must not contain whitespace, `:`, or control bytes",
        });
    }
    Ok(())
}

/// Validate a cookie name. Rejects empty names, names containing `=`
/// (the cookie separator), whitespace, or control bytes.
///
/// Shared between [`Cookie::parse_kv`] (CLI input parser) and the
/// pre-injection sweep in `install_cookies` (library boundary). The
/// rules mirror RFC 6265 token characters minus the bytes Chromium's
/// `Network.setCookies` would reject.
fn validate_cookie_name(name: &str) -> Result<(), CdpError> {
    if name.is_empty() {
        return Err(CdpError::InvalidCookie {
            field: "name",
            input: name.to_owned(),
            reason: "name must not be empty",
        });
    }
    if name
        .bytes()
        .any(|b| b == b'=' || b == b' ' || b == b'\t' || is_disallowed_ctl(b))
    {
        return Err(CdpError::InvalidCookie {
            field: "name",
            input: name.to_owned(),
            reason: "name must not contain whitespace, `=`, or control bytes",
        });
    }
    Ok(())
}

/// Validate a cookie value. Rejects values containing whitespace
/// (which Chromium normalizes inconsistently) or control bytes.
///
/// Shared between [`Cookie::parse_kv`] and `install_cookies`.
fn validate_cookie_value(value: &str) -> Result<(), CdpError> {
    if value.bytes().any(is_disallowed_ctl) {
        return Err(CdpError::InvalidCookie {
            field: "value",
            input: value.to_owned(),
            reason: "control characters (C0 / DEL) are not allowed",
        });
    }
    Ok(())
}

/// Parse and validate an HTTP header `name: value` token.
///
/// # Errors
///
/// Returns [`CdpError::InvalidHeader`] when:
/// - The token has no `:` separator.
/// - The name is empty or contains whitespace / `:` / control bytes.
/// - The value contains control bytes (header injection).
pub fn parse_header_kv(token: &str) -> Result<(String, String), CdpError> {
    let (name, value) = token
        .split_once(':')
        .ok_or_else(|| CdpError::InvalidHeader {
            field: "name",
            input: token.to_owned(),
            reason: "expected `name: value`",
        })?;
    let name = name.trim().to_owned();
    let value = value.trim_start().to_owned();
    validate_header_name(&name)?;
    validate_no_ctl(&value, "value", "header")?;
    Ok((name, value))
}

/// Playwright `storage-state.json` representation.
///
/// Matches the format Playwright writes via
/// [`browserContext.storageState()`](https://playwright.dev/docs/api/class-browsercontext#browser-context-storage-state)
/// — a `cookies` array plus an `origins` array of `{ origin,
/// localStorage }`. Deserialized with `deny_unknown_fields` so a
/// future Playwright addition fails loudly rather than being silently
/// ignored.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageState {
    /// Cookies preserved across the session.
    #[serde(default)]
    pub cookies: Vec<StorageStateCookie>,
    /// Per-origin localStorage entries.
    #[serde(default)]
    pub origins: Vec<StorageStateOrigin>,
}

/// One cookie entry in a Playwright `storage-state.json`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageStateCookie {
    /// Cookie name.
    pub name: String,
    /// Cookie value.
    pub value: String,
    /// Cookie domain.
    pub domain: String,
    /// Cookie path.
    pub path: String,
    /// Cookie expiration as a Unix timestamp; Playwright uses `-1` for
    /// session cookies.
    #[serde(default)]
    pub expires: f64,
    /// `HttpOnly` flag.
    #[serde(default, rename = "httpOnly")]
    pub http_only: bool,
    /// `Secure` flag.
    #[serde(default)]
    pub secure: bool,
    /// `SameSite` attribute (typically `"Strict" | "Lax" | "None"`).
    #[serde(default, rename = "sameSite")]
    pub same_site: Option<String>,
}

/// One `origins[]` entry in a Playwright `storage-state.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageStateOrigin {
    /// The origin URL (e.g. `https://example.com`).
    pub origin: String,
    /// `localStorage` entries for the origin.
    #[serde(default, rename = "localStorage")]
    pub local_storage: Vec<StorageStateLocalStorageEntry>,
}

/// One `localStorage[]` entry in a Playwright `storage-state.json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageStateLocalStorageEntry {
    /// localStorage key.
    pub name: String,
    /// localStorage value.
    pub value: String,
}

impl StorageState {
    /// Parse a Playwright `storage-state.json` from a string.
    ///
    /// # Errors
    ///
    /// Returns [`CdpError::MalformedStorageState`] with `path = ""` when
    /// the JSON cannot be parsed. Callers that have a real path on hand
    /// should use [`Self::load_from_path`] instead so the error carries
    /// the source filename.
    pub fn parse_str(json: &str) -> Result<Self, CdpError> {
        serde_json::from_str(json).map_err(|err| CdpError::MalformedStorageState {
            path: PathBuf::new(),
            reason: err.to_string(),
        })
    }

    /// Read and parse a storage-state file from disk.
    ///
    /// # Errors
    ///
    /// Returns [`CdpError::InvalidPath`] when the path fails the safe-path
    /// check, or [`CdpError::MalformedStorageState`] when the file cannot
    /// be read or parsed.
    ///
    /// # Security boundary
    ///
    /// The safe-path check via [`canonicalize_safe_path`] is
    /// **best-effort** only — see that function's docs. The
    /// canonicalize-then-open sequence has an inherent TOCTOU window
    /// where a co-located attacker with write access to a parent
    /// directory could swap the resolved file for a symlink between
    /// the check and the read. Plumb's storage-state loader is
    /// intended for files the invoking user controls (typically a
    /// Playwright export checked into the project). It MUST NOT be
    /// treated as a sandbox against hostile local users. The full
    /// mitigation (`cap_std::Dir::open`) is out of scope for the wave
    /// that introduced this loader.
    pub fn load_from_path(path: &Path) -> Result<Self, CdpError> {
        let canonical = canonicalize_safe_path(path)?;
        let bytes =
            std::fs::read_to_string(&canonical).map_err(|err| CdpError::MalformedStorageState {
                path: canonical.clone(),
                reason: err.to_string(),
            })?;
        let mut state: Self =
            serde_json::from_str(&bytes).map_err(|err| CdpError::MalformedStorageState {
                path: canonical.clone(),
                reason: err.to_string(),
            })?;
        // Validate every cookie name/value for header-injection style
        // payloads — Playwright files are typically machine-written but
        // Plumb cannot trust their provenance.
        for cookie in &state.cookies {
            validate_no_ctl(&cookie.name, "name", "cookie")?;
            validate_no_ctl(&cookie.value, "value", "cookie")?;
        }
        // Sort cookies and origins for deterministic injection order.
        state.cookies.sort_by(|a, b| {
            (a.domain.as_str(), a.name.as_str()).cmp(&(b.domain.as_str(), b.name.as_str()))
        });
        state.origins.sort_by(|a, b| a.origin.cmp(&b.origin));
        for origin in &mut state.origins {
            origin.local_storage.sort_by(|a, b| a.name.cmp(&b.name));
        }
        Ok(state)
    }
}

/// Canonicalize `path` and reject symlinks pointing outside the current
/// working directory.
///
/// `--auth-script` and `--storage-state` accept arbitrary file paths,
/// so the caller-side check is the last guard before we read user
/// content. The check refuses paths that:
/// - cannot be canonicalized (file does not exist / no permission),
/// - resolve to a different prefix than the current working directory.
///
/// # Security boundary
///
/// This is a **best-effort** guard against accidental path issues
/// (typos, copy-pasted absolute paths, runs from the wrong CWD). It is
/// **not** a security boundary against a co-located attacker who can
/// race the file system — the canonicalize step and the subsequent
/// `std::fs::read_to_string` are two separate `open(2)` syscalls, and
/// an attacker with write access to a parent directory of `path` can
/// swap the canonicalized target for a symlink between the check and
/// the read (TOCTOU). A full mitigation would use `cap_std::Dir::open`
/// to keep the canonicalization and the read inside a single
/// directory handle; that change is out of scope for the wave that
/// added this helper.
///
/// Future maintainers MUST NOT assume this function defends against a
/// hostile local user. Treat it as a usability check, not a sandbox.
fn canonicalize_safe_path(path: &Path) -> Result<PathBuf, CdpError> {
    let canonical = path.canonicalize().map_err(|err| CdpError::InvalidPath {
        path: path.to_path_buf(),
        reason: format!("could not canonicalize: {err}"),
    })?;
    let cwd = std::env::current_dir().map_err(|err| CdpError::InvalidPath {
        path: path.to_path_buf(),
        reason: format!("could not read CWD: {err}"),
    })?;
    let cwd_canonical = cwd.canonicalize().unwrap_or(cwd);
    if !canonical.starts_with(&cwd_canonical) {
        return Err(CdpError::InvalidPath {
            path: path.to_path_buf(),
            reason: format!(
                "path resolves to `{}`, which is outside the current working directory `{}`",
                canonical.display(),
                cwd_canonical.display()
            ),
        });
    }
    Ok(canonical)
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
    /// Cookies to install before navigation (PRD §15 — `--cookie`).
    /// Iterated in `(name, value)` order for deterministic CDP traffic.
    pub cookies: Vec<Cookie>,
    /// Extra HTTP headers to attach to every request (PRD §15 —
    /// `--header`). Sorted by name on injection so CDP traffic is
    /// stable across runs.
    pub headers: Vec<(String, String)>,
    /// Path to a JavaScript file evaluated on every new document via
    /// `Page.addScriptToEvaluateOnNewDocument` before navigation
    /// (PRD §15 — `--auth-script`).
    pub auth_script: Option<PathBuf>,
    /// Path to a Playwright `storage-state.json` file. Cookies in the
    /// file are installed before navigation; localStorage entries are
    /// preserved as a parsed [`StorageState`] for downstream evaluation
    /// after navigation when the origin matches.
    pub storage_state: Option<PathBuf>,
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
                let snap = capture_target(&session.browser, target, &self.options).await?;
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

async fn capture_target(
    browser: &Browser,
    target: &Target,
    options: &ChromiumOptions,
) -> Result<PlumbSnapshot, CdpError> {
    let page = browser
        .new_page("about:blank")
        .await
        .map_err(driver_error)?;

    capture_on_page(&page, target, options).await
}

/// Apply viewport / animation hooks, install cookies and headers,
/// navigate, capture a DOM snapshot.
///
/// Shared between `ChromiumDriver::capture_target` and
/// [`PersistentBrowser::snapshot`] so that the per-target work is
/// expressed in exactly one place. The function is split into discrete
/// stages — `apply_viewport` (DPR + dimensions), `pre_navigate`
/// (cookies, headers, auth-script, storage-state, animation killer,
/// scrollbar killer), `goto` + waits, then capture.
async fn capture_on_page(
    page: &Page,
    target: &Target,
    options: &ChromiumOptions,
) -> Result<PlumbSnapshot, CdpError> {
    apply_viewport(page, target).await?;
    // `pre_navigate` returns the parsed `StorageState` (when one is
    // configured) so the post-navigate localStorage step reuses the
    // same parsed value. Loading the file twice would open a
    // time-of-check / time-of-use race where the file changes between
    // cookie installation and localStorage replay.
    let storage_state = pre_navigate(page, target, options).await?;

    page.goto(target.url.as_str()).await.map_err(driver_error)?;
    page.wait_for_navigation().await.map_err(driver_error)?;

    apply_post_navigate_waits(page, target).await?;
    apply_storage_state_local_storage(page, target, storage_state.as_ref()).await?;

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
    options: ChromiumOptions,
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
                options,
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
            capture_on_page(&page, &target, &self.inner.options).await
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
    // `pin_dpr` (PRD §15 — `--dpr`) wins over `device_pixel_ratio` so
    // that callers can stress determinism by pinning a hidpi factor
    // independent of the viewport's logical DPR.
    let params = SetDeviceMetricsOverrideParams {
        width: i64::from(target.width),
        height: i64::from(target.height),
        device_scale_factor: target.effective_dpr(),
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

/// All work that must happen on a fresh page before navigation.
///
/// Runs in this fixed order so behavior matches what users expect:
/// 1. Animation/scrollbar CSS killers — PRD §16 determinism.
/// 2. Auth script — runs before any page script, so the page-side
///    bootstrap can set window globals before the SPA boots.
/// 3. Cookies and HTTP headers — set on the network layer before the
///    very first request leaves Chromium.
/// 4. Storage-state cookies — same network layer; localStorage entries
///    in the storage-state are deferred to [`apply_storage_state_local_storage`]
///    after the origin loads, since localStorage is origin-scoped.
///
/// When [`ChromiumOptions::storage_state`] is set, the file is loaded
/// and parsed exactly once here. The returned [`StorageState`] is
/// threaded back into [`apply_storage_state_local_storage`] so the
/// driver never re-reads the file (closing a TOCTOU window where the
/// content could change between cookie installation and localStorage
/// replay).
async fn pre_navigate(
    page: &Page,
    target: &Target,
    options: &ChromiumOptions,
) -> Result<Option<StorageState>, CdpError> {
    if target.disable_animations {
        inject_animation_killer(page).await?;
    }
    if target.hide_scrollbars {
        inject_scrollbar_killer(page).await?;
    }
    if let Some(script_path) = options.auth_script.as_deref() {
        inject_auth_script(page, script_path).await?;
    }
    if !options.headers.is_empty() {
        install_extra_headers(page, &options.headers).await?;
    }
    if !options.cookies.is_empty() {
        install_cookies(page, &options.cookies, target.url.as_str()).await?;
    }
    let storage_state = if let Some(state_path) = options.storage_state.as_deref() {
        let state = StorageState::load_from_path(state_path)?;
        install_storage_state_cookies(page, &state).await?;
        Some(state)
    } else {
        None
    };
    Ok(storage_state)
}

/// Wait stages that must run *after* navigation. PRD §15 — `--wait-for`
/// and `--wait-ms`.
///
/// Selector wait fires first (so users can synchronize on a
/// known-rendered element); the additional `--wait-ms` then runs as a
/// belt-and-suspenders sleep for SPAs whose post-render work doesn't
/// finish in the same tick.
async fn apply_post_navigate_waits(page: &Page, target: &Target) -> Result<(), CdpError> {
    if let Some(selector) = target.wait_for_selector.as_deref() {
        wait_for_selector(page, selector).await?;
    }
    if let Some(ms) = target.wait_ms {
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    }
    Ok(())
}

/// Install localStorage entries from an already-parsed Playwright
/// storage-state.
///
/// Runs *after* navigation because `localStorage` is origin-scoped and
/// the only way to write to it from the driver is to evaluate a script
/// in the page context. Entries whose `origin` does not match the
/// navigated URL's origin are skipped (same isolation Playwright applies).
///
/// The caller provides the parsed [`StorageState`] (loaded once in
/// [`pre_navigate`]) so the file is never read twice — closing the
/// TOCTOU window between cookie installation and localStorage replay.
async fn apply_storage_state_local_storage(
    page: &Page,
    target: &Target,
    state: Option<&StorageState>,
) -> Result<(), CdpError> {
    let Some(state) = state else {
        return Ok(());
    };
    let target_origin = origin_of(target.url.as_str()).unwrap_or_default();
    for origin_entry in &state.origins {
        if origin_entry.origin != target_origin {
            continue;
        }
        for entry in &origin_entry.local_storage {
            // Build a JSON.stringify-style argument so the values are
            // safe regardless of contained quotes.
            let key =
                serde_json::to_string(&entry.name).map_err(|err| CdpError::MalformedStorageState {
                    path: PathBuf::new(),
                    reason: format!("could not serialize key: {err}"),
                })?;
            let value = serde_json::to_string(&entry.value).map_err(|err| {
                CdpError::MalformedStorageState {
                    path: PathBuf::new(),
                    reason: format!("could not serialize value: {err}"),
                }
            })?;
            let script = format!("window.localStorage.setItem({key}, {value});");
            page.evaluate(script.as_str()).await.map_err(driver_error)?;
        }
    }
    Ok(())
}

fn origin_of(input: &str) -> Option<String> {
    // WHATWG-compliant origin: `Url::origin().ascii_serialization()`
    // handles default-port elision (`:443` for `https`, `:80` for
    // `http`), scheme case-folding, IDNA host normalization, and
    // strips userinfo / path / query / fragment. Matches Playwright's
    // stored `origin` shape so storage-state origin compares are not
    // tripped up by `https://example.com:443/foo` vs
    // `https://example.com`.
    let parsed = url::Url::parse(input).ok()?;
    let origin = parsed.origin();
    if origin.is_tuple() {
        Some(origin.ascii_serialization())
    } else {
        // Opaque origins (e.g. `data:`, `file:`) cannot match a
        // Playwright-recorded site origin — bail out.
        None
    }
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
    add_script_to_evaluate_on_new_document(page, source).await
}

async fn inject_scrollbar_killer(page: &Page) -> Result<(), CdpError> {
    // PRD §16 determinism mitigation: scrollbar overlay differs across
    // platforms / DPRs. The `--hide-scrollbars` Chromium launch arg is a
    // first line of defense; this CSS injection covers the cases where
    // the launch arg alone is not honored (Linux non-overlay scrollbars,
    // CSS-painted scrollbars in some apps).
    let source = "(() => { \
        const style = document.createElement('style'); \
        style.textContent = 'html { overflow: hidden !important; } \
            ::-webkit-scrollbar { display: none !important; }'; \
        (document.head || document.documentElement).appendChild(style); \
    })();";
    add_script_to_evaluate_on_new_document(page, source).await
}

/// Read `path` (validated as a `.js` file under the CWD) and register
/// it as `Page.addScriptToEvaluateOnNewDocument` so it runs before any
/// page script.
///
/// # Security boundary
///
/// The safe-path check via [`canonicalize_safe_path`] is best-effort
/// only — see that function's docs. Treat the resulting file content
/// as user-trusted: the CLI hands us a path supplied either by the
/// invoking user or by an `auth-script` already in the project, never
/// by a remote source. The TOCTOU window between canonicalization and
/// `std::fs::read_to_string` is acknowledged but not yet closed; the
/// full fix requires `cap_std`.
async fn inject_auth_script(page: &Page, path: &Path) -> Result<(), CdpError> {
    let canonical = canonicalize_safe_path(path)?;
    if canonical.extension().and_then(|s| s.to_str()) != Some("js") {
        return Err(CdpError::InvalidPath {
            path: path.to_path_buf(),
            reason: "auth script must have a `.js` extension".to_owned(),
        });
    }
    let source = std::fs::read_to_string(&canonical).map_err(|err| CdpError::InvalidPath {
        path: canonical.clone(),
        reason: format!("could not read: {err}"),
    })?;
    add_script_to_evaluate_on_new_document(page, &source).await
}

async fn add_script_to_evaluate_on_new_document(page: &Page, source: &str) -> Result<(), CdpError> {
    let params = AddScriptToEvaluateOnNewDocumentParams {
        source: source.to_owned(),
        world_name: None,
        include_command_line_api: None,
        run_immediately: Some(true),
    };
    page.execute(params).await.map_err(driver_error)?;
    Ok(())
}

async fn install_extra_headers(page: &Page, headers: &[(String, String)]) -> Result<(), CdpError> {
    // Sort by name for deterministic CDP traffic. Plumb's invariant is
    // byte-identical *output*, but stable network-layer requests make
    // diffing tcpdumps across runs viable too.
    let mut entries: Vec<(String, String)> = headers.to_vec();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut object = serde_json::Map::with_capacity(entries.len());
    for (name, value) in entries {
        // Library-boundary re-validation: `headers: Vec<(String, String)>`
        // is `pub` on `ChromiumOptions`, so a downstream consumer can
        // construct entries without going through `parse_header_kv`.
        // Apply the same checks here to keep header-injection guards
        // intact regardless of how the entries were built.
        validate_header_name(&name)?;
        validate_no_ctl(&value, "value", "header")?;
        object.insert(name, serde_json::Value::String(value));
    }
    let params = SetExtraHttpHeadersParams::new(Headers::new(serde_json::Value::Object(object)));
    page.execute(params).await.map_err(driver_error)?;
    Ok(())
}

async fn install_cookies(
    page: &Page,
    cookies: &[Cookie],
    default_url: &str,
) -> Result<(), CdpError> {
    // Sort by `(name, value)` so the network-layer call is stable across
    // runs even when the caller supplied cookies in a different order.
    let mut sorted: Vec<Cookie> = cookies.to_vec();
    sorted.sort_by(|a, b| {
        (a.name.as_str(), a.value.as_str()).cmp(&(b.name.as_str(), b.value.as_str()))
    });
    // Library-boundary re-validation: `Cookie` fields are all `pub`, so
    // a downstream consumer can build a `Cookie` without going through
    // `Cookie::parse_kv`. Apply the same name/value checks here so the
    // injection guards are not bypassable. `domain` and `path`, when
    // present, also pass through the control-byte check.
    for cookie in &sorted {
        validate_cookie_name(&cookie.name)?;
        validate_cookie_value(&cookie.value)?;
        if let Some(domain) = cookie.domain.as_deref() {
            validate_no_ctl(domain, "domain", "cookie")?;
        }
        if let Some(path) = cookie.path.as_deref() {
            validate_no_ctl(path, "path", "cookie")?;
        }
    }
    let url_for_cookies = if default_url.starts_with("http") {
        Some(default_url)
    } else {
        None
    };
    let params = SetCookiesParams::new(
        sorted
            .into_iter()
            .map(|c| c.into_cdp_param(url_for_cookies))
            .collect(),
    );
    page.execute(params).await.map_err(driver_error)?;
    Ok(())
}

async fn install_storage_state_cookies(page: &Page, state: &StorageState) -> Result<(), CdpError> {
    if state.cookies.is_empty() {
        return Ok(());
    }
    let mut params: Vec<CookieParam> = Vec::with_capacity(state.cookies.len());
    for cookie in &state.cookies {
        let mut p = CookieParam::new(cookie.name.clone(), cookie.value.clone());
        p.domain = Some(cookie.domain.clone());
        p.path = Some(cookie.path.clone());
        p.secure = Some(cookie.secure);
        p.http_only = Some(cookie.http_only);
        params.push(p);
    }
    page.execute(SetCookiesParams::new(params))
        .await
        .map_err(driver_error)?;
    Ok(())
}

async fn wait_for_selector(page: &Page, selector: &str) -> Result<(), CdpError> {
    // Poll `find_element` with a 50ms backoff up to 10 seconds total
    // (PRD §15 default). The selector is the users contract for "the
    // page is rendered enough for me" — burning the full 10 seconds is
    // intentional when the selector never matches; we surface that as a
    // driver error so CI fails loudly rather than capturing a half-baked
    // snapshot.
    //
    // Wall-clock-free implementation: an outer `tokio::time::timeout`
    // bounds the whole loop. Tokios timer infrastructure does its own
    // monotonic time tracking internally and is allowed in `plumb-cdp`
    // because it doesnt leak into the snapshot (PRD §9 isolates the
    // "no wall-clock" rule to the rule engine and observable output).
    let attempt = async {
        loop {
            match page.find_element(selector.to_owned()).await {
                Ok(_) => return Ok::<(), CdpError>(()),
                Err(_) => {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            }
        }
    };
    match tokio::time::timeout(std::time::Duration::from_secs(10), attempt).await {
        Ok(result) => result,
        Err(_) => Err(CdpError::Driver(Box::new(io::Error::other(format!(
            "wait_for_selector `{selector}` exhausted 10s budget"
        ))))),
    }
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

    let text_boxes = extract_text_boxes(document, &layout_view, &node_to_dom_order);

    Ok(PlumbSnapshot {
        url: target.url.clone(),
        viewport: target.viewport.clone(),
        viewport_width: target.width,
        viewport_height: target.height,
        nodes,
        text_boxes,
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

/// Extract text boxes from `document.text_boxes`, mapping layout indices
/// back to `dom_order` via the layout view and node-to-dom-order map.
///
/// Gracefully skips entries whose layout index is out of range or
/// points to a non-element node. Returns sorted by `(dom_order, start)`.
fn extract_text_boxes(
    document: &DocumentSnapshot,
    layout_view: &LayoutView<'_>,
    node_to_dom_order: &[Option<u64>],
) -> Vec<TextBox> {
    let tb = &document.text_boxes;
    let count = tb.layout_index.len();

    // Parallel arrays must agree on length; if not, return empty rather
    // than panic — the snapshot is still usable without text boxes.
    if tb.bounds.len() != count || tb.start.len() != count || tb.length.len() != count {
        return Vec::new();
    }

    let mut result: Vec<TextBox> = Vec::with_capacity(count);
    for i in 0..count {
        let layout_idx = tb.layout_index[i];
        let Ok(layout_idx_usize) = usize::try_from(layout_idx) else {
            continue;
        };
        if layout_idx_usize >= layout_view.len() {
            continue;
        }
        // layout_view.node_index maps layout slot → CDP node index.
        let Ok(cdp_node_idx) = layout_view.node_index(layout_idx_usize) else {
            continue;
        };
        let Ok(cdp_node_idx_usize) = usize::try_from(cdp_node_idx) else {
            continue;
        };
        if cdp_node_idx_usize >= node_to_dom_order.len() {
            continue;
        }
        let Some(dom_order) = node_to_dom_order[cdp_node_idx_usize] else {
            continue;
        };

        let bounds_inner = tb.bounds[i].inner();
        if bounds_inner.len() != 4 {
            continue;
        }
        let bounds = rect_from_bounds(bounds_inner);

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let start = tb.start[i].max(0) as u32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let length = tb.length[i].max(0) as u32;

        result.push(TextBox {
            dom_order,
            bounds,
            start,
            length,
        });
    }

    // Sort by (dom_order, start) for determinism.
    result.sort_by_key(|tb| (tb.dom_order, tb.start));
    result
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

    use super::{Cookie, StorageState, parse_header_kv};

    #[test]
    fn cookie_parse_kv_accepts_simple_pair() {
        let c = Cookie::parse_kv("session=abc123").unwrap();
        assert_eq!(c.name, "session");
        assert_eq!(c.value, "abc123");
        assert!(c.url.is_none());
    }

    #[test]
    fn cookie_parse_kv_rejects_missing_separator() {
        let err = Cookie::parse_kv("nosep").unwrap_err();
        assert!(matches!(err, CdpError::InvalidCookie { .. }));
    }

    #[test]
    fn cookie_parse_kv_rejects_empty_name() {
        let err = Cookie::parse_kv("=value").unwrap_err();
        assert!(matches!(err, CdpError::InvalidCookie { .. }));
    }

    #[test]
    fn cookie_parse_kv_rejects_crlf_in_value() {
        let err = Cookie::parse_kv("name=hello\r\nSet-Cookie: pwn=1").unwrap_err();
        match err {
            CdpError::InvalidCookie { field, reason, .. } => {
                assert_eq!(field, "value");
                assert!(reason.contains("control characters"));
            }
            other => panic!("expected InvalidCookie, got {other:?}"),
        }
    }

    #[test]
    fn header_parse_kv_accepts_pair() {
        let (n, v) = parse_header_kv("X-Trace-Id: 12345").unwrap();
        assert_eq!(n, "X-Trace-Id");
        assert_eq!(v, "12345");
    }

    #[test]
    fn header_parse_kv_rejects_missing_colon() {
        let err = parse_header_kv("nope").unwrap_err();
        assert!(matches!(err, CdpError::InvalidHeader { .. }));
    }

    #[test]
    fn header_parse_kv_rejects_lf_in_value() {
        let err = parse_header_kv("X-Pwn: hi\nInjected: 1").unwrap_err();
        assert!(matches!(err, CdpError::InvalidHeader { .. }));
    }

    #[test]
    fn header_parse_kv_rejects_space_in_name() {
        let err = parse_header_kv("X Header: 1").unwrap_err();
        assert!(matches!(err, CdpError::InvalidHeader { .. }));
    }

    #[test]
    fn validate_header_name_rejects_colon() {
        // Library-boundary check: a downstream consumer might construct
        // `headers: vec![("Foo:Bar".into(), "1".into())]` directly.
        // `parse_header_kv` would split that, but
        // `install_extra_headers` calls the validator straight on the
        // tuple — so the validator must catch `:` itself.
        let err = super::validate_header_name("Foo:Bar").unwrap_err();
        assert!(matches!(err, CdpError::InvalidHeader { field: "name", .. }));
    }

    #[test]
    fn validate_header_name_rejects_whitespace() {
        let err = super::validate_header_name("X Header").unwrap_err();
        assert!(matches!(err, CdpError::InvalidHeader { .. }));
        let err = super::validate_header_name("X\tHeader").unwrap_err();
        assert!(matches!(err, CdpError::InvalidHeader { .. }));
    }

    #[test]
    fn validate_header_name_rejects_control_bytes() {
        // Every C0 control byte (and DEL) is rejected. Spot-check the
        // canonical ones plus a non-CRLF C1-adjacent byte (BEL, 0x07).
        for &c in b"\r\n\0\x07\x1b\x7f" {
            let name = format!("X-Hi{}Foo", c as char);
            let err = super::validate_header_name(&name).unwrap_err();
            assert!(
                matches!(err, CdpError::InvalidHeader { .. }),
                "expected InvalidHeader for byte {c:#x}, got {err:?}"
            );
        }
    }

    #[test]
    fn validate_cookie_name_rejects_equals_and_whitespace() {
        // Library-boundary: `Cookie { name: "foo=bar", .. }` would be
        // accepted by the parser (it splits on the *first* `=`) but
        // direct construction would let `=` through. The standalone
        // validator must reject it.
        let err = super::validate_cookie_name("foo=bar").unwrap_err();
        assert!(matches!(err, CdpError::InvalidCookie { field: "name", .. }));
        let err = super::validate_cookie_name("foo bar").unwrap_err();
        assert!(matches!(err, CdpError::InvalidCookie { .. }));
    }

    #[test]
    fn validate_cookie_value_rejects_full_c0_range() {
        // Tightened beyond CR/LF/NUL — every C0 byte and DEL is now
        // rejected. Tab is in the C0 range so it's also rejected.
        for c in 0u8..0x20 {
            let value = format!("v{}x", c as char);
            let err = super::validate_cookie_value(&value).unwrap_err();
            assert!(
                matches!(err, CdpError::InvalidCookie { .. }),
                "expected InvalidCookie for byte {c:#x}, got {err:?}"
            );
        }
        let err = super::validate_cookie_value("v\x7fx").unwrap_err();
        assert!(matches!(err, CdpError::InvalidCookie { .. }));
    }

    #[test]
    fn storage_state_parses_minimal_payload() {
        let json = r#"{
            "cookies": [
                {"name":"a","value":"1","domain":".example.com","path":"/","expires":-1,"httpOnly":false,"secure":false,"sameSite":"Lax"}
            ],
            "origins": [
                {"origin":"https://example.com","localStorage":[{"name":"k","value":"v"}]}
            ]
        }"#;
        let state = StorageState::parse_str(json).unwrap();
        assert_eq!(state.cookies.len(), 1);
        assert_eq!(state.cookies[0].name, "a");
        assert_eq!(state.origins.len(), 1);
        assert_eq!(state.origins[0].origin, "https://example.com");
        assert_eq!(state.origins[0].local_storage[0].name, "k");
    }

    #[test]
    fn storage_state_parses_empty_payload() {
        let state = StorageState::parse_str(r#"{"cookies":[],"origins":[]}"#).unwrap();
        assert!(state.cookies.is_empty());
        assert!(state.origins.is_empty());
    }

    #[test]
    fn storage_state_rejects_unknown_fields() {
        let json = r#"{"cookies":[],"origins":[],"unexpected":42}"#;
        let err = StorageState::parse_str(json).unwrap_err();
        assert!(matches!(err, CdpError::MalformedStorageState { .. }));
    }

    #[test]
    fn target_default_sets_capture_knobs() {
        let t = super::Target::default();
        assert!(t.disable_animations);
        assert!(t.hide_scrollbars);
        assert!(t.wait_for_selector.is_none());
        assert!(t.wait_ms.is_none());
        assert!(t.pin_dpr.is_none());
    }

    #[test]
    fn target_effective_dpr_prefers_pin_over_default() {
        let mut t = super::Target {
            device_pixel_ratio: 1.0,
            ..super::Target::default()
        };
        assert!((t.effective_dpr() - 1.0).abs() < f64::EPSILON);
        t.pin_dpr = Some(3.0);
        assert!((t.effective_dpr() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn origin_of_handles_https_url() {
        assert_eq!(
            super::origin_of("https://example.com/path?q=1").as_deref(),
            Some("https://example.com")
        );
        assert_eq!(
            super::origin_of("http://example.com:8080/").as_deref(),
            Some("http://example.com:8080")
        );
        assert_eq!(super::origin_of("notaurl").as_deref(), None);
    }

    #[test]
    fn origin_of_strips_default_ports() {
        // WHATWG origin: default ports are elided.
        assert_eq!(
            super::origin_of("https://example.com:443/").as_deref(),
            Some("https://example.com")
        );
        assert_eq!(
            super::origin_of("http://example.com:80/").as_deref(),
            Some("http://example.com")
        );
    }

    #[test]
    fn origin_of_normalizes_scheme_and_host_case() {
        assert_eq!(
            super::origin_of("HTTPS://Example.COM/path").as_deref(),
            Some("https://example.com")
        );
    }

    #[test]
    fn origin_of_strips_userinfo_query_fragment() {
        assert_eq!(
            super::origin_of("https://user:pw@example.com/p?q=1#frag").as_deref(),
            Some("https://example.com")
        );
    }

    #[test]
    fn origin_of_returns_none_for_opaque_origins() {
        // `data:` and `file:` URLs have opaque origins and cannot match
        // a Playwright-recorded site origin.
        assert_eq!(super::origin_of("data:text/plain,hello").as_deref(), None);
    }
}
