//! # plumb-mcp
//!
//! Model Context Protocol server for Plumb.
//!
//! ## Walking-skeleton implementation
//!
//! This crate implements the newline-delimited JSON-RPC 2.0 subset of
//! MCP by hand. It handles `initialize`, `tools/list`, and `tools/call`
//! for the `echo` and `lint_url` tools. A full [`rmcp`] integration with
//! capability discovery, tool schemas, and progress streaming lands in a
//! later PR — see the tracking issue template under
//! `.github/ISSUE_TEMPLATE/`.
//!
//! [`rmcp`]: https://crates.io/crates/rmcp
//!
//! ## Why hand-rolled?
//!
//! The walking skeleton needs deterministic compiles on any toolchain
//! matching `rust-toolchain.toml`. rmcp's public API is evolving; pinning
//! to a specific macro surface now would guarantee churn. The hand-rolled
//! server speaks the same wire protocol, so AI agents and test clients
//! work against it unchanged when rmcp drops in.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::io;

use plumb_core::{Config, PlumbSnapshot, run};
use plumb_format::mcp_compact;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// MCP server errors.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum McpError {
    /// Underlying I/O error on stdin/stdout.
    #[error("mcp i/o: {0}")]
    Io(#[from] io::Error),
    /// JSON (de)serialization error.
    #[error("mcp json: {0}")]
    Json(#[from] serde_json::Error),
}

/// The pinned JSON-RPC version string.
const JSONRPC_VERSION: &str = "2.0";

/// The MCP protocol version this server implements.
const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

/// A JSON-RPC 2.0 request.
#[derive(Debug, Deserialize)]
struct Request {
    #[serde(default)]
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

/// A JSON-RPC 2.0 response.
#[derive(Debug, Serialize)]
struct Response {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorObj>,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Serialize)]
struct ErrorObj {
    code: i32,
    message: String,
}

/// Run the MCP server on stdin/stdout until EOF.
///
/// # Errors
///
/// Returns [`McpError::Io`] on any stdin/stdout failure. Malformed
/// requests are answered with JSON-RPC errors rather than aborting the
/// loop.
pub async fn run_stdio() -> Result<(), McpError> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            // EOF
            return Ok(());
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            continue;
        }

        let response_json = match serde_json::from_str::<Request>(trimmed) {
            Ok(req) => handle(req),
            Err(err) => error_response(Value::Null, -32700, format!("parse error: {err}")),
        };

        let bytes = serde_json::to_vec(&response_json)?;
        stdout.write_all(&bytes).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }
}

fn handle(req: Request) -> Value {
    if req.jsonrpc != JSONRPC_VERSION {
        return error_response(
            req.id.unwrap_or(Value::Null),
            -32600,
            format!("invalid jsonrpc version: {}", req.jsonrpc),
        );
    }
    let id = req.id.clone().unwrap_or(Value::Null);

    match req.method.as_str() {
        "initialize" => ok_response(id, initialize_result()),
        "tools/list" => ok_response(id, tools_list_result()),
        "tools/call" => tools_call(id, &req.params.unwrap_or(Value::Null)),
        "ping" => ok_response(id, json!({})),
        other => error_response(id, -32601, format!("method not found: {other}")),
    }
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "capabilities": {
            "tools": { "listChanged": false }
        },
        "serverInfo": {
            "name": "plumb",
            "version": env!("CARGO_PKG_VERSION"),
        },
    })
}

fn tools_list_result() -> Value {
    json!({
        "tools": [
            {
                "name": "echo",
                "description": "Echo a message — smoke test the MCP transport.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    },
                    "required": ["message"],
                }
            },
            {
                "name": "lint_url",
                "description": "Lint a URL with Plumb. Walking-skeleton accepts `plumb-fake://` URLs only.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "url": { "type": "string" }
                    },
                    "required": ["url"],
                }
            }
        ]
    })
}

fn tools_call(id: Value, params: &Value) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(Value::Null);

    match name {
        "echo" => {
            let msg = args.get("message").and_then(Value::as_str).unwrap_or("");
            ok_response(id, tool_text_result(msg))
        }
        "lint_url" => {
            let url = args.get("url").and_then(Value::as_str).unwrap_or("");
            if !url.starts_with("plumb-fake://") {
                return ok_response(
                    id,
                    tool_text_result(
                        "lint_url currently only accepts plumb-fake:// URLs in the walking skeleton.",
                    ),
                );
            }
            let snapshot = PlumbSnapshot::canned();
            let config = Config::default();
            let violations = run(&snapshot, &config);
            let (text, structured) = mcp_compact(&violations);
            ok_response(
                id,
                json!({
                    "content": [{ "type": "text", "text": text }],
                    "isError": false,
                    "structuredContent": structured,
                }),
            )
        }
        other => error_response(id, -32602, format!("unknown tool: {other}")),
    }
}

fn tool_text_result(text: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false,
    })
}

fn ok_response(id: Value, result: Value) -> Value {
    serde_json::to_value(Response {
        jsonrpc: JSONRPC_VERSION,
        id,
        result: Some(result),
        error: None,
    })
    .unwrap_or(Value::Null)
}

fn error_response(id: Value, code: i32, message: String) -> Value {
    serde_json::to_value(Response {
        jsonrpc: JSONRPC_VERSION,
        id,
        result: None,
        error: Some(ErrorObj { code, message }),
    })
    .unwrap_or(Value::Null)
}

// Export the handler for unit tests and in-process harnesses.
#[doc(hidden)]
pub fn __dispatch(request: &str) -> String {
    match serde_json::from_str::<Request>(request) {
        Ok(req) => serde_json::to_string(&handle(req)).unwrap_or_default(),
        Err(err) => serde_json::to_string(&error_response(
            Value::Null,
            -32700,
            format!("parse error: {err}"),
        ))
        .unwrap_or_default(),
    }
}
