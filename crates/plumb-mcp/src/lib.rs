//! # plumb-mcp
//!
//! Model Context Protocol server for Plumb, backed by the official
//! [`rmcp`] Rust SDK.
//!
//! The server exposes these tools to AI coding agents:
//!
//! - `echo` — smoke-tests the transport.
//! - `lint_url` — lints a URL and returns violations in the MCP-compact
//!   shape from `docs/local/prd.md` §14.2. Accepts `http(s)://` URLs
//!   (driven by `plumb_cdp::ChromiumDriver`) and `plumb-fake://` URLs
//!   (served from the canned snapshot). Resolves the project's
//!   `plumb.toml` from `working_dir` (or the server CWD) so the lint
//!   honors the caller's design tokens. Output is aggregated and capped
//!   at 10 KB of `structuredContent`.
//! - `lint_page_html` — lints a self-contained HTML string by rendering
//!   it through the same persistent Chromium as `lint_url` (via a
//!   `data:` URL) so embedded `<style>`/inline styles produce real
//!   computed styles. External stylesheets and resources are not
//!   fetched. Same aggregated, capped response shape as `lint_url`.
//!   Capped at 1 MiB of input and 10 000 elements.
//! - `explain_rule` — returns the canonical markdown documentation and
//!   metadata for a built-in rule by id.
//! - `list_rules` — enumerates every built-in rule with id, default
//!   severity, and one-line summary.
//! - `get_config` — resolves `plumb.toml` for a given working directory
//!   and returns the [`Config`] as JSON. Memoized per `(path, mtime)`.
//! - `compare_viewports` — captures snapshots at two-or-more viewports
//!   and returns a deterministic diff (missing nodes, size changes,
//!   reorderings, computed-style changes). 10 KB structuredContent
//!   budget; aggregate counts plus a capped diff list.
//!
//! The [`PlumbServer`] type implements [`rmcp::ServerHandler`] directly.
//! Extend it by adding a tool descriptor to `list_tools` and a matching
//! branch in `call_tool`; see `.agents/rules/mcp-tool-patterns.md`.
//!
//! [`rmcp`]: https://crates.io/crates/rmcp

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod compare_viewports;
mod explain;

#[doc(hidden)]
pub use explain::rule_ids as documented_rule_ids;

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use plumb_cdp::{ChromiumOptions, PersistentBrowser, Target, is_fake_url};
use plumb_config::ConfigError;
use plumb_core::{Config, PlumbSnapshot, ViewportKey, Violation, register_builtin, run};
use plumb_format::{json as full_json, mcp_compact};
use rmcp::{
    RoleServer, ServerHandler, ServiceExt,
    handler::server::tool::schema_for_type,
    model::AnnotateAble,
    model::{
        CallToolRequestParams, CallToolResult, Content, ErrorData, Implementation, JsonObject,
        ListResourcesResult, ListToolsResult, PaginatedRequestParams, ProtocolVersion, RawResource,
        ReadResourceRequestParams, ReadResourceResult, ResourceContents, ServerCapabilities,
        ServerInfo, Tool,
    },
    schemars::{self, JsonSchema},
    service::RequestContext,
    transport::{
        StreamableHttpServerConfig, StreamableHttpService, stdio,
        streamable_http_server::session::local::LocalSessionManager,
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use subtle::ConstantTimeEq;
use thiserror::Error;
use tokio::sync::OnceCell;

/// MCP server errors.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum McpError {
    /// Underlying I/O error on stdin/stdout.
    #[error("mcp i/o: {0}")]
    Io(#[from] io::Error),
    /// rmcp service or transport failure.
    #[error("mcp service: {0}")]
    Service(String),
}

#[derive(Clone)]
struct HttpAuthState {
    token: HttpAuthToken,
}

#[derive(Clone)]
struct HttpAuthToken(Arc<str>);

impl HttpAuthToken {
    fn new(token: String) -> Self {
        Self(Arc::<str>::from(token))
    }

    fn matches_authorization_header(&self, value: &str) -> bool {
        value
            .strip_prefix("Bearer ")
            .is_some_and(|candidate| secure_token_eq(self.0.as_bytes(), candidate.as_bytes()))
    }
}

/// Arguments to the `echo` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EchoArgs {
    /// Message to echo back.
    pub message: String,
}

/// Arguments to the `lint_url` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LintUrlArgs {
    /// URL to lint. Accepts `http(s)://` and `plumb-fake://` URLs.
    pub url: String,
    /// Response detail level. Defaults to the compact MCP payload.
    #[serde(default)]
    pub detail: LintUrlDetail,
    /// Optional working directory whose `plumb.toml` configures the
    /// lint. When omitted, the server resolves `plumb.toml` from its
    /// own current working directory; when neither exists, the built-in
    /// `Config::default()` is used. Resolution shares the `get_config`
    /// cache.
    #[serde(default)]
    pub working_dir: Option<String>,
}

/// Structured response detail level for `lint_url`.
#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LintUrlDetail {
    /// Current token-efficient MCP payload.
    #[default]
    Compact,
    /// Canonical full JSON envelope with complete per-violation fields.
    Full,
}

/// Arguments to the `lint_page_html` tool.
///
/// `lint_page_html` lints a self-contained HTML string by rendering it
/// through the same persistent Chromium as [`LintUrlArgs`]: the document
/// is loaded as a `data:` URL, so embedded `<style>` blocks and inline
/// `style="…"` attributes produce real computed styles and geometry.
/// External stylesheets and resources are **not** fetched (a relative
/// `<link>` or `<img>` will not load) — for a full page use `lint_url`.
/// The response uses the same aggregated, capped MCP-compact shape.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LintPageHtmlArgs {
    /// Raw HTML source for the page. Hard-capped at 1 MiB and 10 000
    /// elements; oversized inputs return a JSON-RPC error before any
    /// rendering happens.
    pub html: String,
    /// Base URL recorded as the snapshot's `url`. The function does not
    /// fetch this URL or validate it — callers pass whatever URL the
    /// downstream report should attribute the document to.
    pub base_url: String,
    /// Optional working directory whose `plumb.toml` configures the
    /// lint. Resolved exactly like [`LintUrlArgs::working_dir`].
    #[serde(default)]
    pub working_dir: Option<String>,
}

