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
//! and then loops over the requested targets — the first target's
//! viewport is pinned at launch through Chromium's window size and DPR
//! flags, later targets and explicit DPR pins are applied via CDP
//! `Emulation.setDeviceMetricsOverride`, then Plumb navigates to the
//! URL and calls `DOMSnapshot.captureSnapshot` with the
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
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/aram-devdocs/plumb/main/assets/brand/plumb-mark.svg",
    html_favicon_url = "https://raw.githubusercontent.com/aram-devdocs/plumb/main/theme/favicon.svg"
)]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod chrome_path;
pub mod fetcher;

use indexmap::IndexMap;
use plumb_core::report::Rect;
use plumb_core::snapshot::{SnapshotNode, TextBox};
use plumb_core::{PlumbSnapshot, ViewportKey};
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;

use chromiumoxide::Page;
use chromiumoxide::browser::BrowserConfigBuilder;
use chromiumoxide::cdp::browser_protocol::browser::{
    CloseParams as BrowserCloseParams, GetVersionParams,
};
use chromiumoxide::cdp::browser_protocol::dom_snapshot::{
    CaptureSnapshotParams, CaptureSnapshotReturns, DocumentSnapshot,
};
use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
use chromiumoxide::cdp::browser_protocol::network::{
    CookieParam, Headers, SetCookiesParams, SetExtraHttpHeadersParams,
};
use chromiumoxide::cdp::browser_protocol::page::{
    AddScriptToEvaluateOnNewDocumentParams, EnableParams as PageEnableParams, NavigateParams,
};
use chromiumoxide::cdp::browser_protocol::target::{
    AttachToTargetParams, CreateBrowserContextParams, CreateTargetParams, SessionId,
};
use chromiumoxide::cdp::js_protocol::runtime::EvaluateParams;
use chromiumoxide::cdp::{CdpEvent, CdpEventMessage};
use chromiumoxide::detection::DetectionOptions;
use chromiumoxide::types::{CallId, Command, Message, MethodId, Response};
use chromiumoxide::{Browser, BrowserConfig, Connection, Handler};
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

const BROWSER_LAUNCH_TIMEOUT: Duration = Duration::from_secs(30);
const BROWSER_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
const BROWSER_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const BROWSER_KILL_TIMEOUT: Duration = Duration::from_secs(5);
const CHROMIUMOXIDE_REQUEST_TIMEOUT: Duration = Duration::from_mins(1);
const CDP_CONTROL_TIMEOUT: Duration = Duration::from_secs(10);
const TARGET_CREATE_TIMEOUT: Duration = Duration::from_secs(10);
const TARGET_ATTACH_TIMEOUT: Duration = Duration::from_secs(75);
const PAGE_COMMAND_TIMEOUT: Duration = Duration::from_secs(25);
const PAGE_ENABLE_TIMEOUT: Duration = Duration::from_secs(5);
const NAVIGATION_ASSIGNMENT_TIMEOUT: Duration = Duration::from_secs(2);
const DOCUMENT_READY_TIMEOUT: Duration = Duration::from_secs(30);
const INITIAL_DOCUMENT_SETTLE_DELAY: Duration = Duration::from_millis(100);
const NAVIGATION_STATE_READ_TIMEOUT: Duration = Duration::from_secs(2);
const SNAPSHOT_CAPTURE_TIMEOUT: Duration = Duration::from_secs(25);
const TRANSIENT_CAPTURE_RETRIES: usize = 1;
const INITIAL_PAGE_URL: &str = "about:blank";

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
    /// Inject CSS that disables animations and transitions before
    /// capture. Defaults to `true` — the historical Plumb behavior
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
    /// Auto-fetch (`--auto-fetch-chromium`) failed to download or
    /// install Chromium. Wraps the upstream chromiumoxide fetcher
    /// failure in a typed Plumb error so the CLI can surface a single
    /// "auto-fetch could not produce a working binary" message.
    #[error("Chromium auto-fetch failed: {reason}")]
    AutoFetchFailed {
        /// Human-readable reason (download / unzip / options error).
        reason: String,
    },
    /// A cached Chromium binary's SHA-256 disagrees with the recorded
    /// `.plumb-sha256` sidecar. Plumb refuses to launch the binary so
    /// a tampered cache cannot silently be promoted into an
    /// arbitrary-code-execution path.
    #[error(
        "Chromium binary `{}` failed hash verification: expected {expected}, found {found}",
        path.display()
    )]
    HashMismatch {
        /// Path of the offending binary.
        path: PathBuf,
        /// Hex SHA-256 from the sidecar (the value Plumb originally
        /// trusted).
        expected: String,
        /// Hex SHA-256 of the binary as it currently exists.
        found: String,
    },
    /// Auto-fetch needs a platform cache directory, but the host
    /// environment did not provide enough information to resolve one
    /// (no `HOME` / `LOCALAPPDATA` / `XDG_CACHE_HOME`).
    #[error("could not resolve a Plumb cache directory: {reason}")]
    CacheDirUnavailable {
        /// Human-readable reason (which env var was missing).
        reason: String,
    },
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
    /// Validates every cookie name, value, domain, and path for
    /// header-injection-style payloads (control bytes) and sorts the
    /// cookies / origins / localStorage entries for deterministic
    /// injection order.
    ///
    /// # Errors
    ///
    /// Returns [`CdpError::MalformedStorageState`] with `path = ""` when
    /// the JSON cannot be parsed. Returns [`CdpError::InvalidCookie`]
    /// when a cookie field contains control bytes. Callers that have a
    /// real path on hand should use [`Self::load_from_path`] instead so
    /// the error carries the source filename.
    pub fn parse_str(json: &str) -> Result<Self, CdpError> {
        let mut state: Self =
            serde_json::from_str(json).map_err(|err| CdpError::MalformedStorageState {
                path: PathBuf::new(),
                reason: err.to_string(),
            })?;
        // Validate every cookie name/value/domain/path for
        // header-injection style payloads — Playwright files are
        // typically machine-written but Plumb cannot trust their
        // provenance. `domain` and `path` flow into a CDP
        // `Network.setCookies` call alongside the name/value, so an
        // unchecked CR/LF in either field would smuggle just as
        // effectively as one in the value.
        for cookie in &state.cookies {
            validate_no_ctl(&cookie.name, "name", "cookie")?;
            validate_no_ctl(&cookie.value, "value", "cookie")?;
            validate_no_ctl(&cookie.domain, "domain", "cookie")?;
            validate_no_ctl(&cookie.path, "path", "cookie")?;
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
    /// The safe-path check via `canonicalize_safe_path` is
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
        // Re-stamp `MalformedStorageState` errors with the source path
        // so callers see *which* file failed; cookie-validation errors
        // pass through unchanged because they carry the offending input
        // rather than a path.
        Self::parse_str(&bytes).map_err(|err| match err {
            CdpError::MalformedStorageState { reason, .. } => CdpError::MalformedStorageState {
                path: canonical,
                reason,
            },
            other => other,
        })
    }
}

/// Public CLI-facing wrapper around `canonicalize_safe_path`.
///
/// `plumb-cli` validates `--auth-script` / `--storage-state` paths up
/// front (before driver dispatch) so the FakeDriver path also rejects
/// outside-CWD inputs — without this, the safe-path check would only
/// fire on the real Chromium code path and tests against
/// `plumb-fake://hello` would silently accept a malicious-looking
/// `--auth-script /etc/passwd`.
///
/// # Errors
///
/// Returns [`CdpError::InvalidPath`] when `path` cannot be
/// canonicalized or canonicalizes to a location outside the current
/// working directory.
///
/// # Security boundary
///
/// Same caveats as `canonicalize_safe_path`: this is a best-effort
/// usability guard, **not** a sandbox. See that function's docs for
/// the full TOCTOU discussion.
pub fn validate_safe_path(path: &Path) -> Result<PathBuf, CdpError> {
    canonicalize_safe_path(path)
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
    /// Override the Chromium profile directory. When unset, Plumb creates an
    /// isolated temporary profile per browser launch so concurrent or
    /// back-to-back drivers never contend on Chromium's SingletonLock.
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
    /// Opt-in: when no [`Self::executable_path`] is set and no system
    /// Chromium is detected, download Chrome-for-Testing pinned at
    /// [`MIN_SUPPORTED_CHROMIUM_MAJOR`] into a Plumb-managed cache
    /// directory and verify its SHA-256 before launch. Defaults to
    /// `false`. See [`fetcher`] for the security trade-offs.
    pub auto_fetch_chromium: bool,
    /// Override the auto-fetch cache directory. When `None`, Plumb
    /// resolves the platform default via [`fetcher::resolve_cache_dir`].
    /// Useful for tests that want a tempdir-scoped cache and for
    /// callers that ship Chromium alongside their app.
    pub auto_fetch_cache_dir: Option<PathBuf>,
}

/// Real Chromium-backed driver.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ChromiumDriver {
    options: ChromiumOptions,
}

struct ChromiumLaunch {
    config: BrowserConfig,
    profile_dir: Option<TempDir>,
}

impl ChromiumDriver {
    /// Build a driver with explicit options.
    #[must_use]
    pub fn new(options: ChromiumOptions) -> Self {
        Self { options }
    }

    fn browser_config(
        &self,
        target: &Target,
        resolved_executable: Option<&Path>,
    ) -> Result<ChromiumLaunch, CdpError> {
        // PRD §16: pinning launch args removes a class of nondeterminism
        // (scrollbar overlay differences across DPRs, OS-level scaling).
        let scale_factor_arg = format!("--force-device-scale-factor={}", target.device_pixel_ratio);
        let builder = BrowserConfig::builder()
            .new_headless_mode()
            .chrome_detection(DetectionOptions {
                msedge: false,
                unstable: false,
            })
            .request_timeout(CHROMIUMOXIDE_REQUEST_TIMEOUT)
            .launch_timeout(BROWSER_LAUNCH_TIMEOUT)
            .window_size(target.width, target.height)
            .viewport(None)
            .arg("--hide-scrollbars")
            .arg(scale_factor_arg);

        // Precedence:
        //   1. caller-resolved path (auto-fetch produced one),
        //   2. user-supplied `executable_path`,
        //   3. macOS `.app`-bundle priority list (see `chrome_path::detect`),
        //   4. chromiumoxide auto-detect (no `chrome_executable` call).
        let builder = if let Some(path) = resolved_executable {
            ensure_executable_path(path)?;
            builder.chrome_executable(path)
        } else if let Some(path) = &self.options.executable_path {
            ensure_executable_path(path)?;
            builder.chrome_executable(path)
        } else if let Some(path) = chrome_path::detect() {
            // No need to `ensure_executable_path` — `detect` only
            // returns paths that already passed an `is_file` probe.
            builder.chrome_executable(path)
        } else {
            builder
        };

        let (builder, profile_dir) =
            apply_user_data_dir(builder, self.options.user_data_dir.as_deref())?;

        let config = builder.build().map_err(|_| chromium_not_found())?;
        Ok(ChromiumLaunch {
            config,
            profile_dir,
        })
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

        let mut attempts = 0;
        loop {
            let result = self.snapshot_all_once(&targets).await;
            if attempts < TRANSIENT_CAPTURE_RETRIES
                && result
                    .as_ref()
                    .err()
                    .is_some_and(is_retryable_capture_timeout)
            {
                if let Err(err) = &result {
                    tracing::debug!(attempt = attempts + 1, error = %err, "retrying Chromium capture after transient timeout");
                }
                attempts += 1;
                continue;
            }
            return result;
        }
    }
}

