//! # plumb-mcp
//!
//! Model Context Protocol server for Plumb, backed by the official
//! [`rmcp`] Rust SDK.
//!
//! The server exposes two tools to AI coding agents:
//!
//! - `echo` — smoke-tests the transport.
//! - `lint_url` — lints a URL and returns violations in the MCP-compact
//!   shape from `docs/local/prd.md` §14.2. Walking-skeleton accepts
//!   `plumb-fake://` URLs only.
//!
//! The [`PlumbServer`] type implements [`rmcp::ServerHandler`] directly.
//! Extend it by adding a tool descriptor to `list_tools` and a matching
//! branch in `call_tool`; see `.agents/rules/mcp-tool-patterns.md`.
//!
//! [`rmcp`]: https://crates.io/crates/rmcp

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::{borrow::Cow, io, sync::Arc};

use plumb_core::{Config, PlumbSnapshot, run};
use plumb_format::mcp_compact;
use rmcp::{
    RoleServer, ServerHandler, ServiceExt,
    handler::server::tool::schema_for_type,
    model::{
        CallToolRequestParam, CallToolResult, Content, ErrorData, Implementation, JsonObject,
        ListToolsResult, PaginatedRequestParam, ProtocolVersion, ServerCapabilities, ServerInfo,
        Tool,
    },
    schemars::{self, JsonSchema},
    service::RequestContext,
    transport::stdio,
};
use serde::Deserialize;
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
        // rmcp 0.2 CallToolResult carries text/image/resource content
        // directly. Ship the structured JSON as an additional text block;
        // agents that want strict JSON parse the second block.
        let structured_text = serde_json::to_string(&structured).map_err(|error| {
            ErrorData::internal_error(
                format!("failed to serialize structured MCP result: {error}"),
                None,
            )
        })?;
        Ok(CallToolResult::success(vec![
            Content::text(text),
            Content::text(structured_text),
        ]))
    }
}

impl ServerHandler for PlumbServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "plumb".into(),
                title: None,
                version: env!("CARGO_PKG_VERSION").into(),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "Deterministic design-system linter. Call `lint_url` with a URL to get violations; \
                 use `echo` to smoke-test the transport."
                    .into(),
            ),
        }
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let arguments = request.arguments.unwrap_or_default();
        match request.name.as_ref() {
            "echo" => self.echo(parse_tool_args(arguments)?).await,
            "lint_url" => self.lint_url(parse_tool_args(arguments)?).await,
            unknown => Err(ErrorData::invalid_params(
                format!("unknown tool: {unknown}"),
                None,
            )),
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_ {
        let tools = vec![
            tool_descriptor::<EchoArgs>("echo", "Echo a message — smoke test the MCP transport."),
            tool_descriptor::<LintUrlArgs>(
                "lint_url",
                "Lint a URL with Plumb. Walking-skeleton accepts plumb-fake:// URLs only.",
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
    T: JsonSchema,
{
    Tool {
        name: Cow::Borrowed(name),
        title: None,
        description: Some(Cow::Borrowed(description)),
        input_schema: Arc::new(schema_for_type::<T>()),
        output_schema: None,
        annotations: None,
        icons: None,
        meta: None,
    }
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
