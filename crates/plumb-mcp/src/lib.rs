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
//!   (served from the canned snapshot).
//! - `explain_rule` — returns the canonical markdown documentation and
//!   metadata for a built-in rule by id.
//! - `list_rules` — enumerates every built-in rule with id, default
//!   severity, and one-line summary.
//! - `get_config` — resolves `plumb.toml` for a given working directory
//!   and returns the [`Config`] as JSON. Memoized per `(path, mtime)`.
//!
//! The [`PlumbServer`] type implements [`rmcp::ServerHandler`] directly.
//! Extend it by adding a tool descriptor to `list_tools` and a matching
//! branch in `call_tool`; see `.agents/rules/mcp-tool-patterns.md`.
//!
//! [`rmcp`]: https://crates.io/crates/rmcp

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod explain;

#[doc(hidden)]
pub use explain::rule_ids as documented_rule_ids;

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use plumb_cdp::{ChromiumOptions, PersistentBrowser, Target, is_fake_url};
use plumb_config::ConfigError;
use plumb_core::{Config, PlumbSnapshot, ViewportKey, Violation, register_builtin, run};
use plumb_format::{json as full_json, mcp_compact};
use rmcp::{
    RoleServer, ServerHandler, ServiceExt,
    handler::server::tool::schema_for_type,
    model::{
        CallToolRequestParams, CallToolResult, Content, ErrorData, Implementation, JsonObject,
        ListResourcesResult, ListToolsResult, PaginatedRequestParams, ProtocolVersion,
        RawResource, ReadResourceRequestParams, ReadResourceResult, ResourceContents,
        ServerCapabilities, ServerInfo, Tool,
    },
    schemars::{self, JsonSchema},
    service::RequestContext,
    transport::stdio,
    model::AnnotateAble,
};
use serde::Deserialize;
use serde_json::{Value, json};
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

/// The Plumb MCP server.
///
/// Cheap to construct; `Clone` shares the memo cache and the persistent
/// browser handle. The browser is warmed lazily on the first non-fake
/// `lint_url` call and reused for the rest of the session — see
/// [`PersistentBrowser`] for the per-call incognito-context invariant.
#[derive(Clone, Default)]
pub struct PlumbServer {
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
    pub fn new() -> Self {
        Self::default()
    }

    async fn echo(&self, args: EchoArgs) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text(args.message)]))
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
    /// Driver failures (Chromium not found, version out of range,
    /// CDP error, malformed snapshot) are returned as a successful
    /// JSON-RPC response with `isError = true` and a single text
    /// content block — never as a JSON-RPC error.
    pub async fn lint_url(&self, args: LintUrlArgs) -> Result<CallToolResult, ErrorData> {
        let snapshot = if is_fake_url(&args.url) {
            PlumbSnapshot::canned()
        } else {
            let target = Target {
                url: args.url.clone(),
                viewport: ViewportKey::new("desktop"),
                width: 1280,
                height: 800,
                device_pixel_ratio: 1.0,
            };
            match self
                .browser
                .get_or_try_init(|| PersistentBrowser::launch(ChromiumOptions::default()))
                .await
            {
                Ok(browser) => match browser.snapshot(target).await {
                    Ok(snap) => snap,
                    Err(err) => {
                        return Ok(CallToolResult::error(vec![Content::text(format!(
                            "lint_url failed: {err}"
                        ))]));
                    }
                },
                Err(err) => {
                    return Ok(CallToolResult::error(vec![Content::text(format!(
                        "lint_url failed: {err}"
                    ))]));
                }
            }
        };
        let config = Config::default();
        let violations = run(&snapshot, &config);
        build_lint_url_result(&violations, args.detail)
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
            "explain_rule" => self.explain_rule(parse_tool_args(arguments)?).await,
            "list_rules" => self.list_rules(parse_tool_args(arguments)?).await,
            "get_config" => self.get_config(parse_tool_args(arguments)?).await,
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
                "Lint a URL with Plumb. Accepts http(s):// and plumb-fake:// URLs.",
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
        ];
        std::future::ready(Ok(ListToolsResult::with_all_items(tools)))
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, ErrorData>> + Send + '_ {
        let resources = vec![RawResource::new(CONFIG_RESOURCE_URI, CONFIG_RESOURCE_NAME)
            .with_description(CONFIG_RESOURCE_DESCRIPTION)
            .with_mime_type(CONFIG_RESOURCE_MIME_TYPE)
            .no_annotation()];
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

        let working_dir = std::env::current_dir()
            .map_err(|err| ErrorData::internal_error(format!("current_dir: {err}"), None))?;
        let resolved = self.resolve_config_for_dir(&working_dir)?;
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
pub async fn run_stdio() -> Result<(), McpError> {
    let handler = PlumbServer::new();
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