impl ChromiumDriver {
    async fn snapshot_all_once(&self, targets: &[Target]) -> Result<Vec<PlumbSnapshot>, CdpError> {
        // Use the first target's dimensions and DPR for the initial
        // launch. Chromiumoxide's built-in viewport emulation is
        // disabled in `browser_config`; otherwise it sends its own
        // unlabelled page-level Emulation commands during target
        // initialization. The first unpinned target can reuse the
        // launch-pinned window/DPR, while later targets and explicit
        // `--dpr` pins still use Plumb's bounded/labeled override.
        let first = &targets[0];
        let resolved_executable = resolve_auto_fetch(&self.options).await?;
        let launch = self.browser_config(first, resolved_executable.as_deref())?;
        let mut session = RawChromiumSession::launch(launch).await?;
        let mut raw = RawCdpClient::connect(session.websocket_address()).await?;

        let result: Result<Vec<PlumbSnapshot>, CdpError> = async {
            validate_browser_version_raw(&mut raw).await?;
            let mut snapshots = Vec::with_capacity(targets.len());
            for (target_index, target) in targets.iter().enumerate() {
                let snap = capture_target_raw(
                    &mut raw,
                    target,
                    &self.options,
                    should_apply_viewport_override(target_index, target),
                )
                .await?;
                snapshots.push(snap);
            }
            Ok(snapshots)
        }
        .await;

        if let Err(cleanup_err) = session.shutdown(&mut raw).await {
            tracing::debug!(error = %cleanup_err, "failed to clean up Chromium session");
            if result.is_ok() {
                return Err(cleanup_err);
            }
        }

        result
    }
}

async fn capture_target_raw(
    cdp: &mut RawCdpClient,
    target: &Target,
    options: &ChromiumOptions,
    apply_viewport_override: bool,
) -> Result<PlumbSnapshot, CdpError> {
    let page = RawPage::create(
        cdp,
        CreateTargetParams {
            width: Some(i64::from(target.width)),
            height: Some(i64::from(target.height)),
            new_window: Some(true),
            ..CreateTargetParams::new(INITIAL_PAGE_URL)
        },
    )
    .await?;

    settle_initial_document().await;
    capture_on_raw_page(cdp, &page, target, options, apply_viewport_override).await
}

struct RawCdpClient {
    conn: Connection<CdpEventMessage>,
}

impl RawCdpClient {
    async fn connect(websocket_address: &str) -> Result<Self, CdpError> {
        let conn = with_timeout("CDP websocket connect", CDP_CONTROL_TIMEOUT, async {
            Connection::<CdpEventMessage>::connect(websocket_address)
                .await
                .map_err(driver_error)
        })
        .await?;
        Ok(Self { conn })
    }

    async fn execute<T: Command>(
        &mut self,
        session_id: Option<&SessionId>,
        cmd: T,
    ) -> Result<T::Response, CdpError> {
        let method = cmd.identifier();
        let call_id = self.submit(session_id, cmd)?;
        self.wait_for_response::<T>(call_id, method).await
    }

    fn submit<T: Command>(
        &mut self,
        session_id: Option<&SessionId>,
        cmd: T,
    ) -> Result<CallId, CdpError> {
        let method = cmd.identifier();
        let params = serde_json::to_value(cmd).map_err(serde_driver_error)?;
        self.conn
            .submit_command(method, session_id.cloned(), params)
            .map_err(serde_driver_error)
    }

    async fn execute_collecting_page_events<T: Command>(
        &mut self,
        session_id: &SessionId,
        cmd: T,
        events: &mut RawNavigationEvents,
    ) -> Result<T::Response, CdpError> {
        let method = cmd.identifier();
        let params = serde_json::to_value(cmd).map_err(serde_driver_error)?;
        let call_id = self
            .conn
            .submit_command(method.clone(), Some(session_id.clone()), params)
            .map_err(serde_driver_error)?;
        self.wait_for_response_collecting_page_events::<T>(call_id, method, session_id, events)
            .await
    }

    async fn wait_for_response<T: Command>(
        &mut self,
        call_id: CallId,
        method: MethodId,
    ) -> Result<T::Response, CdpError> {
        loop {
            match self.conn.next().await {
                Some(Ok(Message::Response(response))) if response.id == call_id => {
                    return raw_command_response::<T>(response, &method);
                }
                Some(Ok(Message::Response(_) | Message::Event(_))) => {}
                Some(Err(err)) => return Err(driver_error(err)),
                None => {
                    return Err(CdpError::Driver(Box::new(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        format!("{method} received no response from Chromium"),
                    ))));
                }
            }
        }
    }

    async fn wait_for_response_collecting_page_events<T: Command>(
        &mut self,
        call_id: CallId,
        method: MethodId,
        session_id: &SessionId,
        events: &mut RawNavigationEvents,
    ) -> Result<T::Response, CdpError> {
        loop {
            match self.conn.next().await {
                Some(Ok(Message::Response(response))) if response.id == call_id => {
                    return raw_command_response::<T>(response, &method);
                }
                Some(Ok(Message::Event(event))) => {
                    events.observe_message(&event, session_id);
                }
                Some(Ok(Message::Response(_))) => {}
                Some(Err(err)) => return Err(driver_error(err)),
                None => {
                    return Err(CdpError::Driver(Box::new(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        format!("{method} received no response from Chromium"),
                    ))));
                }
            }
        }
    }

    async fn collect_next_page_event(
        &mut self,
        session_id: &SessionId,
        events: &mut RawNavigationEvents,
    ) -> Result<(), CdpError> {
        loop {
            match self.conn.next().await {
                Some(Ok(Message::Event(event))) => {
                    if events.observe_message(&event, session_id) {
                        return Ok(());
                    }
                }
                Some(Ok(Message::Response(_))) => {}
                Some(Err(err)) => return Err(driver_error(err)),
                None => {
                    return Err(CdpError::Driver(Box::new(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "raw CDP event stream ended before navigation completed",
                    ))));
                }
            }
        }
    }
}

fn raw_command_response<T: Command>(
    response: Response,
    method: &MethodId,
) -> Result<T::Response, CdpError> {
    if let Some(result) = response.result {
        return T::response_from_value(result).map_err(serde_driver_error);
    }
    if let Some(err) = response.error {
        return Err(CdpError::Driver(Box::new(io::Error::other(format!(
            "{method} failed: {err}"
        )))));
    }
    Err(CdpError::Driver(Box::new(io::Error::other(format!(
        "{method} returned neither result nor error"
    )))))
}

struct RawPage {
    session_id: SessionId,
}

#[derive(Default)]
struct RawNavigationEvents {
    main_frame_url: Option<String>,
    dom_content_event: bool,
    load_event: bool,
}

impl RawNavigationEvents {
    fn observe_message(&mut self, event: &CdpEventMessage, session_id: &SessionId) -> bool {
        if event.session_id.as_deref() != Some(session_id.as_ref()) {
            return false;
        }
        match &event.params {
            CdpEvent::PageFrameNavigated(frame) if frame.frame.parent_id.is_none() => {
                self.observe_main_frame_url(&frame.frame.url);
                true
            }
            CdpEvent::PageDomContentEventFired(_) => {
                self.observe_dom_content_event();
                true
            }
            CdpEvent::PageLoadEventFired(_) => {
                self.observe_load_event();
                true
            }
            _ => false,
        }
    }

    fn observe_main_frame_url(&mut self, url: &str) {
        if url_has_navigated(url) {
            self.main_frame_url = Some(url.to_owned());
            self.dom_content_event = false;
            self.load_event = false;
        }
    }

    fn observe_dom_content_event(&mut self) {
        self.dom_content_event = true;
    }

    fn observe_load_event(&mut self) {
        self.load_event = true;
    }

    fn is_ready_for_capture(&self, allow_interactive: bool) -> bool {
        self.main_frame_url.is_some()
            && (self.load_event || (allow_interactive && self.dom_content_event))
    }

    fn has_navigated(&self) -> bool {
        self.main_frame_url.is_some()
    }

    fn is_chrome_error_page(&self) -> bool {
        self.main_frame_url
            .as_deref()
            .is_some_and(|url| url.starts_with("chrome-error:"))
    }

    fn main_frame_url(&self) -> Option<&str> {
        self.main_frame_url.as_deref()
    }
}

impl RawPage {
    async fn create(cdp: &mut RawCdpClient, params: CreateTargetParams) -> Result<Self, CdpError> {
        let target_id = with_timeout("Target.createTarget", TARGET_CREATE_TIMEOUT, async {
            cdp.execute(None, params)
                .await
                .map(|response| response.target_id)
        })
        .await
        .map_err(|err| target_lifecycle_error("Target.createTarget", &err))?;

        let attach = AttachToTargetParams::builder()
            .target_id(target_id)
            .flatten(true)
            .build()
            .map_err(driver_message)?;
        let session_id = with_timeout("Target.attachToTarget", TARGET_ATTACH_TIMEOUT, async {
            cdp.execute(None, attach)
                .await
                .map(|response| response.session_id)
        })
        .await
        .map_err(|err| target_lifecycle_error("Target.attachToTarget", &err))?;

        Ok(Self { session_id })
    }

    async fn execute<T: Command>(
        &self,
        cdp: &mut RawCdpClient,
        operation: &str,
        timeout: Duration,
        cmd: T,
    ) -> Result<T::Response, CdpError> {
        with_timeout(operation, timeout, async {
            cdp.execute(Some(&self.session_id), cmd).await
        })
        .await
    }

    async fn execute_collecting_page_events<T: Command>(
        &self,
        cdp: &mut RawCdpClient,
        operation: &str,
        timeout: Duration,
        cmd: T,
        events: &mut RawNavigationEvents,
    ) -> Result<T::Response, CdpError> {
        with_timeout(operation, timeout, async {
            cdp.execute_collecting_page_events(&self.session_id, cmd, events)
                .await
        })
        .await
    }

