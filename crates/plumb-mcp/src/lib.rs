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

use std::io;

use plumb_cdp::{BrowserDriver, ChromiumDriver, ChromiumOptions, Target, is_fake_url};
use plumb_core::{Config, PlumbSnapshot, ViewportKey, register_builtin, run};
use plumb_format::mcp_compact;
use rmcp::{
    RoleServer, ServerHandler, ServiceExt,
    handler::server::tool::schema_for_type,
    model::{
        CallToolRequestParams, CallToolResult, Content, ErrorData, Implementation, JsonObject,
        ListToolsResult, PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerInfo,
        Tool,
    },
    schemars::{self, JsonSchema},
    service::RequestContext,
    transport::stdio,
};
use serde::Deserialize;
use serde_json::{Value, json};
use thiserror::Error;

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

/// The Plumb MCP server. Cheap to construct.
#[derive(Clone, Default)]
pub struct PlumbServer {
    _private: (),
}

impl PlumbServer {
    /// Construct a new server.
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }

    async fn echo(&self, args: EchoArgs) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text(args.message)]))
    }

    async fn lint_url(&self, args: LintUrlArgs) -> Result<CallToolResult, ErrorData> {
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
            let driver = ChromiumDriver::new(ChromiumOptions::default());
            match driver.snapshot(target).await {
                Ok(snap) => snap,
                Err(err) => {
                    return Ok(CallToolResult::error(vec![Content::text(format!(
                        "lint_url failed: {err}"
                    ))]));
                }
            }
        };
        let config = Config::default();
        let violations = run(&snapshot, &config);
        let (text, structured) = mcp_compact(&violations);
        let mut result = CallToolResult::success(vec![Content::text(text)]);
        result.structured_content = Some(structured);
        Ok(result)
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
}

impl ServerHandler for PlumbServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.protocol_version = ProtocolVersion::V_2024_11_05;
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info = Implementation::new("plumb", env!("CARGO_PKG_VERSION"));
        info.instructions = Some(
            "Deterministic design-system linter. Call `lint_url` with a URL to get violations; \
             use `explain_rule` for canonical rule documentation; use `list_rules` to enumerate \
             every built-in rule; use `echo` to smoke-test the transport."
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
        ];
        std::future::ready(Ok(ListToolsResult::with_all_items(tools)))
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

/// Run the MCP server on stdin/stdout until EOF.
///
/// # Errors
///
/// Returns [`McpError::Service`] if rmcp's service loop fails or [`McpError::Io`]
/// on transport errors.
pub async fn run_stdio() -> Result<(), McpError> {
    let handler = PlumbServer::new();
    let service = handler
        .serve(stdio())
        .await
        .map_err(|e| McpError::Service(e.to_string()))?;
    service
        .waiting()
        .await
        .map_err(|e| McpError::Service(e.to_string()))?;
    Ok(())
}
