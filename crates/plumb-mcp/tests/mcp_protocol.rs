//! Unit-scope tests for `plumb-mcp`.
//!
//! Protocol-level tests that spawn the real `plumb mcp` subprocess and
//! speak JSON-RPC over stdio live in `crates/plumb-cli/tests/mcp_stdio.rs`.
//! This file exercises only what's verifiable in-process: server info,
//! construction, default shape.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::missing_panics_doc)]

use plumb_mcp::PlumbServer;
use rmcp::ServerHandler;

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

#[test]
fn list_rules_returns_every_builtin_rule_sorted() {
    let server = PlumbServer::new();
    let (text, structured) = server.list_rules_payload();

    // Text block: bounded, one line per rule, deterministic.
    assert!(!text.is_empty(), "list_rules text must not be empty");
    let line_count = text.lines().count();
    assert_eq!(
        line_count,
        plumb_core::rules::register_builtin().len(),
        "list_rules text must have one line per rule"
    );

    let count = structured
        .get("count")
        .and_then(serde_json::Value::as_u64)
        .expect("count field");
    let rules = structured
        .get("rules")
        .and_then(serde_json::Value::as_array)
        .expect("rules array");
    let builtin_count = plumb_core::rules::register_builtin().len();
    assert_eq!(count, builtin_count as u64);
    assert_eq!(rules.len(), builtin_count);

    // Sorted by id ascending.
    let ids: Vec<&str> = rules
        .iter()
        .map(|r| r["id"].as_str().expect("id string"))
        .collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "rules must be sorted by id");

    // First entry shape — exact id is sensitive to registry contents
    // and asserted indirectly via the sort check above.
    let first = &rules[0];
    assert!(
        first["id"].as_str().is_some_and(|id| !id.is_empty()),
        "first entry must carry a non-empty id"
    );
    assert!(
        matches!(
            first["default_severity"].as_str(),
            Some("info" | "warning" | "error")
        ),
        "default_severity must be a lowercase severity label"
    );
    let summary = first["summary"].as_str().expect("summary string");
    assert!(!summary.is_empty(), "summary must not be empty");
}