    async fn evaluate_value<T: serde::de::DeserializeOwned>(
        &self,
        cdp: &mut RawCdpClient,
        operation: &str,
        timeout: Duration,
        expression: &str,
    ) -> Result<T, CdpError> {
        let params = EvaluateParams::builder()
            .expression(expression)
            .await_promise(true)
            .return_by_value(true)
            .build()
            .map_err(driver_message)?;
        let result = self.execute(cdp, operation, timeout, params).await?;
        if let Some(exception) = result.exception_details {
            return Err(driver_error(
                chromiumoxide::error::CdpError::JavascriptException(Box::new(exception)),
            ));
        }
        let value = result.result.value.ok_or_else(|| {
            CdpError::Driver(Box::new(io::Error::other(format!(
                "{operation} returned no value"
            ))))
        })?;
        serde_json::from_value(value).map_err(serde_driver_error)
    }

    async fn evaluate_unit(
        &self,
        cdp: &mut RawCdpClient,
        operation: &str,
        timeout: Duration,
        expression: &str,
    ) -> Result<(), CdpError> {
        let params = EvaluateParams::builder()
            .expression(expression)
            .await_promise(true)
            .return_by_value(true)
            .build()
            .map_err(driver_message)?;
        let result = self.execute(cdp, operation, timeout, params).await?;
        if let Some(exception) = result.exception_details {
            return Err(driver_error(
                chromiumoxide::error::CdpError::JavascriptException(Box::new(exception)),
            ));
        }
        Ok(())
    }
}

async fn capture_on_raw_page(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    target: &Target,
    options: &ChromiumOptions,
    apply_viewport_override: bool,
) -> Result<PlumbSnapshot, CdpError> {
    if apply_viewport_override {
        apply_viewport_raw(cdp, page, target).await?;
    }
    let storage_state = pre_navigate_raw(cdp, page, target, options).await?;

    navigate_raw(cdp, page, target).await?;

    apply_post_navigate_waits_raw(cdp, page, target).await?;
    apply_storage_state_local_storage_raw(cdp, page, target, storage_state.as_ref()).await?;
    apply_deterministic_styles_raw(cdp, page, target).await?;

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

    let response = page
        .execute(
            cdp,
            "DOMSnapshot.captureSnapshot",
            SNAPSHOT_CAPTURE_TIMEOUT,
            params,
        )
        .await?;
    flatten_snapshot(target, &response)
}

async fn create_page_without_load_wait(
    browser: &Browser,
    params: CreateTargetParams,
) -> Result<Page, CdpError> {
    let target_id = with_timeout("Target.createTarget", TARGET_CREATE_TIMEOUT, async {
        browser
            .execute(params)
            .await
            .map(|response| response.result.target_id)
            .map_err(driver_error)
    })
    .await
    .map_err(|err| target_lifecycle_error("Target.createTarget", &err))?;

    with_timeout("Target.attachToTarget", TARGET_ATTACH_TIMEOUT, async {
        loop {
            match browser.get_page(target_id.clone()).await {
                Ok(page) => return Ok(page),
                Err(chromiumoxide::error::CdpError::NotFound) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(err) => return Err(driver_error(err)),
            }
        }
    })
    .await
    .map_err(|err| target_lifecycle_error("Target.attachToTarget", &err))
}

fn target_lifecycle_error(stage: &str, err: &CdpError) -> CdpError {
    let kind = if is_retryable_capture_timeout(err) {
        io::ErrorKind::TimedOut
    } else {
        io::ErrorKind::Other
    };
    CdpError::Driver(Box::new(io::Error::new(
        kind,
        format!("{stage} failed before navigation: {err}"),
    )))
}