/// Arguments to the `explain_rule` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExplainRuleArgs {
    /// Stable rule id, `<category>/<id>` (e.g. `spacing/scale-conformance`).
    pub rule_id: String,
}

/// Arguments to the `list_rules` tool. Currently empty — agents call
/// it without parameters.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ListRulesArgs {}

/// Arguments to the `get_config` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetConfigArgs {
    /// Absolute path to the working directory containing `plumb.toml`.
    /// The tool resolves `<working_dir>/plumb.toml` deterministically;
    /// it never reads the process current directory.
    pub working_dir: String,
}

/// One viewport definition for the `compare_viewports` tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CompareViewport {
    /// Stable name (echoed back in the response under `viewports`).
    pub name: String,
    /// Viewport width in CSS pixels.
    pub width: u32,
    /// Viewport height in CSS pixels.
    pub height: u32,
    /// Device pixel ratio (e.g. 1.0 for desktop, 2.0 for retina).
    pub dpr: f32,
}

/// Arguments to the `compare_viewports` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompareViewportsArgs {
    /// URL to capture. Accepts `http(s)://` and `plumb-fake://` URLs.
    pub url: String,
    /// Two-or-more viewports to capture. The first viewport acts as
    /// the diff baseline; later viewports are compared against it.
    pub viewports: Vec<CompareViewport>,
    /// Pixel threshold above which width/height differences are
    /// reported. Defaults to 4 px when omitted.
    #[serde(default)]
    pub size_threshold_px: Option<u32>,
}

/// The Plumb MCP server.
///
/// Cheap to construct; `Clone` shares the memo cache and the persistent
/// browser handle. The browser is warmed lazily on the first non-fake
/// `lint_url` call and reused for the rest of the session — see
/// [`PersistentBrowser`] for the per-call incognito-context invariant.
#[derive(Clone)]
pub struct PlumbServer {
    cwd: PathBuf,
    config_cache: Arc<Mutex<HashMap<PathBuf, ConfigCacheEntry>>>,
    browser: Arc<OnceCell<PersistentBrowser>>,
}

const CONFIG_RESOURCE_URI: &str = "plumb://config";
const CONFIG_RESOURCE_NAME: &str = "resolved_config";
const CONFIG_RESOURCE_DESCRIPTION: &str =
    "Resolved plumb.toml for the server working directory as JSON.";
const CONFIG_RESOURCE_MIME_TYPE: &str = "application/json";

#[derive(Clone)]
struct ConfigCacheEntry {
    mtime: SystemTime,
    value: serde_json::Value,
}

struct ResolvedConfig {
    value: serde_json::Value,
    summary: String,
}

