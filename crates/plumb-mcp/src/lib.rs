//! # plumb-mcp
//!
//! Model Context Protocol server for Plumb, backed by the official
//! [`rmcp`] Rust SDK.
//!
//! The server exposes three tools to AI coding agents:
//!
//! - `echo` — smoke-tests the transport.
//! - `lint_url` — lints a URL and returns violations in the MCP-compact
//!   shape from `docs/local/prd.md` §14.2. Walking-skeleton accepts
//!   `plumb-fake://` URLs only.
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

use std::io;

use plumb_core::{Config, PlumbSnapshot, rules::register_builtin, run};
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
    /// URL to lint. Walking-skeleton accepts `plumb-fake://` URLs only.
    pub url: String,
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
        if !args.url.starts_with("plumb-fake://") {
            return Ok(CallToolResult::success(vec![Content::text(
                "lint_url currently only accepts plumb-fake:// URLs in the walking skeleton.",
            )]));
        }
        let snapshot = PlumbSnapshot::canned();
        let config = Config::default();
        let violations = run(&snapshot, &config);
        let (text, structured) = mcp_compact(&violations);
        let mut result = CallToolResult::success(vec![Content::text(text)]);
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
            "Deterministic design-system linter. Call `lint_url` with a URL to get violations, \
             `list_rules` to enumerate every built-in rule, or `echo` to smoke-test the transport."
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
                "Lint a URL with Plumb. Walking-skeleton accepts plumb-fake:// URLs only.",
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