/// Apply viewport, install pre-navigation state, navigate, wait for
/// final page state, apply deterministic styling, then capture a DOM
/// snapshot.
///
/// Shared between `ChromiumDriver::capture_target` and
/// [`PersistentBrowser::snapshot`] so that the per-target work is
/// expressed in exactly one place. The function is split into discrete
/// stages — `apply_viewport` (DPR + dimensions), `pre_navigate`
/// (cookies, headers, auth-script, storage-state cookies), `goto`,
/// waits, deterministic style injection, then capture.
async fn capture_on_page(
    page: &Page,
    target: &Target,
    options: &ChromiumOptions,
    apply_viewport_override: bool,
) -> Result<PlumbSnapshot, CdpError> {
    if apply_viewport_override {
        apply_viewport(page, target).await?;
    }
    // `pre_navigate` returns the parsed `StorageState` (when one is
    // configured) so the post-navigate localStorage step reuses the
    // same parsed value. Loading the file twice would open a
    // time-of-check / time-of-use race where the file changes between
    // cookie installation and localStorage replay.
    let storage_state = pre_navigate(page, target, options).await?;

    navigate_page(page, target.url.as_str()).await?;

    apply_post_navigate_waits(page, target).await?;
    apply_storage_state_local_storage(page, target, storage_state.as_ref()).await?;
    apply_deterministic_styles(page, target).await?;

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

    let response = with_timeout(
        "DOMSnapshot.captureSnapshot",
        SNAPSHOT_CAPTURE_TIMEOUT,
        async { page.execute(params).await.map_err(driver_error) },
    )
    .await?;
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
    _profile_dir: Option<TempDir>,
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
        let resolved_executable = resolve_auto_fetch(&options).await?;
        let launch = persistent_browser_config(&options, resolved_executable.as_deref())?;
        let ChromiumLaunch {
            config,
            profile_dir,
        } = launch;
        let (mut browser, handler) =
            with_timeout("Chromium launch", BROWSER_LAUNCH_TIMEOUT, async {
                Browser::launch(config).await.map_err(map_launch_error)
            })
            .await?;
        let handler_task = poll_handler(handler);

        // Validate the version before stashing the browser in `Arc` —
        // on failure, explicitly close/wait before the isolated
        // profile is dropped so Windows does not retain locked files.
        if let Err(err) = validate_browser_version(&browser).await {
            if let Err(cleanup_err) =
                cleanup_failed_persistent_launch(&mut browser, handler_task).await
            {
                tracing::debug!(
                    error = %cleanup_err,
                    "failed to clean up Chromium after version validation failure"
                );
            }
            let _profile_dir = profile_dir;
            return Err(err);
        }

        Ok(Self {
            inner: Arc::new(PersistentBrowserInner {
                browser,
                handler_task: Mutex::new(Some(handler_task)),
                _profile_dir: profile_dir,
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
        let mut attempts = 0;
        loop {
            let result = self.snapshot_once(&target).await;
            if attempts < TRANSIENT_CAPTURE_RETRIES
                && result
                    .as_ref()
                    .err()
                    .is_some_and(is_retryable_capture_timeout)
            {
                if let Err(err) = &result {
                    tracing::debug!(attempt = attempts + 1, error = %err, "retrying persistent Chromium capture after transient timeout");
                }
                attempts += 1;
                continue;
            }
            return result;
        }
    }

    async fn snapshot_once(&self, target: &Target) -> Result<PlumbSnapshot, CdpError> {
        let ctx_id = with_timeout("Target.createBrowserContext", CDP_CONTROL_TIMEOUT, async {
            self.inner
                .browser
                .create_browser_context(CreateBrowserContextParams::default())
                .await
                .map_err(driver_error)
        })
        .await?;

        let result: Result<PlumbSnapshot, CdpError> = async {
            let create_params = CreateTargetParams {
                url: INITIAL_PAGE_URL.to_string(),
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
            let page = create_page_without_load_wait(&self.inner.browser, create_params).await?;
            settle_initial_document().await;
            capture_on_page(&page, target, &self.inner.options, true).await
        }
        .await;

        // Always dispose the incognito context, even on failure. Mirror
        // the swallow-and-log pattern from `ChromiumSession::shutdown`
        // so cleanup errors never mask the underlying snapshot result.
        if let Err(err) = with_timeout("Target.disposeBrowserContext", CDP_CONTROL_TIMEOUT, async {
            self.inner
                .browser
                .dispose_browser_context(ctx_id)
                .await
                .map_err(driver_error)
        })
        .await
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

fn apply_user_data_dir(
    builder: BrowserConfigBuilder,
    user_data_dir: Option<&Path>,
) -> Result<(BrowserConfigBuilder, Option<TempDir>), CdpError> {
    if let Some(profile) = user_data_dir {
        return Ok((builder.user_data_dir(profile), None));
    }

    let profile = tempfile::Builder::new()
        .prefix("plumb-chromium-")
        .tempdir()
        .map_err(|err| {
            CdpError::Driver(Box::new(io::Error::other(format!(
                "create isolated Chromium profile: {err}"
            ))))
        })?;
    let builder = builder.user_data_dir(profile.path());
    Ok((builder, Some(profile)))
}

fn persistent_browser_config(
    options: &ChromiumOptions,
    resolved_executable: Option<&Path>,
) -> Result<ChromiumLaunch, CdpError> {
    // PRD §16: pinning launch args removes a class of nondeterminism
    // (scrollbar overlay differences across DPRs, OS-level scaling).
    // `PersistentBrowser` does not fix a launch-time DPR — every
    // snapshot calls `Emulation.setDeviceMetricsOverride` to drive
    // both viewport and DPR per-call.
    let builder = BrowserConfig::builder()
        .new_headless_mode()
        .chrome_detection(DetectionOptions {
            msedge: false,
            unstable: false,
        })
        .request_timeout(CHROMIUMOXIDE_REQUEST_TIMEOUT)
        .launch_timeout(BROWSER_LAUNCH_TIMEOUT)
        .window_size(1280, 800)
        .viewport(None)
        .arg("--hide-scrollbars");

    // Same precedence rule as `ChromiumDriver::browser_config`.
    let builder = if let Some(path) = resolved_executable {
        ensure_executable_path(path)?;
        builder.chrome_executable(path)
    } else if let Some(path) = &options.executable_path {
        ensure_executable_path(path)?;
        builder.chrome_executable(path)
    } else if let Some(path) = chrome_path::detect() {
        builder.chrome_executable(path)
    } else {
        builder
    };

    let (builder, profile_dir) = apply_user_data_dir(builder, options.user_data_dir.as_deref())?;

    let config = builder.build().map_err(|_| chromium_not_found())?;
    Ok(ChromiumLaunch {
        config,
        profile_dir,
    })
}

/// When auto-fetch is enabled and the user didn't pin an
/// `executable_path`, resolve the cache directory and ensure a fetched
/// Chromium binary lives there. Returns the executable path the
/// `BrowserConfig` should pin; `None` means "fall through to whatever
/// the user supplied or to chromiumoxide's auto-detect."
///
/// Pure precedence rule: an explicit `executable_path` always wins
/// over auto-fetch. The two are not allowed to collide — if both are
/// set, the user's path is used and the fetcher is skipped.
async fn resolve_auto_fetch(options: &ChromiumOptions) -> Result<Option<PathBuf>, CdpError> {
    if !options.auto_fetch_chromium || options.executable_path.is_some() {
        return Ok(None);
    }
    let cache_dir = if let Some(dir) = options.auto_fetch_cache_dir.clone() {
        dir
    } else {
        fetcher::resolve_cache_dir()?
    };
    let path = fetcher::ensure_chromium(&cache_dir).await?;
    Ok(Some(path))
}

fn should_apply_viewport_override(target_index: usize, target: &Target) -> bool {
    target_index != 0 || target.pin_dpr.is_some()
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
    with_timeout(
        "Emulation.setDeviceMetricsOverride",
        PAGE_COMMAND_TIMEOUT,
        async { page.execute(params).await.map(|_| ()).map_err(driver_error) },
    )
    .await?;
    Ok(())
}

async fn apply_viewport_raw(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    target: &Target,
) -> Result<(), CdpError> {
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
    page.execute(
        cdp,
        "Emulation.setDeviceMetricsOverride",
        PAGE_COMMAND_TIMEOUT,
        params,
    )
    .await?;
    Ok(())
}

/// All work that must happen on a fresh page before navigation.
///
/// Runs in this fixed order so behavior matches what users expect:
/// 1. Auth script — runs before any page script, so the page-side
///    bootstrap can set window globals before the SPA boots.
/// 2. Cookies and HTTP headers — set on the network layer before the
///    very first request leaves Chromium.
/// 3. Storage-state cookies — same network layer; localStorage entries
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

async fn pre_navigate_raw(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    target: &Target,
    options: &ChromiumOptions,
) -> Result<Option<StorageState>, CdpError> {
    if let Some(script_path) = options.auth_script.as_deref() {
        inject_auth_script_raw(cdp, page, script_path).await?;
    }
    if !options.headers.is_empty() {
        install_extra_headers_raw(cdp, page, &options.headers).await?;
    }
    if !options.cookies.is_empty() {
        install_cookies_raw(cdp, page, &options.cookies, target.url.as_str()).await?;
    }
    let storage_state = if let Some(state_path) = options.storage_state.as_deref() {
        let state = StorageState::load_from_path(state_path)?;
        install_storage_state_cookies_raw(cdp, page, &state).await?;
        Some(state)
    } else {
        None
    };
    Ok(storage_state)
}

#[derive(Debug, Deserialize)]
struct NavigationState {
    href: String,
    #[serde(rename = "readyState")]
    ready_state: String,
    #[serde(rename = "isChromeErrorPage", default)]
    is_chrome_error_page: bool,
}

async fn navigate_page(page: &Page, url: &str) -> Result<(), CdpError> {
    let initial_result = match navigation_method_for_url(url) {
        NavigationMethod::ChromiumoxideGoto => {
            with_timeout("Page.navigate", DOCUMENT_READY_TIMEOUT, async {
                page.goto(url).await.map(|_| ()).map_err(driver_error)
            })
            .await
        }
        NavigationMethod::CdpNavigate => {
            with_timeout("Page.navigate", PAGE_COMMAND_TIMEOUT, async {
                page.execute(NavigateParams::new(url))
                    .await
                    .map_err(driver_error)
                    .and_then(|response| {
                        if let Some(error_text) = &response.error_text {
                            Err(CdpError::Driver(Box::new(io::Error::other(format!(
                                "Page.navigate failed: {error_text}"
                            )))))
                        } else {
                            Ok(())
                        }
                    })
            })
            .await
        }
        NavigationMethod::LocationAssign => {
            let script = navigation_assignment_script(url)?;
            with_timeout(
                "navigation location assignment",
                NAVIGATION_ASSIGNMENT_TIMEOUT,
                async {
                    page.evaluate(script.as_str())
                        .await
                        .map(|_| ())
                        .map_err(driver_error)
                },
            )
            .await
        }
    };

    wait_for_document_ready(page, navigation_display_url(url), initial_result.err()).await
}

async fn navigate_raw(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    target: &Target,
) -> Result<(), CdpError> {
    let page_events_enabled = enable_raw_page_events(cdp, page).await;
    let mut events = RawNavigationEvents::default();
    let initial_result = if uses_raw_async_page_navigate(target.url.as_str()) {
        submit_raw_page_navigate(cdp, page, target.url.as_str())
    } else {
        navigate_raw_by_page_navigate(
            cdp,
            page,
            target.url.as_str(),
            page_events_enabled,
            &mut events,
        )
        .await
    };

    wait_for_document_ready_raw(
        cdp,
        page,
        navigation_display_url(target.url.as_str()),
        initial_result.err(),
        target.wait_for_selector.is_some(),
        page_events_enabled.then_some(events),
    )
    .await
}

async fn navigate_raw_by_page_navigate(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    url: &str,
    page_events_enabled: bool,
    events: &mut RawNavigationEvents,
) -> Result<(), CdpError> {
    if page_events_enabled {
        page.execute_collecting_page_events(
            cdp,
            "Page.navigate",
            PAGE_COMMAND_TIMEOUT,
            NavigateParams::new(url),
            events,
        )
        .await
    } else {
        page.execute(
            cdp,
            "Page.navigate",
            PAGE_COMMAND_TIMEOUT,
            NavigateParams::new(url),
        )
        .await
    }
    .and_then(|response| {
        if let Some(error_text) = response.error_text {
            Err(CdpError::Driver(Box::new(io::Error::other(format!(
                "Page.navigate failed: {error_text}"
            )))))
        } else {
            Ok(())
        }
    })
}

fn submit_raw_page_navigate(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    url: &str,
) -> Result<(), CdpError> {
    cdp.submit(Some(&page.session_id), NavigateParams::new(url))?;
    Ok(())
}

async fn enable_raw_page_events(cdp: &mut RawCdpClient, page: &RawPage) -> bool {
    match page
        .execute(
            cdp,
            "Page.enable",
            PAGE_ENABLE_TIMEOUT,
            PageEnableParams::default(),
        )
        .await
    {
        Ok(_) => true,
        Err(err) => {
            tracing::debug!(error = %err, "Page.enable failed; falling back to raw ready-state polling");
            false
        }
    }
}

fn uses_chromiumoxide_goto(url: &str) -> bool {
    url.starts_with("file://")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavigationMethod {
    ChromiumoxideGoto,
    CdpNavigate,
    LocationAssign,
}

fn navigation_method_for_url(url: &str) -> NavigationMethod {
    if uses_chromiumoxide_goto(url) {
        NavigationMethod::ChromiumoxideGoto
    } else if url.starts_with("data:") {
        NavigationMethod::CdpNavigate
    } else {
        NavigationMethod::LocationAssign
    }
}

fn uses_raw_async_page_navigate(url: &str) -> bool {
    matches!(
        navigation_method_for_url(url),
        NavigationMethod::LocationAssign
    )
}

fn navigation_assignment_script(url: &str) -> Result<String, CdpError> {
    let quoted_url = serde_json::to_string(url).map_err(|err| {
        CdpError::Driver(Box::new(io::Error::other(format!(
            "serialize navigation URL: {err}"
        ))))
    })?;
    Ok(format!("window.location.assign({quoted_url});"))
}

async fn wait_for_document_ready(
    page: &Page,
    display_url: &str,
    initial_error: Option<CdpError>,
) -> Result<(), CdpError> {
    let mut last_state_error = None;
    let attempt = async {
        loop {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if poll_document_ready(page, display_url, &mut last_state_error).await? {
                return Ok(());
            }
        }
    };

    if let Ok(result) = tokio::time::timeout(DOCUMENT_READY_TIMEOUT, attempt).await {
        return result;
    }

    let reason = navigation_ready_timeout_reason(
        display_url,
        initial_error.as_ref().map(ToString::to_string).as_deref(),
        last_state_error.as_deref(),
    );
    Err(CdpError::Driver(Box::new(io::Error::other(reason))))
}

async fn settle_initial_document() {
    // Avoid probing the bootstrap page before the real navigation. On
    // macOS CFT 150, pre-navigation probes and interrupted data: loads
    // can make the subsequent Page.navigate unreliable.
    tokio::time::sleep(INITIAL_DOCUMENT_SETTLE_DELAY).await;
}

async fn wait_for_document_ready_raw(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    display_url: &str,
    initial_error: Option<CdpError>,
    allow_interactive: bool,
    events: Option<RawNavigationEvents>,
) -> Result<(), CdpError> {
    let Some(mut events) = events else {
        return wait_for_document_ready_raw_by_polling(
            cdp,
            page,
            display_url,
            initial_error,
            allow_interactive,
        )
        .await;
    };

    let mut last_state_error = None;
    let mut event_wait_timed_out = false;
    if !events.is_ready_for_capture(allow_interactive) {
        match wait_for_raw_navigation_events(cdp, page, &mut events, allow_interactive).await {
            Ok(()) => {}
            Err(err) => {
                let err = err.to_string();
                event_wait_timed_out =
                    err == timeout_reason("raw navigation page event", DOCUMENT_READY_TIMEOUT);
                last_state_error = Some(err);
            }
        }
    }

    if events.is_chrome_error_page() {
        return Err(chrome_error_page_error(
            display_url,
            events
                .main_frame_url()
                .unwrap_or("chrome-error://chromewebdata/"),
        ));
    }

    match tokio::time::timeout(
        NAVIGATION_STATE_READ_TIMEOUT,
        read_navigation_state_raw(cdp, page),
    )
    .await
    {
        Ok(Ok(state)) if state.is_chrome_error_page => {
            return Err(chrome_error_page_error(display_url, &state.href));
        }
        Ok(Ok(state)) if document_is_ready_for_capture(&state, allow_interactive) => return Ok(()),
        Ok(Ok(state))
            if events.is_ready_for_capture(allow_interactive) && document_has_navigated(&state) =>
        {
            return Ok(());
        }
        Ok(Ok(_)) if events.is_ready_for_capture(allow_interactive) => return Ok(()),
        Ok(Ok(_)) => {}
        Ok(Err(err)) if events.is_ready_for_capture(allow_interactive) => {
            tracing::debug!(error = %err, "raw navigation state check failed after page event readiness");
            return Ok(());
        }
        Ok(Err(err)) => last_state_error = Some(err.to_string()),
        Err(_) if events.is_ready_for_capture(allow_interactive) => {
            tracing::debug!("raw navigation state check timed out after page event readiness");
            return Ok(());
        }
        Err(_) if event_wait_timed_out && events.has_navigated() => {
            tracing::debug!(
                "raw navigation state check timed out after main-frame navigation event"
            );
            return Ok(());
        }
        Err(_) => {
            last_state_error = Some(timeout_reason(
                "navigation state read",
                NAVIGATION_STATE_READ_TIMEOUT,
            ));
        }
    }

    let reason = navigation_ready_timeout_reason(
        display_url,
        initial_error.as_ref().map(ToString::to_string).as_deref(),
        last_state_error.as_deref(),
    );
    Err(CdpError::Driver(Box::new(io::Error::other(reason))))
}

async fn wait_for_document_ready_raw_by_polling(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    display_url: &str,
    initial_error: Option<CdpError>,
    allow_interactive: bool,
) -> Result<(), CdpError> {
    let mut last_state_error = None;
    let attempt = async {
        loop {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if poll_document_ready_raw(
                cdp,
                page,
                display_url,
                &mut last_state_error,
                allow_interactive,
            )
            .await?
            {
                return Ok(());
            }
        }
    };

    if let Ok(result) = tokio::time::timeout(DOCUMENT_READY_TIMEOUT, attempt).await {
        return result;
    }

    let reason = navigation_ready_timeout_reason(
        display_url,
        initial_error.as_ref().map(ToString::to_string).as_deref(),
        last_state_error.as_deref(),
    );
    Err(CdpError::Driver(Box::new(io::Error::other(reason))))
}

async fn wait_for_raw_navigation_events(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    events: &mut RawNavigationEvents,
    allow_interactive: bool,
) -> Result<(), CdpError> {
    let attempt = async {
        loop {
            if events.is_ready_for_capture(allow_interactive) {
                return Ok(());
            }
            cdp.collect_next_page_event(&page.session_id, events)
                .await?;
        }
    };

    match tokio::time::timeout(DOCUMENT_READY_TIMEOUT, attempt).await {
        Ok(result) => result,
        Err(_) => Err(CdpError::Driver(Box::new(io::Error::other(
            timeout_reason("raw navigation page event", DOCUMENT_READY_TIMEOUT),
        )))),
    }
}

async fn poll_document_ready_raw(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    display_url: &str,
    last_state_error: &mut Option<String>,
    allow_interactive: bool,
) -> Result<bool, CdpError> {
    match tokio::time::timeout(
        NAVIGATION_STATE_READ_TIMEOUT,
        read_navigation_state_raw(cdp, page),
    )
    .await
    {
        Ok(Ok(state)) if state.is_chrome_error_page => {
            Err(chrome_error_page_error(display_url, &state.href))
        }
        Ok(Ok(state)) if document_is_ready_for_capture(&state, allow_interactive) => Ok(true),
        Ok(Ok(_)) => Ok(false),
        Ok(Err(err)) => {
            *last_state_error = Some(err.to_string());
            Ok(false)
        }
        Err(_) => {
            *last_state_error = Some(timeout_reason(
                "navigation state read",
                NAVIGATION_STATE_READ_TIMEOUT,
            ));
            Ok(false)
        }
    }
}

async fn poll_document_ready(
    page: &Page,
    display_url: &str,
    last_state_error: &mut Option<String>,
) -> Result<bool, CdpError> {
    match tokio::time::timeout(NAVIGATION_STATE_READ_TIMEOUT, read_navigation_state(page)).await {
        Ok(Ok(state)) if state.is_chrome_error_page => {
            Err(chrome_error_page_error(display_url, &state.href))
        }
        Ok(Ok(state)) if document_is_loaded(&state) => Ok(true),
        Ok(Ok(_)) => Ok(false),
        Ok(Err(err)) => {
            *last_state_error = Some(err.to_string());
            Ok(false)
        }
        Err(_) => {
            *last_state_error = Some(timeout_reason(
                "navigation state read",
                NAVIGATION_STATE_READ_TIMEOUT,
            ));
            Ok(false)
        }
    }
}

fn chrome_error_page_error(display_url: &str, error_href: &str) -> CdpError {
    CdpError::Driver(Box::new(io::Error::other(format!(
        "navigation to `{display_url}` failed: Chrome rendered error page `{error_href}`"
    ))))
}

fn document_is_loaded(state: &NavigationState) -> bool {
    document_has_navigated(state) && state.ready_state == "complete"
}

fn document_is_ready_for_capture(state: &NavigationState, allow_interactive: bool) -> bool {
    document_has_navigated(state)
        && (state.ready_state == "complete"
            || (allow_interactive && state.ready_state == "interactive"))
}

fn document_has_navigated(state: &NavigationState) -> bool {
    url_has_navigated(&state.href) && !state.is_chrome_error_page
}

fn url_has_navigated(url: &str) -> bool {
    url != INITIAL_PAGE_URL
}

async fn read_navigation_state(page: &Page) -> Result<NavigationState, CdpError> {
    let result = page
        .evaluate(
            "JSON.stringify({
                href: window.location.href,
                readyState: document.readyState,
                isChromeErrorPage: window.location.protocol === 'chrome-error:'
                    || document.getElementById('main-frame-error') !== null
            })",
        )
        .await
        .map_err(driver_error)?;
    let raw: String = result.into_value().map_err(|err| {
        CdpError::Driver(Box::new(io::Error::other(format!(
            "read navigation state: {err}"
        ))))
    })?;
    parse_navigation_state(&raw)
}

async fn read_navigation_state_raw(
    cdp: &mut RawCdpClient,
    page: &RawPage,
) -> Result<NavigationState, CdpError> {
    let raw: String = page
        .evaluate_value(
            cdp,
            "Runtime.evaluate navigation state",
            PAGE_COMMAND_TIMEOUT,
            "JSON.stringify({
                href: window.location.href,
                readyState: document.readyState,
                isChromeErrorPage: window.location.protocol === 'chrome-error:'
                    || document.getElementById('main-frame-error') !== null
            })",
        )
        .await?;
    parse_navigation_state(&raw)
}

fn parse_navigation_state(raw: &str) -> Result<NavigationState, CdpError> {
    serde_json::from_str(raw).map_err(|err| {
        CdpError::Driver(Box::new(io::Error::other(format!(
            "parse navigation state `{raw}`: {err}"
        ))))
    })
}

fn navigation_ready_timeout_reason(
    display_url: &str,
    initial_error: Option<&str>,
    last_state_error: Option<&str>,
) -> String {
    let mut reason = format!(
        "navigation to `{display_url}` exhausted {} ready-state budget",
        timeout_budget_label(DOCUMENT_READY_TIMEOUT)
    );
    if let Some(err) = initial_error {
        reason.push_str(" after initial location assignment failed: ");
        reason.push_str(err);
    }
    if let Some(err) = last_state_error {
        reason.push_str("; last navigation state read failed: ");
        reason.push_str(err);
    }
    reason
}

fn navigation_display_url(url: &str) -> &str {
    if url.starts_with("data:") {
        "data:<redacted>"
    } else {
        url
    }
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

async fn apply_post_navigate_waits_raw(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    target: &Target,
) -> Result<(), CdpError> {
    if let Some(selector) = target.wait_for_selector.as_deref() {
        wait_for_selector_raw(cdp, page, selector).await?;
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
            let key = serde_json::to_string(&entry.name).map_err(|err| {
                CdpError::MalformedStorageState {
                    path: PathBuf::new(),
                    reason: format!("could not serialize key: {err}"),
                }
            })?;
            let value = serde_json::to_string(&entry.value).map_err(|err| {
                CdpError::MalformedStorageState {
                    path: PathBuf::new(),
                    reason: format!("could not serialize value: {err}"),
                }
            })?;
            let script = format!("window.localStorage.setItem({key}, {value});");
            with_timeout(
                "Runtime.evaluate localStorage",
                PAGE_COMMAND_TIMEOUT,
                async {
                    page.evaluate(script.as_str())
                        .await
                        .map(|_| ())
                        .map_err(driver_error)
                },
            )
            .await?;
        }
    }
    Ok(())
}

async fn apply_storage_state_local_storage_raw(
    cdp: &mut RawCdpClient,
    page: &RawPage,
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
            let key = serde_json::to_string(&entry.name).map_err(|err| {
                CdpError::MalformedStorageState {
                    path: PathBuf::new(),
                    reason: format!("could not serialize key: {err}"),
                }
            })?;
            let value = serde_json::to_string(&entry.value).map_err(|err| {
                CdpError::MalformedStorageState {
                    path: PathBuf::new(),
                    reason: format!("could not serialize value: {err}"),
                }
            })?;
            let script = format!("window.localStorage.setItem({key}, {value});");
            page.evaluate_unit(
                cdp,
                "Runtime.evaluate localStorage",
                PAGE_COMMAND_TIMEOUT,
                script.as_str(),
            )
            .await?;
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

async fn apply_deterministic_styles(page: &Page, target: &Target) -> Result<(), CdpError> {
    let Some(source) = deterministic_style_source(target) else {
        return Ok(());
    };

    with_timeout(
        "Runtime.evaluate deterministic styles",
        PAGE_COMMAND_TIMEOUT,
        async {
            page.evaluate(source.as_str())
                .await
                .map(|_| ())
                .map_err(driver_error)
        },
    )
    .await?;
    Ok(())
}

async fn apply_deterministic_styles_raw(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    target: &Target,
) -> Result<(), CdpError> {
    let Some(source) = deterministic_style_source(target) else {
        return Ok(());
    };

    page.evaluate_unit(
        cdp,
        "Runtime.evaluate deterministic styles",
        PAGE_COMMAND_TIMEOUT,
        source.as_str(),
    )
    .await?;
    Ok(())
}

fn deterministic_style_source(target: &Target) -> Option<String> {
    if !target.disable_animations && !target.hide_scrollbars {
        return None;
    }

    let mut css = String::new();
    if target.disable_animations {
        // PRD §16 determinism mitigation: transitions/animations should
        // not race with `captureSnapshot` and produce different bounds
        // across runs.
        css.push_str(
            "*, *::before, *::after { \
            animation-duration: 0s !important; \
            animation-delay: 0s !important; \
            transition-duration: 0s !important; \
            transition-delay: 0s !important; \
            caret-color: transparent !important; \
        }",
        );
    }
    if target.hide_scrollbars {
        // The `--hide-scrollbars` Chromium launch arg is the first line
        // of defense; this CSS covers cases where the launch arg alone
        // is not honored or the page paints custom scrollbars.
        css.push_str(
            "html { overflow: hidden !important; } \
            ::-webkit-scrollbar { display: none !important; }",
        );
    }

    let css_literal = serde_json::to_string(&css).ok()?;
    Some(format!(
        "(() => {{ \
            const style = document.createElement('style'); \
            style.setAttribute('data-plumb-deterministic-style', 'true'); \
            style.textContent = {css_literal}; \
            (document.head || document.documentElement).appendChild(style); \
        }})();"
    ))
}

/// Read `path` (validated as a `.js` file under the CWD) and register
/// it as `Page.addScriptToEvaluateOnNewDocument` so it runs before any
/// page script.
///
/// # Security boundary
///
/// The safe-path check via `canonicalize_safe_path` is best-effort
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

async fn inject_auth_script_raw(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    path: &Path,
) -> Result<(), CdpError> {
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
    add_script_to_evaluate_on_new_document_raw(cdp, page, &source).await
}

async fn add_script_to_evaluate_on_new_document(page: &Page, source: &str) -> Result<(), CdpError> {
    let params = add_script_to_evaluate_params(source);
    with_timeout(
        "Page.addScriptToEvaluateOnNewDocument",
        PAGE_COMMAND_TIMEOUT,
        async { page.execute(params).await.map(|_| ()).map_err(driver_error) },
    )
    .await?;
    Ok(())
}

async fn add_script_to_evaluate_on_new_document_raw(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    source: &str,
) -> Result<(), CdpError> {
    let params = add_script_to_evaluate_params(source);
    page.execute(
        cdp,
        "Page.addScriptToEvaluateOnNewDocument",
        PAGE_COMMAND_TIMEOUT,
        params,
    )
    .await?;
    Ok(())
}

fn add_script_to_evaluate_params(source: &str) -> AddScriptToEvaluateOnNewDocumentParams {
    AddScriptToEvaluateOnNewDocumentParams {
        source: source.to_owned(),
        world_name: None,
        include_command_line_api: None,
        run_immediately: None,
    }
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
    with_timeout("Network.setExtraHTTPHeaders", PAGE_COMMAND_TIMEOUT, async {
        page.execute(params).await.map(|_| ()).map_err(driver_error)
    })
    .await?;
    Ok(())
}

async fn install_extra_headers_raw(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    headers: &[(String, String)],
) -> Result<(), CdpError> {
    let mut entries: Vec<(String, String)> = headers.to_vec();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut object = serde_json::Map::with_capacity(entries.len());
    for (name, value) in entries {
        validate_header_name(&name)?;
        validate_no_ctl(&value, "value", "header")?;
        object.insert(name, serde_json::Value::String(value));
    }
    let params = SetExtraHttpHeadersParams::new(Headers::new(serde_json::Value::Object(object)));
    page.execute(
        cdp,
        "Network.setExtraHTTPHeaders",
        PAGE_COMMAND_TIMEOUT,
        params,
    )
    .await?;
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
    with_timeout("Network.setCookies", PAGE_COMMAND_TIMEOUT, async {
        page.execute(params).await.map(|_| ()).map_err(driver_error)
    })
    .await?;
    Ok(())
}

async fn install_cookies_raw(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    cookies: &[Cookie],
    default_url: &str,
) -> Result<(), CdpError> {
    let mut sorted: Vec<Cookie> = cookies.to_vec();
    sorted.sort_by(|a, b| {
        (a.name.as_str(), a.value.as_str()).cmp(&(b.name.as_str(), b.value.as_str()))
    });
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
    page.execute(cdp, "Network.setCookies", PAGE_COMMAND_TIMEOUT, params)
        .await?;
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
    with_timeout(
        "Network.setCookies storageState",
        PAGE_COMMAND_TIMEOUT,
        async {
            page.execute(SetCookiesParams::new(params))
                .await
                .map(|_| ())
                .map_err(driver_error)
        },
    )
    .await?;
    Ok(())
}

async fn install_storage_state_cookies_raw(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    state: &StorageState,
) -> Result<(), CdpError> {
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
    page.execute(
        cdp,
        "Network.setCookies storageState",
        PAGE_COMMAND_TIMEOUT,
        SetCookiesParams::new(params),
    )
    .await?;
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

async fn wait_for_selector_raw(
    cdp: &mut RawCdpClient,
    page: &RawPage,
    selector: &str,
) -> Result<(), CdpError> {
    let selector = serde_json::to_string(selector).map_err(serde_driver_error)?;
    let script = format!("document.querySelector({selector}) !== null");
    let attempt = async {
        loop {
            match page
                .evaluate_value::<bool>(
                    cdp,
                    "Runtime.evaluate wait_for_selector",
                    PAGE_COMMAND_TIMEOUT,
                    script.as_str(),
                )
                .await
            {
                Ok(true) => return Ok::<(), CdpError>(()),
                Ok(false) | Err(_) => {
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

struct RawChromiumSession {
    browser: Browser,
    profile_dir: Option<TempDir>,
}

impl RawChromiumSession {
    async fn launch(launch: ChromiumLaunch) -> Result<Self, CdpError> {
        let (browser, _handler) = with_timeout("Chromium launch", BROWSER_LAUNCH_TIMEOUT, async {
            Browser::launch(launch.config)
                .await
                .map_err(map_launch_error)
        })
        .await?;
        Ok(Self {
            browser,
            profile_dir: launch.profile_dir,
        })
    }

    fn websocket_address(&self) -> &str {
        self.browser.websocket_address()
    }

    async fn shutdown(&mut self, cdp: &mut RawCdpClient) -> Result<(), CdpError> {
        let cleanup_result = close_raw_browser_best_effort(cdp, &mut self.browser).await;
        let _profile_dir = self.profile_dir.take();
        cleanup_result
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

async fn with_timeout<T, F>(operation: &str, timeout: Duration, future: F) -> Result<T, CdpError>
where
    F: Future<Output = Result<T, CdpError>>,
{
    match tokio::time::timeout(timeout, future).await {
        Ok(result) => result.map_err(|err| contextualize_request_timeout(operation, err)),
        Err(_) => Err(timeout_error(operation, timeout)),
    }
}

fn contextualize_request_timeout(operation: &str, err: CdpError) -> CdpError {
    let CdpError::Driver(source) = &err else {
        return err;
    };

    if matches!(
        source.downcast_ref::<chromiumoxide::error::CdpError>(),
        Some(chromiumoxide::error::CdpError::Timeout)
    ) {
        return CdpError::Driver(Box::new(io::Error::new(
            io::ErrorKind::TimedOut,
            format!(
                "{operation} hit Chromiumoxide request budget ({})",
                timeout_budget_label(CHROMIUMOXIDE_REQUEST_TIMEOUT)
            ),
        )));
    }

    err
}

fn is_retryable_capture_timeout(err: &CdpError) -> bool {
    let CdpError::Driver(source) = err else {
        return false;
    };

    if matches!(
        source.downcast_ref::<chromiumoxide::error::CdpError>(),
        Some(chromiumoxide::error::CdpError::Timeout)
    ) {
        return true;
    }

    source.downcast_ref::<io::Error>().is_some_and(|err| {
        err.kind() == io::ErrorKind::TimedOut || is_startup_navigation_abort(err)
    })
}

fn is_startup_navigation_abort(err: &io::Error) -> bool {
    if err.kind() != io::ErrorKind::Other {
        return false;
    }

    let message = err.to_string();
    message.contains("exhausted 30s ready-state budget")
        && message.contains("after initial location assignment failed:")
        && message.contains("Page.navigate failed: net::ERR_ABORTED")
        && message.contains("last navigation state read failed: navigation state read exceeded")
}

fn timeout_error(operation: &str, timeout: Duration) -> CdpError {
    CdpError::Driver(Box::new(io::Error::new(
        io::ErrorKind::TimedOut,
        timeout_reason(operation, timeout),
    )))
}

fn timeout_reason(operation: &str, timeout: Duration) -> String {
    format!(
        "{operation} exceeded {} budget",
        timeout_budget_label(timeout)
    )
}

fn timeout_budget_label(timeout: Duration) -> String {
    if timeout.as_millis().is_multiple_of(1_000) {
        format!("{}s", timeout.as_secs())
    } else {
        format!("{}ms", timeout.as_millis())
    }
}

async fn cleanup_failed_persistent_launch(
    browser: &mut Browser,
    handler_task: JoinHandle<()>,
) -> Result<(), CdpError> {
    let close_result = close_browser_best_effort(browser).await;
    handler_task.abort();
    if let Err(join_err) = handler_task.await
        && !join_err.is_cancelled()
    {
        tracing::debug!(error = %join_err, "Chromium handler task failed");
    }

    close_result
}

async fn close_browser_best_effort(browser: &mut Browser) -> Result<(), CdpError> {
    if let Err(err) = with_timeout("Browser.close", BROWSER_CLOSE_TIMEOUT, async {
        browser.close().await.map_err(driver_error)
    })
    .await
    {
        tracing::debug!(error = %err, "failed to close Chromium");
    }

    if let Err(wait_err) = with_timeout("Chromium process wait", BROWSER_WAIT_TIMEOUT, async {
        browser.wait().await.map_err(io_error)
    })
    .await
    {
        tracing::debug!(error = %wait_err, "failed to wait for Chromium process");
        kill_browser(browser).await?;
        with_timeout(
            "Chromium process wait after kill",
            BROWSER_WAIT_TIMEOUT,
            async { browser.wait().await.map_err(io_error) },
        )
        .await?;
    }
    Ok(())
}

async fn close_raw_browser_best_effort(
    cdp: &mut RawCdpClient,
    browser: &mut Browser,
) -> Result<(), CdpError> {
    if let Err(err) = with_timeout("Browser.close", BROWSER_CLOSE_TIMEOUT, async {
        cdp.execute(None, BrowserCloseParams::default())
            .await
            .map(|_: chromiumoxide::cdp::browser_protocol::browser::CloseReturns| ())
    })
    .await
    {
        tracing::debug!(error = %err, "failed to close Chromium over raw CDP");
    }

    if let Err(wait_err) = with_timeout("Chromium process wait", BROWSER_WAIT_TIMEOUT, async {
        browser.wait().await.map_err(io_error)
    })
    .await
    {
        tracing::debug!(error = %wait_err, "failed to wait for Chromium process");
        kill_browser(browser).await?;
        with_timeout(
            "Chromium process wait after kill",
            BROWSER_WAIT_TIMEOUT,
            async { browser.wait().await.map_err(io_error) },
        )
        .await?;
    }
    Ok(())
}

async fn kill_browser(browser: &mut Browser) -> Result<(), CdpError> {
    if let Some(result) = tokio::time::timeout(BROWSER_KILL_TIMEOUT, browser.kill())
        .await
        .map_err(|_| timeout_error("Chromium kill", BROWSER_KILL_TIMEOUT))?
    {
        result.map_err(io_error)?;
    }
    Ok(())
}

async fn validate_browser_version(browser: &Browser) -> Result<(), CdpError> {
    let version = with_timeout("Browser.version", CDP_CONTROL_TIMEOUT, async {
        browser.version().await.map_err(driver_error)
    })
    .await?;
    validate_chromium_product_major(&version.product)
}

async fn validate_browser_version_raw(cdp: &mut RawCdpClient) -> Result<(), CdpError> {
    let version = with_timeout("Browser.version", CDP_CONTROL_TIMEOUT, async {
        cdp.execute(None, GetVersionParams::default()).await
    })
    .await?;
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

fn serde_driver_error(err: serde_json::Error) -> CdpError {
    driver_error(chromiumoxide::error::CdpError::Serde(err))
}

fn driver_message(message: impl Into<String>) -> CdpError {
    CdpError::Driver(Box::new(io::Error::other(message.into())))
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

    let text_boxes = extract_text_boxes(document, &layout_view, &nodes_view, &node_to_dom_order);

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

/// Walk up the CDP parent chain from `node_index` until reaching a node
/// that maps to an element `dom_order`. Returns that `dom_order`, or
/// `None` when no ancestor is a kept element.
///
/// CDP attributes inline text layout boxes to `#text` nodes (nodeType 3),
/// which are not elements and so carry no `dom_order`. Re-attributing a
/// box to the nearest ancestor element keeps `text_boxes_for` non-empty
/// for the painting element (`<p>`, `<span>`, …). When `node_index`
/// already maps to an element, its own `dom_order` is returned
/// immediately — preserving the prior behavior for element-owned boxes.
fn nearest_element_dom_order(
    nodes_view: &NodesView<'_>,
    node_to_dom_order: &[Option<u64>],
    node_index: usize,
) -> Option<u64> {
    let mut cursor = Some(node_index);
    // Bound the walk by the node count: a tree of N nodes has no path
    // longer than N, so this terminates even on a malformed cyclic
    // `parentIndex` chain.
    for _ in 0..=node_to_dom_order.len() {
        let idx = cursor?;
        if let Some(order) = node_to_dom_order.get(idx).copied().flatten() {
            return Some(order);
        }
        cursor = nodes_view
            .parent_index(idx)
            .and_then(|parent| usize::try_from(parent).ok());
    }
    None
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
/// CDP attributes each inline text box to the `#text` layout node that
/// owns it. `#text` nodes (nodeType 3) are not elements, so they carry no
/// `dom_order` of their own. Rather than dropping every real text run,
/// each box is re-attributed to the `dom_order` of its nearest ancestor
/// element via [`nearest_element_dom_order`]. A box is only skipped when
/// its layout index is out of range or no ancestor element has a
/// `dom_order`. Returns sorted by `(dom_order, start)`.
fn extract_text_boxes(
    document: &DocumentSnapshot,
    layout_view: &LayoutView<'_>,
    nodes_view: &NodesView<'_>,
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
        // The layout node owning this box is usually a `#text` node with
        // no `dom_order`. Re-attribute to the nearest ancestor element so
        // the painting element (`<p>`, `<span>`, …) carries its text run;
        // only drop the box when no ancestor element has a `dom_order`.
        let Some(dom_order) =
            nearest_element_dom_order(nodes_view, node_to_dom_order, cdp_node_idx_usize)
        else {
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
    use std::io;
    use std::path::PathBuf;

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
    fn storage_state_parse_str_rejects_crlf_in_cookie_domain() {
        // `parse_str` is the canonical validation entry point —
        // `load_from_path` delegates to it. Drive it directly so the
        // test doesn't need disk I/O or a CWD swap.
        let json = "{\"cookies\":[{\"name\":\"a\",\"value\":\"1\",\
            \"domain\":\"evil\\r\\nSet-Cookie: x=y\",\"path\":\"/\",\
            \"expires\":-1,\"httpOnly\":false,\"secure\":false,\"sameSite\":\"Lax\"}],\
            \"origins\":[]}";
        let err = StorageState::parse_str(json).unwrap_err();
        match err {
            CdpError::InvalidCookie { field, reason, .. } => {
                assert_eq!(field, "domain");
                assert!(reason.contains("control characters"));
            }
            other => panic!("expected InvalidCookie domain rejection, got {other:?}"),
        }
    }

    #[test]
    fn storage_state_parse_str_rejects_crlf_in_cookie_path() {
        let json = "{\"cookies\":[{\"name\":\"a\",\"value\":\"1\",\
            \"domain\":\"example.com\",\"path\":\"/foo\\nbar\",\
            \"expires\":-1,\"httpOnly\":false,\"secure\":false,\"sameSite\":\"Lax\"}],\
            \"origins\":[]}";
        let err = StorageState::parse_str(json).unwrap_err();
        match err {
            CdpError::InvalidCookie { field, reason, .. } => {
                assert_eq!(field, "path");
                assert!(reason.contains("control characters"));
            }
            other => panic!("expected InvalidCookie path rejection, got {other:?}"),
        }
    }

    #[test]
    fn storage_state_parse_str_rejects_full_c0_range_in_cookie_value() {
        // M1 + M3: the parser rejects every C0 byte (and DEL) in
        // cookie value, not only CR/LF/NUL.
        let json = "{\"cookies\":[{\"name\":\"a\",\"value\":\"v\\u001bx\",\
            \"domain\":\"example.com\",\"path\":\"/\",\
            \"expires\":-1,\"httpOnly\":false,\"secure\":false,\"sameSite\":\"Lax\"}],\
            \"origins\":[]}";
        let err = StorageState::parse_str(json).unwrap_err();
        assert!(matches!(
            err,
            CdpError::InvalidCookie { field: "value", .. }
        ));
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
    fn deterministic_style_source_uses_default_capture_knobs() {
        let Some(source) = super::deterministic_style_source(&super::Target::default()) else {
            panic!("default target should inject deterministic CSS");
        };

        assert!(source.contains("data-plumb-deterministic-style"));
        assert!(source.contains("animation-duration"));
        assert!(source.contains("transition-duration"));
        assert!(source.contains("overflow: hidden"));
        assert!(source.contains("::-webkit-scrollbar"));
    }

    #[test]
    fn deterministic_style_source_skips_when_knobs_disabled() {
        let target = super::Target {
            disable_animations: false,
            hide_scrollbars: false,
            ..super::Target::default()
        };

        assert!(super::deterministic_style_source(&target).is_none());
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
    fn viewport_override_skips_first_unpinned_target() {
        let target = super::Target::default();

        assert!(!super::should_apply_viewport_override(0, &target));
    }

    #[test]
    fn viewport_override_applies_to_first_pinned_target() {
        let target = super::Target {
            pin_dpr: Some(2.0),
            ..super::Target::default()
        };

        assert!(super::should_apply_viewport_override(0, &target));
    }

    #[test]
    fn viewport_override_applies_to_later_targets() {
        let target = super::Target::default();

        assert!(super::should_apply_viewport_override(1, &target));
    }

    #[test]
    fn initial_page_url_uses_blank_bootstrap_document() {
        assert_eq!(super::INITIAL_PAGE_URL, "about:blank");
    }

    #[test]
    fn add_script_params_registers_for_future_documents_only() {
        let params = super::add_script_to_evaluate_params("window.__plumb = true;");

        assert_eq!(params.source, "window.__plumb = true;");
        assert!(params.world_name.is_none());
        assert!(params.include_command_line_api.is_none());
        assert!(params.run_immediately.is_none());
    }

    #[test]
    fn browser_config_creates_isolated_profile_by_default() {
        let driver = super::ChromiumDriver::new(super::ChromiumOptions {
            executable_path: Some(test_executable_path()),
            ..super::ChromiumOptions::default()
        });
        let launch = match driver.browser_config(&super::Target::default(), None) {
            Ok(launch) => launch,
            Err(err) => panic!("browser config failed: {err}"),
        };

        assert!(launch.profile_dir.is_some());
        let Some(configured) = launch.config.user_data_dir.as_deref() else {
            panic!("expected generated user data dir");
        };
        assert!(configured.exists());
    }

    #[test]
    fn browser_config_preserves_explicit_profile() {
        let profile = match tempfile::tempdir() {
            Ok(profile) => profile,
            Err(err) => panic!("tempdir failed: {err}"),
        };
        let driver = super::ChromiumDriver::new(super::ChromiumOptions {
            executable_path: Some(test_executable_path()),
            user_data_dir: Some(profile.path().to_path_buf()),
            ..super::ChromiumOptions::default()
        });
        let launch = match driver.browser_config(&super::Target::default(), None) {
            Ok(launch) => launch,
            Err(err) => panic!("browser config failed: {err}"),
        };

        assert!(launch.profile_dir.is_none());
        assert_eq!(launch.config.user_data_dir.as_deref(), Some(profile.path()));
    }

    #[test]
    fn navigation_assignment_script_json_escapes_url() {
        let script = match super::navigation_assignment_script("https://example.com/a\"b\nc") {
            Ok(script) => script,
            Err(err) => panic!("script generation failed: {err}"),
        };
        assert_eq!(
            script,
            "window.location.assign(\"https://example.com/a\\\"b\\nc\");"
        );
    }

    #[test]
    fn file_urls_keep_chromiumoxide_goto_path() {
        assert!(super::uses_chromiumoxide_goto("file:///tmp/static.html"));
        assert_eq!(
            super::navigation_method_for_url("file:///tmp/static.html"),
            super::NavigationMethod::ChromiumoxideGoto
        );
    }

    #[test]
    fn navigation_method_avoids_script_assignment_for_data_urls() {
        assert_eq!(
            super::navigation_method_for_url("data:text/html;base64,PHNjcmlwdD4="),
            super::NavigationMethod::CdpNavigate
        );
        assert_eq!(
            super::navigation_method_for_url("http://127.0.0.1:49197/"),
            super::NavigationMethod::LocationAssign
        );
        assert_eq!(
            super::navigation_method_for_url("https://example.com/"),
            super::NavigationMethod::LocationAssign
        );
    }

    #[test]
    fn raw_navigation_submits_page_navigate_for_web_urls() {
        assert!(super::uses_raw_async_page_navigate(
            "http://127.0.0.1:49197/"
        ));
        assert!(super::uses_raw_async_page_navigate("https://example.com/"));
        assert!(!super::uses_raw_async_page_navigate(
            "data:text/html;base64,PHNjcmlwdD4="
        ));
        assert!(!super::uses_raw_async_page_navigate(
            "file:///tmp/static.html"
        ));
    }

    #[test]
    fn document_load_wait_accepts_redirected_complete_document_only() {
        assert!(super::document_is_loaded(&super::NavigationState {
            href: "https://example.com/login".to_string(),
            ready_state: "complete".to_string(),
            is_chrome_error_page: false,
        }));
        assert!(!super::document_is_loaded(&super::NavigationState {
            href: "https://example.com/login".to_string(),
            ready_state: "interactive".to_string(),
            is_chrome_error_page: false,
        }));
        assert!(!super::document_is_loaded(&super::NavigationState {
            href: super::INITIAL_PAGE_URL.to_string(),
            ready_state: "complete".to_string(),
            is_chrome_error_page: false,
        }));
        assert!(!super::document_is_loaded(&super::NavigationState {
            href: "chrome-error://chromewebdata/".to_string(),
            ready_state: "complete".to_string(),
            is_chrome_error_page: true,
        }));
    }

    #[test]
    fn selector_gated_raw_navigation_accepts_interactive_document() {
        let state = super::NavigationState {
            href: "https://example.com/app".to_string(),
            ready_state: "interactive".to_string(),
            is_chrome_error_page: false,
        };

        assert!(super::document_is_ready_for_capture(&state, true));
        assert!(!super::document_is_ready_for_capture(&state, false));
    }

    #[test]
    fn selector_gated_raw_navigation_still_rejects_initial_documents() {
        assert!(!super::document_is_ready_for_capture(
            &super::NavigationState {
                href: super::INITIAL_PAGE_URL.to_string(),
                ready_state: "interactive".to_string(),
                is_chrome_error_page: false,
            },
            true,
        ));
        assert!(!super::document_is_ready_for_capture(
            &super::NavigationState {
                href: "chrome-error://chromewebdata/".to_string(),
                ready_state: "interactive".to_string(),
                is_chrome_error_page: true,
            },
            true,
        ));
    }

    #[test]
    fn raw_navigation_events_require_navigated_main_frame() {
        let mut events = super::RawNavigationEvents::default();
        events.observe_load_event();

        assert!(!events.is_ready_for_capture(false));

        events.observe_main_frame_url(super::INITIAL_PAGE_URL);
        events.observe_load_event();

        assert!(!events.is_ready_for_capture(false));

        events.observe_main_frame_url("https://example.com/app");

        assert!(events.has_navigated());
        assert!(!events.is_ready_for_capture(false));

        events.observe_load_event();

        assert!(events.is_ready_for_capture(false));
    }

    #[test]
    fn selector_gated_raw_events_accept_dom_content_after_navigation() {
        let mut events = super::RawNavigationEvents::default();
        events.observe_main_frame_url("https://example.com/app");
        events.observe_dom_content_event();

        assert!(events.is_ready_for_capture(true));
        assert!(!events.is_ready_for_capture(false));
    }

    #[test]
    fn parse_navigation_state_reads_href_and_ready_state() {
        let state = match super::parse_navigation_state(
            r#"{"href":"http://127.0.0.1:49197/","readyState":"complete"}"#,
        ) {
            Ok(state) => state,
            Err(err) => panic!("navigation state parse failed: {err}"),
        };
        assert_eq!(state.href, "http://127.0.0.1:49197/");
        assert_eq!(state.ready_state, "complete");
        assert!(!state.is_chrome_error_page);
    }

    #[test]
    fn parse_navigation_state_reads_chrome_error_page_marker() {
        let state = match super::parse_navigation_state(
            r#"{"href":"chrome-error://chromewebdata/","readyState":"complete","isChromeErrorPage":true}"#,
        ) {
            Ok(state) => state,
            Err(err) => panic!("navigation state parse failed: {err}"),
        };
        assert!(state.is_chrome_error_page);
    }

    #[test]
    fn parse_navigation_state_rejects_malformed_json() {
        let err = super::parse_navigation_state("not json");
        assert!(matches!(err, Err(CdpError::Driver(_))));
    }

    #[test]
    fn navigation_ready_timeout_reason_preserves_stage_errors() {
        let reason = super::navigation_ready_timeout_reason(
            "http://127.0.0.1:49197/",
            Some("navigation location assignment exceeded 2s budget"),
            Some("navigation state read exceeded 2s budget"),
        );

        assert!(reason.contains("exhausted 30s ready-state budget"));
        assert!(reason.contains("after initial location assignment failed"));
        assert!(reason.contains("last navigation state read failed"));
    }

    #[test]
    fn navigation_display_url_redacts_data_urls() {
        assert_eq!(
            super::navigation_display_url("data:text/html;base64,PHNjcmlwdD4="),
            "data:<redacted>"
        );
        assert_eq!(
            super::navigation_display_url("http://127.0.0.1:49197/"),
            "http://127.0.0.1:49197/"
        );
    }

    #[test]
    fn contextualize_request_timeout_labels_operation() {
        let err = super::contextualize_request_timeout(
            "Target.attachToTarget",
            CdpError::Driver(Box::new(chromiumoxide::error::CdpError::Timeout)),
        );

        let message = err.to_string();
        assert!(message.contains("Target.attachToTarget"));
        assert!(message.contains("Chromiumoxide request budget"));
    }

    #[test]
    fn target_lifecycle_error_labels_pre_navigation_stage() {
        let err = super::target_lifecycle_error(
            "Target.createTarget",
            &CdpError::Driver(Box::new(io::Error::new(
                io::ErrorKind::TimedOut,
                "Target.createTarget exceeded 10s budget",
            ))),
        );

        let message = err.to_string();
        assert!(message.contains("Target.createTarget failed before navigation"));
        assert!(message.contains("Target.createTarget exceeded 10s budget"));
        assert!(super::is_retryable_capture_timeout(&err));
    }

    #[test]
    fn target_lifecycle_error_keeps_non_timeout_errors_non_retryable() {
        let err = super::target_lifecycle_error(
            "Target.attachToTarget",
            &CdpError::Driver(Box::new(io::Error::other("target disappeared"))),
        );

        let message = err.to_string();
        assert!(message.contains("Target.attachToTarget failed before navigation"));
        assert!(message.contains("target disappeared"));
        assert!(!super::is_retryable_capture_timeout(&err));
    }

    #[test]
    fn retryable_capture_timeout_accepts_chromiumoxide_timeout() {
        let err = CdpError::Driver(Box::new(chromiumoxide::error::CdpError::Timeout));

        assert!(super::is_retryable_capture_timeout(&err));
    }

    #[test]
    fn retryable_capture_timeout_accepts_plumb_timed_out_io() {
        let err = CdpError::Driver(Box::new(io::Error::new(
            io::ErrorKind::TimedOut,
            "Emulation.setDeviceMetricsOverride exceeded 25s budget",
        )));

        assert!(super::is_retryable_capture_timeout(&err));
    }

    #[test]
    fn retryable_capture_timeout_accepts_startup_navigation_abort() {
        let err = CdpError::Driver(Box::new(io::Error::other(
            "navigation to `http://127.0.0.1:49216/` exhausted 30s ready-state budget \
             after initial location assignment failed: driver failure: Page.navigate failed: \
             net::ERR_ABORTED; last navigation state read failed: navigation state read \
             exceeded 2s budget",
        )));

        assert!(super::is_retryable_capture_timeout(&err));
    }

    #[test]
    fn retryable_capture_timeout_rejects_bare_navigation_abort() {
        let err = CdpError::Driver(Box::new(io::Error::other(
            "Page.navigate failed: net::ERR_ABORTED",
        )));

        assert!(!super::is_retryable_capture_timeout(&err));
    }

    #[test]
    fn retryable_capture_timeout_rejects_non_timeout_errors() {
        let err = CdpError::MalformedSnapshot {
            reason: "missing document".to_owned(),
        };

        assert!(!super::is_retryable_capture_timeout(&err));
    }

    fn test_executable_path() -> PathBuf {
        match std::env::current_exe() {
            Ok(path) => path,
            Err(err) => panic!("current executable path unavailable: {err}"),
        }
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

    /// A synthetic `DOMSnapshot.captureSnapshot` response matching the
    /// CDP wire format. The DOM is:
    ///
    /// ```text
    /// html > body > p > #text("Hello")
    ///             > div > span
    /// ```
    ///
    /// CDP owns the inline text box on the `#text` node (node index 3),
    /// not on the `<p>`. The container `<div>` has only an element child,
    /// so no text box references it.
    fn capture_returns_with_text_box() -> super::CaptureSnapshotReturns {
        let value = serde_json::json!({
            "documents": [{
                "documentURL": 0, "title": 0, "baseURL": 0,
                "contentLanguage": 0, "encodingName": 0, "publicId": 0,
                "systemId": 0, "frameId": 0,
                "nodes": {
                    // index:        0   1   2   3   4   5
                    //             html body p  #txt div span
                    "parentIndex": [-1,  0,  1,  2,  1,  4],
                    "nodeType":    [ 1,  1,  1,  3,  1,  1],
                    "nodeName":    [ 1,  2,  3,  4,  5,  6]
                },
                "layout": {
                    // layout slot:  0(p) 1(#text) 2(div) 3(span)
                    "nodeIndex": [2, 3, 4, 5],
                    "styles": [],
                    "bounds": [
                        [10.0, 10.0, 100.0, 20.0],
                        [10.0, 12.0,  40.0, 16.0],
                        [10.0, 40.0, 200.0, 50.0],
                        [10.0, 40.0,   0.0,  0.0]
                    ],
                    "text": [],
                    "stackingContexts": { "index": [] }
                },
                "textBoxes": {
                    // Owned by layout slot 1 — the `#text` node.
                    "layoutIndex": [1],
                    "bounds": [[10.0, 12.0, 40.0, 16.0]],
                    "start": [0],
                    "length": [5]
                }
            }],
            "strings": ["", "HTML", "BODY", "P", "#text", "DIV", "SPAN"]
        });
        serde_json::from_value(value).expect("synthetic CDP response must deserialize")
    }

    #[test]
    fn text_box_reattributed_to_nearest_ancestor_element() {
        let target = super::Target {
            url: "https://example.com/".to_string(),
            ..super::Target::default()
        };
        let snap = super::flatten_snapshot(&target, &capture_returns_with_text_box())
            .expect("flatten must succeed for the synthetic response");

        // `<p>` is the third element in source order → dom_order 2.
        let p = snap
            .nodes
            .iter()
            .find(|n| n.tag == "p")
            .expect("`<p>` element must survive flattening");
        let div = snap
            .nodes
            .iter()
            .find(|n| n.tag == "div")
            .expect("`<div>` element must survive flattening");

        // Exactly one text box, attributed to the `<p>` (the nearest
        // ancestor element of the `#text` layout node), not dropped.
        assert_eq!(snap.text_boxes.len(), 1, "the single text run survives");
        let tb = &snap.text_boxes[0];
        assert_eq!(
            tb.dom_order, p.dom_order,
            "text box must attribute to the `<p>`, not the `#text` node"
        );
        assert_eq!(tb.length, 5, "\"Hello\" is 5 UTF-16 units");
        assert_eq!(tb.start, 0);

        // The container `<div>` has only an element child, so no text box
        // references it.
        assert!(
            snap.text_boxes.iter().all(|b| b.dom_order != div.dom_order),
            "container `<div>` with only element children must carry no text box"
        );
    }

    #[test]
    fn text_box_reattribution_is_byte_deterministic() {
        let target = super::Target::default();
        let a =
            super::flatten_snapshot(&target, &capture_returns_with_text_box()).expect("flatten a");
        let b =
            super::flatten_snapshot(&target, &capture_returns_with_text_box()).expect("flatten b");
        assert_eq!(
            serde_json::to_string(&a).expect("serialize a"),
            serde_json::to_string(&b).expect("serialize b"),
            "two flattens of identical input must match byte-for-byte"
        );
    }
}