impl PlumbServer {
    /// Construct a new server.
    #[must_use]
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            config_cache: Arc::default(),
            browser: Arc::default(),
        }
    }

    /// Smoke-test the MCP transport by echoing a message back to the
    /// caller.
    ///
    /// Returns a [`CallToolResult`] whose `content[0]` is the text
    /// `message` (so chatty agents can surface it directly) and whose
    /// `structured_content` is a `{ "echoed": <message> }` JSON object
    /// (so tool-using agents can parse it without re-parsing the
    /// text). Both fields are required by `mcp-tool-patterns.md`.
    ///
    /// # Errors
    ///
    /// Currently never returns an error — `echo` is a smoke test, not
    /// a validated input gate. The signature retains `Result` for
    /// forward-compatibility with future input checks (e.g. byte caps).
    pub async fn echo(&self, args: EchoArgs) -> Result<CallToolResult, ErrorData> {
        // `mcp-tool-patterns.md`: every tool MUST return both `content`
        // (human-friendly summary) AND `structuredContent` (machine
        // payload). Without the structured block, tool-using agents
        // are forced to re-parse the text — which is exactly what the
        // contract is designed to avoid for transport smoke tests.
        let mut structured = JsonObject::new();
        structured.insert(
            "echoed".to_owned(),
            serde_json::Value::String(args.message.clone()),
        );
        let mut result = CallToolResult::success(vec![Content::text(args.message)]);
        result.structured_content = Some(serde_json::Value::Object(structured));
        Ok(result)
    }

    /// Lint a URL and return aggregated violations.
    ///
    /// `plumb-fake://` URLs are served from [`PlumbSnapshot::canned`]
    /// without ever spinning up a browser. The first non-fake call
    /// warms a single persistent Chromium process via
    /// [`PersistentBrowser::launch`]; subsequent calls reuse the same
    /// process and run inside fresh incognito contexts so that state
    /// from one URL never leaks into another.
    ///
    /// # Errors
    ///
    /// Returns a JSON-RPC error (`ErrorData`) when the resolved
    /// `plumb.toml` cannot be loaded or parsed. Driver failures
    /// (Chromium not found, version out of range, CDP error, malformed
    /// snapshot) are returned as a successful JSON-RPC response with
    /// `isError = true` and a single text content block — never as a
    /// JSON-RPC error.
    pub async fn lint_url(&self, args: LintUrlArgs) -> Result<CallToolResult, ErrorData> {
        let config = self.resolve_config_object(args.working_dir.as_deref())?;

        let snapshot = if is_fake_url(&args.url) {
            PlumbSnapshot::canned()
        } else {
            let target = Target {
                url: args.url.clone(),
                viewport: ViewportKey::new("desktop"),
                width: 1280,
                height: 800,
                device_pixel_ratio: 1.0,
                ..Target::default()
            };
            match self.snapshot_via_browser(target, "lint_url").await {
                Ok(snap) => snap,
                Err(result) => return Ok(result),
            }
        };
        let violations = run(&snapshot, &config);
        build_lint_url_result(&violations, args.detail)
    }

    /// Lint a self-contained HTML string by rendering it in Chromium.
    ///
    /// The HTML is loaded as a `data:text/html;base64,…` URL through the
    /// same persistent browser that powers [`Self::lint_url`], so
    /// embedded `<style>` blocks and inline `style="…"` attributes
    /// produce real computed styles and geometry — the spacing, color,
    /// typography, contrast, and touch-target rules all see what the
    /// browser actually rendered. External stylesheets and resources are
    /// not fetched; a relative `<link>` or `<img>` will not load. For a
    /// full page (with its real stylesheet cascade) use
    /// [`Self::lint_url`].
    ///
    /// The response uses the aggregated, capped MCP-compact shape
    /// ([`mcp_compact`]). Input is hard-capped at 1 MiB and 10 000
    /// opening tags, validated before the document is rendered.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorData::invalid_params`] (JSON-RPC `-32602`) when
    /// `args.html` is empty, when `args.base_url` is empty, or when the
    /// input exceeds the byte or element cap. Returns a JSON-RPC error
    /// when the resolved `plumb.toml` cannot be loaded. Driver failures
    /// (including Chromium being unavailable) surface as a successful
    /// response with `isError = true` and a single text content block,
    /// exactly as [`Self::lint_url`] does — never a misleading clean
    /// result.
    pub async fn lint_page_html(
        &self,
        args: LintPageHtmlArgs,
    ) -> Result<CallToolResult, ErrorData> {
        if args.html.is_empty() {
            return Err(ErrorData::invalid_params(
                "html must not be empty".to_string(),
                None,
            ));
        }
        if args.base_url.is_empty() {
            return Err(ErrorData::invalid_params(
                "base_url must not be empty".to_string(),
                None,
            ));
        }
        if args.html.len() > LINT_PAGE_HTML_INPUT_BYTE_CAP {
            return Err(ErrorData::invalid_params(
                format!(
                    "html is {} bytes, exceeds the {LINT_PAGE_HTML_INPUT_BYTE_CAP}-byte cap",
                    args.html.len()
                ),
                None,
            ));
        }
        let element_estimate = count_open_tags(&args.html);
        if element_estimate > LINT_PAGE_HTML_ELEMENT_CAP {
            return Err(ErrorData::invalid_params(
                format!(
                    "html has roughly {element_estimate} elements, exceeds the {LINT_PAGE_HTML_ELEMENT_CAP}-element cap"
                ),
                None,
            ));
        }

        let config = self.resolve_config_object(args.working_dir.as_deref())?;

        let data_url = format!(
            "data:text/html;base64,{}",
            base64_encode(args.html.as_bytes())
        );
        let target = Target {
            url: data_url,
            viewport: ViewportKey::new("desktop"),
            width: 1280,
            height: 800,
            device_pixel_ratio: 1.0,
            ..Target::default()
        };
        let mut snapshot = match self.snapshot_via_browser(target, "lint_page_html").await {
            Ok(snap) => snap,
            Err(result) => return Ok(result),
        };
        // Attribute the snapshot to the caller-supplied base URL rather
        // than the opaque (and huge) data: URL it was rendered from.
        snapshot.url.clone_from(&args.base_url);

        let violations = run(&snapshot, &config);
        build_lint_url_result(&violations, LintUrlDetail::Compact)
    }

    /// Warm the persistent browser (if needed) and capture a snapshot.
    ///
    /// On any driver failure — Chromium unavailable, version out of
    /// range, CDP error, malformed snapshot — returns `Err` carrying the
    /// `isError = true` [`CallToolResult`] the caller should hand back,
    /// so a missing browser never masquerades as a clean lint.
    async fn snapshot_via_browser(
        &self,
        target: Target,
        tool: &str,
    ) -> Result<PlumbSnapshot, CallToolResult> {
        match self
            .browser
            .get_or_try_init(|| PersistentBrowser::launch(ChromiumOptions::default()))
            .await
        {
            Ok(browser) => browser.snapshot(target).await.map_err(|err| {
                CallToolResult::error(vec![Content::text(format!("{tool} failed: {err}"))])
            }),
            Err(err) => Err(CallToolResult::error(vec![Content::text(format!(
                "{tool} failed: {err}"
            ))])),
        }
    }

    /// Resolve the [`Config`] the lint tools should run with.
    ///
    /// Resolution order: the explicit `working_dir` argument, else the
    /// server's own current working directory, else `Config::default()`
    /// when no `plumb.toml` is found. Delegates to
    /// [`Self::resolve_config_for_dir`] so the `(path, mtime)` cache and
    /// default fallback are shared with the `get_config` tool.
    ///
    /// # Errors
    ///
    /// Propagates the [`ErrorData`] from [`Self::resolve_config_for_dir`]
    /// when an existing `plumb.toml` cannot be read or parsed.
    fn resolve_config_object(&self, working_dir: Option<&str>) -> Result<Config, ErrorData> {
        let dir = match working_dir {
            Some(raw) if !raw.trim().is_empty() => PathBuf::from(raw),
            _ => self.cwd.clone(),
        };
        let resolved = self.resolve_config_for_dir(&dir)?;
        let config_value = resolved.value.get("config").cloned().ok_or_else(|| {
            ErrorData::internal_error("resolved config payload missing `config` field", None)
        })?;
        serde_json::from_value(config_value).map_err(|err| {
            ErrorData::internal_error(format!("deserialize resolved config: {err}"), None)
        })
    }

    /// Gracefully shut down the persistent browser, if any.
    ///
    /// Idempotent: when no browser was warmed (every call so far
    /// targeted `plumb-fake://`, or `shutdown` has already run), this
    /// is a no-op and returns `Ok(())`.
    ///
    /// # Errors
    ///
    /// Returns [`McpError::Service`] if the underlying
    /// [`PersistentBrowser::shutdown`] call surfaces an error.
    pub async fn shutdown(&self) -> Result<(), McpError> {
        if let Some(browser) = self.browser.get() {
            browser
                .shutdown()
                .await
                .map_err(|err| McpError::Service(err.to_string()))?;
        }
        Ok(())
    }

    /// Look up the canonical documentation for a built-in rule.
    ///
    /// Returns a [`CallToolResult`] whose first content block is the
    /// rule's markdown body and whose `structured_content` carries
    /// `{ rule_id, severity, summary, doc_url, markdown }`.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorData::invalid_params`] (JSON-RPC `-32602`) when
    /// `args.rule_id` does not name a built-in rule.
    pub async fn explain_rule(&self, args: ExplainRuleArgs) -> Result<CallToolResult, ErrorData> {
        let Some(entry) = explain::lookup(&args.rule_id) else {
            return Err(ErrorData::invalid_params(
                format!("unknown rule id: {}", args.rule_id),
                None,
            ));
        };

        // Source severity + summary from the Rule trait so metadata
        // never duplicates what `register_builtin` already exposes.
        let Some(rule) = register_builtin()
            .into_iter()
            .find(|rule| rule.id() == entry.rule_id)
        else {
            return Err(ErrorData::internal_error(
                format!(
                    "rule {} has a doc entry but is not registered in register_builtin()",
                    entry.rule_id
                ),
                None,
            ));
        };

        let severity = rule.default_severity().label();
        let summary = rule.summary();
        let doc_url = explain::doc_url(entry.rule_id);

        let structured = serde_json::json!({
            "rule_id": entry.rule_id,
            "severity": severity,
            "summary": summary,
            "doc_url": doc_url,
            "markdown": entry.markdown,
        });

        let mut result = CallToolResult::success(vec![Content::text(entry.markdown.to_owned())]);
        result.structured_content = Some(structured);
        Ok(result)
    }

    async fn list_rules(&self, _args: ListRulesArgs) -> Result<CallToolResult, ErrorData> {
        let (text, structured) = self.list_rules_payload();
        let mut result = CallToolResult::success(vec![Content::text(text)]);
        result.structured_content = Some(structured);
        Ok(result)
    }

    /// Build the `list_rules` response payload — `(human text, structured JSON)`.
    ///
    /// Output is a deterministic function of the built-in rule registry: rules
    /// are sorted by id (which encodes `<category>/<name>` and so sorts by
    /// category first), severity is the lowercase [`Severity::label`] string,
    /// and the structured block carries a `count` plus the `rules` array.
    ///
    /// Token budget: bounded by `register_builtin().len()` — one short line
    /// per rule, well under 10 KB at the current rule count and growth rate.
    ///
    /// Takes `&self` for ergonomic call-site symmetry with other tool methods,
    /// even though the response is purely a function of the built-in registry.
    ///
    /// [`Severity::label`]: plumb_core::report::Severity::label
    #[must_use]
    #[allow(clippy::unused_self)]
    pub fn list_rules_payload(&self) -> (String, Value) {
        let mut entries: Vec<(&'static str, &'static str, &'static str)> = register_builtin()
            .iter()
            .map(|rule| (rule.id(), rule.default_severity().label(), rule.summary()))
            .collect();
        entries.sort_unstable_by(|a, b| a.0.cmp(b.0));

        let mut text = String::new();
        for (id, severity, summary) in &entries {
            use std::fmt::Write as _;
            let _ = writeln!(text, "{severity} {id} — {summary}");
        }

        let rules: Vec<Value> = entries
            .iter()
            .map(|(id, severity, summary)| {
                json!({
                    "id": id,
                    "default_severity": severity,
                    "summary": summary,
                })
            })
            .collect();
        let structured = json!({
            "rules": rules,
            "count": entries.len(),
        });

        (text, structured)
    }

    /// Resolve `<working_dir>/plumb.toml` and return the resolved [`Config`]
    /// as JSON.
    ///
    /// The result is memoized per `(path, mtime)`. A subsequent call with
    /// the same `working_dir` returns the cached JSON until the underlying
    /// file is modified.
    ///
    /// When no `plumb.toml` exists at the requested path, the tool returns
    /// the result of [`Config::default`] with `source = "default"`. This
    /// keeps a fresh checkout usable without scaffolding a config file.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorData::invalid_params`] if `working_dir` is empty or
    /// not absolute, and [`ErrorData::internal_error`] when reading or
    /// parsing an existing `plumb.toml` fails.
    pub async fn get_config(&self, args: GetConfigArgs) -> Result<CallToolResult, ErrorData> {
        let resolved = self.resolve_config_for_tool(&args.working_dir)?;
        let mut result = CallToolResult::success(vec![Content::text(resolved.summary)]);
        result.structured_content = Some(resolved.value);
        Ok(result)
    }

    /// Capture snapshots at two-or-more viewports and return a deterministic
    /// diff: missing nodes, size changes that exceed `size_threshold_px`,
    /// document-order reorderings, and computed-style differences.
    ///
    /// `plumb-fake://` URLs are served from [`PlumbSnapshot::canned`] without
    /// warming Chromium. Real URLs share the persistent browser warmed by
    /// [`PlumbServer::lint_url`].
    ///
    /// The structured payload is bounded: the diff list is capped at 200
    /// entries, and aggregate counts are always reported in `summary`. A
    /// `truncated` flag signals when entries were clipped.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorData::invalid_params`] when fewer than two viewports
    /// are supplied, when viewport names are non-unique, or when any
    /// viewport has zero width or height. Driver failures surface as a
    /// successful response with `is_error = true` and a single text
    /// content block — never as a JSON-RPC error.
    pub async fn compare_viewports(
        &self,
        args: CompareViewportsArgs,
    ) -> Result<CallToolResult, ErrorData> {
        validate_compare_viewports_args(&args)?;

        let viewport_names: Vec<String> = args.viewports.iter().map(|v| v.name.clone()).collect();
        let snapshots = match self.capture_viewport_snapshots(&args).await {
            Ok(snaps) => snaps,
            Err(message) => {
                return Ok(CallToolResult::error(vec![Content::text(message)]));
            }
        };

        let threshold = args
            .size_threshold_px
            .unwrap_or(compare_viewports::DEFAULT_SIZE_THRESHOLD_PX);
        let result = compare_viewports::compare_snapshots(&snapshots, &viewport_names, threshold);

        Ok(build_compare_viewports_result(
            &args.url,
            &viewport_names,
            threshold,
            result,
        ))
    }

    async fn capture_viewport_snapshots(
        &self,
        args: &CompareViewportsArgs,
    ) -> Result<Vec<PlumbSnapshot>, String> {
        if is_fake_url(&args.url) {
            // Fake-URL fast path: reuse `PlumbSnapshot::canned` per viewport,
            // overriding the viewport key + dimensions so downstream diffing
            // sees distinct viewport metadata. Content stays identical so
            // the agent gets a clean "no diffs" result it can rely on as
            // a smoke test.
            let mut out = Vec::with_capacity(args.viewports.len());
            for v in &args.viewports {
                let mut snap = PlumbSnapshot::canned();
                snap.viewport = ViewportKey::new(v.name.clone());
                snap.viewport_width = v.width;
                snap.viewport_height = v.height;
                snap.url.clone_from(&args.url);
                out.push(snap);
            }
            return Ok(out);
        }

        let targets: Vec<Target> = args
            .viewports
            .iter()
            .map(|v| Target {
                url: args.url.clone(),
                viewport: ViewportKey::new(v.name.clone()),
                width: v.width,
                height: v.height,
                device_pixel_ratio: v.dpr,
                ..Target::default()
            })
            .collect();

        let browser = self
            .browser
            .get_or_try_init(|| PersistentBrowser::launch(ChromiumOptions::default()))
            .await
            .map_err(|err| format!("compare_viewports failed: {err}"))?;

        let mut snapshots = Vec::with_capacity(targets.len());
        for target in targets {
            let snap = browser
                .snapshot(target)
                .await
                .map_err(|err| format!("compare_viewports failed: {err}"))?;
            snapshots.push(snap);
        }
        Ok(snapshots)
    }

    fn resolve_config_for_tool(&self, working_dir: &str) -> Result<ResolvedConfig, ErrorData> {
        if working_dir.is_empty() {
            return Err(ErrorData::invalid_params(
                "working_dir must not be empty".to_string(),
                None,
            ));
        }
        let working_dir = PathBuf::from(working_dir);
        if !working_dir.is_absolute() {
            return Err(ErrorData::invalid_params(
                format!("working_dir must be absolute: {}", working_dir.display()),
                None,
            ));
        }

        self.resolve_config_for_dir(&working_dir)
    }

    fn resolve_config_for_dir(&self, working_dir: &Path) -> Result<ResolvedConfig, ErrorData> {
        let config_path = working_dir.join("plumb.toml");

        if config_path.exists() {
            let mtime = std::fs::metadata(&config_path)
                .and_then(|m| m.modified())
                .map_err(|err| {
                    ErrorData::internal_error(
                        format!("stat {}: {err}", config_path.display()),
                        None,
                    )
                })?;

            if let Some(entry) = self.cache_lookup(&config_path, mtime)? {
                let summary = format!("plumb.toml @ {} (cached)", config_path.display());
                Ok(ResolvedConfig {
                    value: entry,
                    summary,
                })
            } else {
                let config =
                    plumb_config::load(&config_path).map_err(|err| map_config_error(&err))?;
                let value = serialize_config(&config, &config_path, ConfigSource::File)?;
                self.cache_store(&config_path, mtime, value.clone())?;
                let summary = format!("plumb.toml @ {}", config_path.display());
                Ok(ResolvedConfig { value, summary })
            }
        } else {
            let value = serialize_config(&Config::default(), &config_path, ConfigSource::Default)?;
            let summary = format!(
                "no plumb.toml at {} — returning Config::default()",
                config_path.display()
            );
            Ok(ResolvedConfig { value, summary })
        }
    }

    fn cache_lookup(
        &self,
        path: &Path,
        mtime: SystemTime,
    ) -> Result<Option<serde_json::Value>, ErrorData> {
        let hit = self
            .config_cache
            .lock()
            .map_err(|err| {
                ErrorData::internal_error(format!("config cache poisoned: {err}"), None)
            })?
            .get(path)
            .and_then(|entry| (entry.mtime == mtime).then(|| entry.value.clone()));
        Ok(hit)
    }

    fn cache_store(
        &self,
        path: &Path,
        mtime: SystemTime,
        value: serde_json::Value,
    ) -> Result<(), ErrorData> {
        self.config_cache
            .lock()
            .map_err(|err| {
                ErrorData::internal_error(format!("config cache poisoned: {err}"), None)
            })?
            .insert(path.to_path_buf(), ConfigCacheEntry { mtime, value });
        Ok(())
    }
}

