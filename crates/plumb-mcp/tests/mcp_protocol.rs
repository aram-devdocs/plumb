//! In-process JSON-RPC round-trips against the hand-rolled MCP handler.
//!
//! These tests don't spawn a subprocess — they drive the handler via the
//! hidden `__dispatch` entry point. That's enough to prove the protocol
//! shape; subprocess-driven tests land alongside the real `rmcp`
//! integration in a later PR.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::missing_panics_doc)]

use serde_json::{Value, json};

fn dispatch(request: &Value) -> Value {
    let body = serde_json::to_string(request).expect("serialize request");
    let out = plumb_mcp::__dispatch(&body);
    serde_json::from_str(&out).expect("parse response")
}

#[test]
fn initialize_returns_server_info() {
    let resp = dispatch(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }));
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["serverInfo"]["name"], "plumb");
}

#[test]
fn tools_list_returns_echo_and_lint_url() {
    let resp = dispatch(&json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    }));
    let tools = resp["result"]["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"echo"));
    assert!(names.contains(&"lint_url"));
}

#[test]
fn echo_round_trips() {
    let resp = dispatch(&json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "echo",
            "arguments": { "message": "hi" }
        }
    }));
    assert_eq!(resp["result"]["content"][0]["text"], "hi");
}

#[test]
fn lint_url_walks_skeleton_against_fake_url() {
    let resp = dispatch(&json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "lint_url",
            "arguments": { "url": "plumb-fake://hello" }
        }
    }));
    let structured = &resp["result"]["structuredContent"];
    assert_eq!(structured["counts"]["total"], 1);
    assert_eq!(
        structured["violations"][0]["rule_id"],
        "placeholder/hello-world"
    );
}

#[test]
fn unknown_method_yields_method_not_found() {
    let resp = dispatch(&json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "does/not/exist"
    }));
    assert_eq!(resp["error"]["code"], -32601);
}

#[test]
fn unknown_tool_yields_invalid_params() {
    let resp = dispatch(&json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "tools/call",
        "params": {
            "name": "nope",
            "arguments": {}
        }
    }));
    assert_eq!(resp["error"]["code"], -32602);
}
