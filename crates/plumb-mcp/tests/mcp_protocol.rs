//! Unit-scope tests for `plumb-mcp`.
//!
//! Protocol-level tests that spawn the real `plumb mcp` subprocess and
//! speak JSON-RPC over stdio live in `crates/plumb-cli/tests/mcp_stdio.rs`.
//! This file exercises only what's verifiable in-process: server info,
//! construction, default shape.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::missing_panics_doc)]

use std::collections::BTreeSet;

use plumb_core::register_builtin;
use plumb_mcp::{ExplainRuleArgs, PlumbServer, documented_rule_ids};
use rmcp::ServerHandler;
use rmcp::model::ErrorCode;

#[test]
fn server_info_declares_plumb() {
    let server = PlumbServer::new();
    let info = server.get_info();
    assert_eq!(info.server_info.name, "plumb");
    assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
}

#[test]
fn server_info_declares_tool_capability() {
    let server = PlumbServer::new();
    let info = server.get_info();
    assert!(
        info.capabilities.tools.is_some(),
        "server must advertise the `tools` capability"
    );
}

#[test]
fn server_info_includes_instructions() {
    let server = PlumbServer::new();
    let info = server.get_info();
    assert!(
        info.instructions.is_some(),
        "server must advertise instructions for agents"
    );
}

#[test]
fn default_is_equivalent_to_new() {
    // Smoke-test that Default::default() constructs a usable server.
    let _server = PlumbServer::default();
}

#[tokio::test]
async fn explain_rule_happy_path_returns_markdown_and_metadata() {
    let server = PlumbServer::new();
    let result = server
        .explain_rule(ExplainRuleArgs {
            rule_id: "spacing/scale-conformance".to_owned(),
        })
        .await
        .expect("explain_rule must succeed for a known built-in rule id");

    let text = result
        .content
        .iter()
        .find_map(|content| content.as_text().map(|text| text.text.clone()))
        .expect("response must include a text content block");
    assert!(
        !text.is_empty(),
        "markdown body in content[0] must be non-empty"
    );
    assert!(
        text.contains("spacing/scale-conformance"),
        "markdown should reference its own rule id"
    );

    let structured = result
        .structured_content
        .expect("response must include structured_content");
    assert_eq!(
        structured.get("rule_id").and_then(|v| v.as_str()),
        Some("spacing/scale-conformance"),
    );
    assert_eq!(
        structured.get("severity").and_then(|v| v.as_str()),
        Some("warning"),
    );
    assert_eq!(
        structured.get("doc_url").and_then(|v| v.as_str()),
        Some("https://plumb.aramhammoudeh.com/rules/spacing-scale-conformance"),
    );
    let summary = structured
        .get("summary")
        .and_then(|v| v.as_str())
        .expect("summary field must be a string");
    assert!(!summary.is_empty(), "summary must be non-empty");
    let markdown = structured
        .get("markdown")
        .and_then(|v| v.as_str())
        .expect("markdown field must be a string");
    assert!(!markdown.is_empty(), "markdown field must be non-empty");
    assert_eq!(
        markdown, text,
        "structured markdown must match the content text block"
    );
}

#[tokio::test]
async fn explain_rule_unknown_rule_id_returns_invalid_params() {
    let server = PlumbServer::new();
    let error = server
        .explain_rule(ExplainRuleArgs {
            rule_id: "does/not-exist".to_owned(),
        })
        .await
        .expect_err("unknown rule id must fail");
    assert_eq!(
        error.code,
        ErrorCode::INVALID_PARAMS,
        "unknown rule id must map to JSON-RPC -32602"
    );
}

#[test]
fn every_builtin_rule_has_doc_entry() {
    let registered: BTreeSet<&'static str> =
        register_builtin().iter().map(|rule| rule.id()).collect();
    let documented: BTreeSet<&'static str> = documented_rule_ids().into_iter().collect();
    assert_eq!(
        registered, documented,
        "the explain_rule doc table must cover every rule in register_builtin() and nothing more",
    );
}