const LINT_URL_FULL_RESPONSE_CAP_BYTES: usize = 50 * 1024;

/// Hard byte cap on `lint_page_html` input (1 MiB). Validated before the
/// document is rendered so a pathological payload never reaches the
/// browser or inflates into an oversized `data:` URL.
const LINT_PAGE_HTML_INPUT_BYTE_CAP: usize = 1024 * 1024;

/// Hard cap on the number of opening tags `lint_page_html` will render
/// (10 000), estimated cheaply from the raw HTML before rendering.
const LINT_PAGE_HTML_ELEMENT_CAP: usize = 10_000;

/// Conservative pre-render estimate of an HTML document's element count:
/// the number of `<` bytes immediately followed by an ASCII letter (an
/// opening tag). Closing tags (`</`), comments (`<!--`), and the doctype
/// (`<!`) are excluded. This is a guardrail, not an exact parse — the
/// real element count after rendering can only be lower (the browser
/// drops malformed tags), so the estimate never under-counts a genuine
/// blow-up.
fn count_open_tags(html: &str) -> usize {
    html.as_bytes()
        .windows(2)
        .filter(|w| w[0] == b'<' && w[1].is_ascii_alphabetic())
        .count()
}

/// Standard (RFC 4648) base64 encoding for the
/// `data:text/html;base64,…` URL `lint_page_html` navigates. Kept local
/// to avoid pulling a base64 crate into this layer for ~20 lines of
/// pure, deterministic code.
fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied();
        let b2 = chunk.get(2).copied();
        let triple =
            (u32::from(b0) << 16) | (u32::from(b1.unwrap_or(0)) << 8) | u32::from(b2.unwrap_or(0));
        out.push(char::from(TABLE[((triple >> 18) & 0x3f) as usize]));
        out.push(char::from(TABLE[((triple >> 12) & 0x3f) as usize]));
        out.push(if b1.is_some() {
            char::from(TABLE[((triple >> 6) & 0x3f) as usize])
        } else {
            '='
        });
        out.push(if b2.is_some() {
            char::from(TABLE[(triple & 0x3f) as usize])
        } else {
            '='
        });
    }
    out
}

fn build_lint_url_result(
    violations: &[Violation],
    detail: LintUrlDetail,
) -> Result<CallToolResult, ErrorData> {
    let (text, compact_structured) = mcp_compact(violations);
    let structured = match detail {
        LintUrlDetail::Compact => compact_structured,
        LintUrlDetail::Full => build_full_lint_payload(violations)?,
    };

    let mut result = CallToolResult::success(vec![Content::text(text)]);
    result.structured_content = Some(structured);
    Ok(result)
}

fn build_full_lint_payload(violations: &[Violation]) -> Result<Value, ErrorData> {
    let payload = full_json(violations).map_err(|err| {
        ErrorData::internal_error(format!("serialize full lint payload: {err}"), None)
    })?;
    enforce_response_cap(
        payload.len(),
        LINT_URL_FULL_RESPONSE_CAP_BYTES,
        "lint_url detail=full payload exceeds 50 KB response cap",
    )?;
    let structured: Value = serde_json::from_str(&payload).map_err(|err| {
        ErrorData::internal_error(format!("parse full lint payload: {err}"), None)
    })?;
    Ok(structured)
}

fn enforce_response_cap(
    payload_len: usize,
    limit_bytes: usize,
    error_message: &'static str,
) -> Result<(), ErrorData> {
    if payload_len > limit_bytes {
        return Err(ErrorData::internal_error(
            format!("{error_message} ({payload_len} bytes)"),
            None,
        ));
    }
    Ok(())
}

/// 10 KB cap on the `compare_viewports` `structuredContent` payload —
/// matches the budget called out in PRD §14.2 for any tool that returns
/// per-node detail. Aggregation + diff-list capping keeps real-world
/// payloads well below this limit.
const COMPARE_VIEWPORTS_RESPONSE_CAP_BYTES: usize = 10 * 1024;

fn validate_compare_viewports_args(args: &CompareViewportsArgs) -> Result<(), ErrorData> {
    if args.url.is_empty() {
        return Err(ErrorData::invalid_params(
            "url must not be empty".to_string(),
            None,
        ));
    }
    if args.viewports.len() < 2 {
        return Err(ErrorData::invalid_params(
            format!(
                "compare_viewports requires at least 2 viewports, got {}",
                args.viewports.len()
            ),
            None,
        ));
    }
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for v in &args.viewports {
        if v.name.is_empty() {
            return Err(ErrorData::invalid_params(
                "viewport name must not be empty".to_string(),
                None,
            ));
        }
        if v.width == 0 || v.height == 0 {
            return Err(ErrorData::invalid_params(
                format!("viewport `{}` must have non-zero width and height", v.name),
                None,
            ));
        }
        if !seen.insert(v.name.as_str()) {
            return Err(ErrorData::invalid_params(
                format!("viewport name `{}` is duplicated", v.name),
                None,
            ));
        }
    }
    Ok(())
}

// `result` is consumed: we move `result.diffs` (a Vec) into the JSON
// payload and only read `result.summary` / `result.truncated` (both
// `Copy`). Borrowing would force a clone of the diff vector for no
// gain.
#[allow(clippy::needless_pass_by_value)]
fn build_compare_viewports_result(
    url: &str,
    viewport_names: &[String],
    threshold_px: u32,
    result: compare_viewports::CompareResult,
) -> CallToolResult {
    let summary = result.summary;
    let truncated = result.truncated;
    let diff_count = result.diffs.len();

    let mut text = format!(
        "compare_viewports {} across {} viewports: {} diff(s) [missing={}, size={}, reorder={}, style={}]",
        url,
        viewport_names.len(),
        summary.total,
        summary.missing,
        summary.size_changes,
        summary.reordered,
        summary.style_changes,
    );
    if truncated {
        use std::fmt::Write as _;
        let _ = write!(text, " (showing first {diff_count})");
    }

    let structured = json!({
        "url": url,
        "viewports": viewport_names,
        "size_threshold_px": threshold_px,
        "summary": {
            "total": summary.total,
            "missing": summary.missing,
            "size_changes": summary.size_changes,
            "reordered": summary.reordered,
            "style_changes": summary.style_changes,
        },
        "diffs": result.diffs,
        "truncated": truncated,
    });

    // Best-effort cap enforcement. Truncation already shrinks oversized
    // payloads in practice; on the rare path where the cap is still
    // tripped (e.g. unusually long selectors), drop the diff list and
    // keep the summary so the caller never receives an oversized blob.
    let serialized = serde_json::to_string(&structured).unwrap_or_default();
    let final_structured = if serialized.len() > COMPARE_VIEWPORTS_RESPONSE_CAP_BYTES {
        json!({
            "url": url,
            "viewports": viewport_names,
            "size_threshold_px": threshold_px,
            "summary": {
                "total": summary.total,
                "missing": summary.missing,
                "size_changes": summary.size_changes,
                "reordered": summary.reordered,
                "style_changes": summary.style_changes,
            },
            "diffs": [],
            "truncated": true,
            "dropped_for_cap": true,
        })
    } else {
        structured
    };

    let mut result_obj = CallToolResult::success(vec![Content::text(text)]);
    result_obj.structured_content = Some(final_structured);
    result_obj
}

impl ServerHandler for PlumbServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.protocol_version = ProtocolVersion::V_2024_11_05;
        info.capabilities = ServerCapabilities::builder()
            .enable_resources()
            .enable_tools()
            .build();
        info.server_info = Implementation::new("plumb", env!("CARGO_PKG_VERSION"));
        info.instructions = Some(
            "Deterministic design-system linter. Call `lint_url` with a URL to get violations; \
             use `lint_page_html` to lint a self-contained HTML string (rendered in Chromium; \
             external stylesheets are not fetched); \
             use `compare_viewports` to diff a URL across mobile/desktop; \
             use `explain_rule` for canonical rule documentation; use `list_rules` to enumerate \
             every built-in rule; use `get_config` to fetch the resolved `plumb.toml` for a \
             working directory; read `plumb://config` to fetch the resolved config for the \
             server working directory; use `echo` to smoke-test the transport."
                .into(),
        );
        info
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let arguments = request.arguments.unwrap_or_default();
        match request.name.as_ref() {
            "echo" => self.echo(parse_tool_args(arguments)?).await,
            "lint_url" => self.lint_url(parse_tool_args(arguments)?).await,
            "lint_page_html" => self.lint_page_html(parse_tool_args(arguments)?).await,
            "explain_rule" => self.explain_rule(parse_tool_args(arguments)?).await,
            "list_rules" => self.list_rules(parse_tool_args(arguments)?).await,
            "get_config" => self.get_config(parse_tool_args(arguments)?).await,
            "compare_viewports" => self.compare_viewports(parse_tool_args(arguments)?).await,
            unknown => Err(ErrorData::invalid_params(
                format!("unknown tool: {unknown}"),
                None,
            )),
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_ {
        let tools = vec![
            tool_descriptor::<EchoArgs>("echo", "Echo a message — smoke test the MCP transport."),
            tool_descriptor::<LintUrlArgs>(
                "lint_url",
                "Lint a URL with Plumb. Accepts http(s):// and plumb-fake:// URLs. Resolves plumb.toml from working_dir (or the server CWD). Output is aggregated and capped at \u{2264} 10 KB structuredContent.",
            ),
            tool_descriptor::<LintPageHtmlArgs>(
                "lint_page_html",
                "Lint a self-contained HTML string by rendering it in Chromium (via a data: URL) so embedded <style>/inline styles produce real computed styles. External stylesheets/resources are NOT fetched \u{2014} use lint_url for full pages. Resolves plumb.toml from working_dir (or the server CWD). Aggregated, \u{2264} 10 KB structuredContent. Capped at 1 MiB input and 10000 elements.",
            ),
            tool_descriptor::<ExplainRuleArgs>(
                "explain_rule",
                "Return canonical documentation and metadata for a Plumb rule by id.",
            ),
            tool_descriptor::<ListRulesArgs>(
                "list_rules",
                "List every built-in Plumb rule with id, default severity, and one-line summary.",
            ),
            tool_descriptor::<GetConfigArgs>(
                "get_config",
                "Return the resolved plumb.toml for a working directory as JSON. Memoized per (path, mtime).",
            ),
            tool_descriptor::<CompareViewportsArgs>(
                "compare_viewports",
                "Capture snapshots at 2+ viewports and return a deterministic diff (missing nodes, size changes, reorderings, computed-style changes). 10 KB structuredContent budget.",
            ),
        ];
        std::future::ready(Ok(ListToolsResult::with_all_items(tools)))
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, ErrorData>> + Send + '_ {
        let resources = vec![
            RawResource::new(CONFIG_RESOURCE_URI, CONFIG_RESOURCE_NAME)
                .with_description(CONFIG_RESOURCE_DESCRIPTION)
                .with_mime_type(CONFIG_RESOURCE_MIME_TYPE)
                .no_annotation(),
        ];
        std::future::ready(Ok(ListResourcesResult::with_all_items(resources)))
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ReadResourceResult, ErrorData>> + Send + '_ {
        std::future::ready(self.read_resource_impl(&request))
    }
}

impl PlumbServer {
    fn read_resource_impl(
        &self,
        request: &ReadResourceRequestParams,
    ) -> Result<ReadResourceResult, ErrorData> {
        if request.uri != CONFIG_RESOURCE_URI {
            return Err(ErrorData::resource_not_found(
                format!("unknown resource: {}", request.uri),
                None,
            ));
        }

        let resolved = self.resolve_config_for_dir(&self.cwd)?;
        let text = serde_json::to_string(&resolved.value).map_err(|err| {
            ErrorData::internal_error(format!("serialize config resource: {err}"), None)
        })?;

        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(text, CONFIG_RESOURCE_URI)
                .with_mime_type(CONFIG_RESOURCE_MIME_TYPE),
        ]))
    }
}

fn parse_tool_args<T>(arguments: JsonObject) -> Result<T, ErrorData>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(serde_json::Value::Object(arguments)).map_err(|error| {
        ErrorData::invalid_params(
            format!("failed to deserialize tool arguments: {error}"),
            None,
        )
    })
}

fn tool_descriptor<T>(name: &'static str, description: &'static str) -> Tool
where
    T: JsonSchema + 'static,
{
    Tool::new(name, description, schema_for_type::<T>())
}

#[derive(Copy, Clone)]
enum ConfigSource {
    Default,
    File,
}

impl ConfigSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::File => "file",
        }
    }
}

fn serialize_config(
    config: &Config,
    path: &Path,
    source: ConfigSource,
) -> Result<serde_json::Value, ErrorData> {
    let inner = serde_json::to_value(config)
        .map_err(|err| ErrorData::internal_error(format!("serialize config: {err}"), None))?;
    Ok(serde_json::json!({
        "config": inner,
        "source": source.as_str(),
        "path": path.display().to_string(),
    }))
}

fn map_config_error(err: &ConfigError) -> ErrorData {
    ErrorData::internal_error(format!("load plumb.toml: {err}"), None)
}

fn secure_token_eq(expected: &[u8], actual: &[u8]) -> bool {
    expected.ct_eq(actual).into()
}

fn unauthorized_bearer_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Bearer")],
    )
        .into_response()
}

async fn authenticate_http_request(
    state: axum::extract::State<HttpAuthState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| state.token.matches_authorization_header(value));

    if !authorized {
        return unauthorized_bearer_response();
    }

    next.run(request).await
}

/// Run the MCP server on stdin/stdout until EOF.
///
/// On clean exit (EOF on stdin) the persistent browser, if it was
/// warmed during the session, is shut down gracefully via
/// [`PlumbServer::shutdown`]. Service-loop errors take priority over
/// shutdown errors so the original cause surfaces to the caller.
///
/// # Errors
///
/// Returns [`McpError::Service`] if rmcp's service loop fails or
/// [`PlumbServer::shutdown`] surfaces an error, and [`McpError::Io`]
/// on transport errors.
pub async fn run_stdio(cwd: PathBuf) -> Result<(), McpError> {
    let handler = PlumbServer::new(cwd);
    let service = handler
        .clone()
        .serve(stdio())
        .await
        .map_err(|e| McpError::Service(e.to_string()))?;
    let service_result = service
        .waiting()
        .await
        .map_err(|e| McpError::Service(e.to_string()));

    let shutdown_result = handler.shutdown().await;

    // Surface the service-loop error first so the original cause
    // wins; only report a shutdown failure when the service itself
    // returned cleanly.
    service_result?;
    shutdown_result?;
    Ok(())
}

/// Run the MCP server over Streamable HTTP.
///
/// Requests must include `Authorization: Bearer <token>`. Missing or
/// invalid bearer tokens are rejected with HTTP 401 before the request
/// reaches the MCP transport.
///
/// # Errors
///
/// Returns [`McpError::Io`] when the TCP listener or HTTP server fails,
/// and [`McpError::Service`] when graceful shutdown of the underlying
/// MCP service fails.
pub async fn run_http(cwd: PathBuf, addr: SocketAddr, token: String) -> Result<(), McpError> {
    let handler = PlumbServer::new(cwd);
    let service: StreamableHttpService<PlumbServer, LocalSessionManager> =
        StreamableHttpService::new(
            {
                let handler = handler.clone();
                move || Ok(handler.clone())
            },
            Arc::default(),
            StreamableHttpServerConfig::default(),
        );

    let app = Router::new()
        .fallback_service(service)
        .layer(middleware::from_fn_with_state(
            HttpAuthState {
                token: HttpAuthToken::new(token),
            },
            authenticate_http_request,
        ));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let serve_result = axum::serve(listener, app).await.map_err(McpError::Io);
    let shutdown_result = handler.shutdown().await;

    serve_result?;
    shutdown_result?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use plumb_core::Severity;
    use rmcp::model::ErrorCode;

    use super::*;

    fn violation_with_message(message_len: usize, dom_order: u64) -> Violation {
        Violation {
            rule_id: "spacing/grid-conformance".to_owned(),
            severity: Severity::Warning,
            message: "x".repeat(message_len),
            selector: "html > body".to_owned(),
            viewport: ViewportKey::new("desktop"),
            rect: None,
            dom_order,
            fix: None,
            doc_url: "https://plumb.aramhammoudeh.com/rules/spacing-grid-conformance".to_owned(),
            metadata: std::iter::empty().collect(),
        }
    }

    #[test]
    fn full_lint_payload_includes_json_envelope() {
        let structured =
            build_full_lint_payload(&[violation_with_message(32, 1)]).expect("full payload");

        assert_eq!(
            structured["plumb_version"].as_str(),
            Some(env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(structured["summary"]["total"].as_u64(), Some(1));
        assert!(
            structured["run_id"]
                .as_str()
                .expect("run_id")
                .starts_with("sha256:")
        );
    }

    #[test]
    fn full_lint_payload_rejects_payloads_above_cap() {
        let violations: Vec<Violation> = (0_u64..32)
            .map(|dom_order| violation_with_message(2_000, dom_order))
            .collect();

        let err = build_full_lint_payload(&violations).expect_err("payload must exceed 50 KB");
        assert!(
            err.to_string()
                .contains("detail=full payload exceeds 50 KB response cap"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn build_lint_url_result_rejects_full_payloads_above_cap() {
        let violations: Vec<Violation> = (0_u64..32)
            .map(|dom_order| violation_with_message(2_000, dom_order))
            .collect();

        let err = build_lint_url_result(&violations, LintUrlDetail::Full)
            .expect_err("full mode must reject payloads above the response cap");
        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(
            err.to_string()
                .contains("detail=full payload exceeds 50 KB response cap"),
            "unexpected error: {err:?}"
        );
    }
}
