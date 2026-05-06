//! Unit-scope tests for `plumb-mcp`.
//!
//! Protocol-level tests that spawn the real `plumb mcp` subprocess and
//! speak JSON-RPC over stdio live in `crates/plumb-cli/tests/mcp_stdio.rs`.
//! This file exercises only what's verifiable in-process: server info,
//! construction, and default shape.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::missing_panics_doc)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use plumb_core::register_builtin;
use plumb_mcp::{
    CompareViewport, CompareViewportsArgs, ExplainRuleArgs, LintUrlArgs, LintUrlDetail,
    PlumbServer, documented_rule_ids,
};
use rmcp::ServerHandler;
use rmcp::model::ErrorCode;

fn server() -> PlumbServer {
    PlumbServer::new(PathBuf::from("/"))
}

#[test]
fn server_info_declares_plumb() {
    let server = server();
    let info = server.get_info();
    assert_eq!(info.server_info.name, "plumb");
    assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
}

#[test]
fn server_info_declares_tool_capability() {
    let server = server();
    let info = server.get_info();
    assert!(
        info.capabilities.tools.is_some(),
        "server must advertise the `tools` capability"
    );
}

#[test]
fn server_info_declares_resource_capability() {
    let server = server();
    let info = server.get_info();
    assert!(
        info.capabilities.resources.is_some(),
        "server must advertise the `resources` capability"
    );
}

#[test]
fn server_info_includes_instructions() {
    let server = server();
    let info = server.get_info();
    assert!(
        info.instructions.is_some(),
        "server must advertise instructions for agents"
    );
}

#[tokio::test]
async fn explain_rule_happy_path_returns_markdown_and_metadata() {
    let server = server();
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
    let server = server();
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

#[test]
fn list_rules_returns_every_builtin_rule_sorted() {
    let server = server();
    let (text, structured) = server.list_rules_payload();

    let builtin_count = register_builtin().len();

    // Text block: bounded, one line per rule, deterministic.
    assert!(!text.is_empty(), "list_rules text must not be empty");
    let line_count = text.lines().count();
    assert_eq!(
        line_count, builtin_count,
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

/// `plumb-fake://` URLs MUST be served from the canned snapshot
/// without warming Chromium. We assert this behaviorally: on hosts
/// without Chromium, a fake-url `lint_url` succeeds, then `shutdown`
/// is a no-op (no browser was launched, so there is nothing to close).
#[tokio::test]
async fn fake_url_lint_does_not_warm_chromium_and_shutdown_is_noop() {
    let server = server();
    let result = server
        .lint_url(LintUrlArgs {
            url: "plumb-fake://hello".to_owned(),
            detail: LintUrlDetail::default(),
        })
        .await
        .expect("fake-url lint must succeed without a browser");

    assert!(
        !result.is_error.unwrap_or(false),
        "fake-url lint must not surface a driver error"
    );
    let structured = result
        .structured_content
        .expect("fake-url lint must return structured content");
    assert!(
        structured.get("violations").is_some() && structured.get("counts").is_some(),
        "structured payload follows the mcp_compact shape (violations + counts)"
    );

    server
        .shutdown()
        .await
        .expect("shutdown must be a no-op when no browser was warmed");
    server
        .shutdown()
        .await
        .expect("shutdown must remain idempotent across repeated calls");
}

/// Ten back-to-back fake-url calls all succeed and never trip the
/// browser-warm path — a regression guard against a future refactor
/// that accidentally routes the fake scheme through Chromium.
#[tokio::test]
async fn many_fake_url_lints_share_one_server_without_warming_chromium() {
    let server = server();
    for _ in 0..10 {
        let result = server
            .lint_url(LintUrlArgs {
                url: "plumb-fake://hello".to_owned(),
                detail: LintUrlDetail::default(),
            })
            .await
            .expect("fake-url lint must succeed without a browser");
        assert!(
            !result.is_error.unwrap_or(false),
            "fake-url lint must not surface a driver error"
        );
    }
    server
        .shutdown()
        .await
        .expect("shutdown must be a no-op after only fake-url calls");
}

fn viewport(name: &str, width: u32, height: u32, dpr: f32) -> CompareViewport {
    CompareViewport {
        name: name.to_owned(),
        width,
        height,
        dpr,
    }
}

/// Happy path: two viewports against `plumb-fake://hello` succeed
/// without warming Chromium and produce a structured payload with the
/// expected shape.
#[tokio::test]
async fn compare_viewports_happy_path_returns_structured_payload() {
    let server = server();
    let result = server
        .compare_viewports(CompareViewportsArgs {
            url: "plumb-fake://hello".to_owned(),
            viewports: vec![
                viewport("mobile", 375, 800, 2.0),
                viewport("desktop", 1280, 800, 1.0),
            ],
            size_threshold_px: None,
        })
        .await
        .expect("fake-url compare_viewports must succeed");

    assert!(
        !result.is_error.unwrap_or(false),
        "fake-url compare_viewports must not surface a driver error"
    );

    let structured = result
        .structured_content
        .expect("response must include structuredContent");

    assert_eq!(
        structured.get("url").and_then(serde_json::Value::as_str),
        Some("plumb-fake://hello"),
    );
    assert_eq!(
        structured
            .get("size_threshold_px")
            .and_then(serde_json::Value::as_u64),
        Some(4),
    );
    let viewports = structured
        .get("viewports")
        .and_then(serde_json::Value::as_array)
        .expect("viewports array");
    assert_eq!(viewports.len(), 2);
    assert_eq!(viewports[0].as_str(), Some("mobile"));
    assert_eq!(viewports[1].as_str(), Some("desktop"));

    let summary = structured
        .get("summary")
        .and_then(serde_json::Value::as_object)
        .expect("summary object");
    assert_eq!(summary["total"].as_u64(), Some(0));
    assert_eq!(summary["missing"].as_u64(), Some(0));
    assert_eq!(summary["size_changes"].as_u64(), Some(0));
    assert_eq!(summary["reordered"].as_u64(), Some(0));
    assert_eq!(summary["style_changes"].as_u64(), Some(0));

    assert!(
        structured
            .get("diffs")
            .and_then(serde_json::Value::as_array)
            .is_some_and(std::vec::Vec::is_empty),
        "identical canned snapshots produce zero diffs"
    );
    assert_eq!(
        structured
            .get("truncated")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );

    server
        .shutdown()
        .await
        .expect("shutdown must be a no-op when no browser was warmed");
}

/// Fewer than two viewports MUST return JSON-RPC `-32602`.
#[tokio::test]
async fn compare_viewports_requires_two_viewports() {
    let server = server();
    let error = server
        .compare_viewports(CompareViewportsArgs {
            url: "plumb-fake://hello".to_owned(),
            viewports: vec![viewport("desktop", 1280, 800, 1.0)],
            size_threshold_px: None,
        })
        .await
        .expect_err("single-viewport input must be rejected");
    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    assert!(
        error.to_string().contains("at least 2 viewports"),
        "unexpected error: {error:?}"
    );
}

/// Duplicate viewport names MUST return JSON-RPC `-32602`.
#[tokio::test]
async fn compare_viewports_rejects_duplicate_viewport_names() {
    let server = server();
    let error = server
        .compare_viewports(CompareViewportsArgs {
            url: "plumb-fake://hello".to_owned(),
            viewports: vec![
                viewport("desktop", 1280, 800, 1.0),
                viewport("desktop", 1440, 900, 2.0),
            ],
            size_threshold_px: None,
        })
        .await
        .expect_err("duplicate viewport names must be rejected");
    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    assert!(
        error.to_string().contains("duplicated"),
        "unexpected error: {error:?}"
    );
}

/// Empty URL MUST return JSON-RPC `-32602`.
#[tokio::test]
async fn compare_viewports_rejects_empty_url() {
    let server = server();
    let error = server
        .compare_viewports(CompareViewportsArgs {
            url: String::new(),
            viewports: vec![
                viewport("mobile", 375, 800, 2.0),
                viewport("desktop", 1280, 800, 1.0),
            ],
            size_threshold_px: None,
        })
        .await
        .expect_err("empty url must be rejected");
    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
}

/// Zero-dimension viewports MUST return JSON-RPC `-32602`.
#[tokio::test]
async fn compare_viewports_rejects_zero_dimension_viewports() {
    let server = server();
    let error = server
        .compare_viewports(CompareViewportsArgs {
            url: "plumb-fake://hello".to_owned(),
            viewports: vec![
                viewport("mobile", 0, 800, 2.0),
                viewport("desktop", 1280, 800, 1.0),
            ],
            size_threshold_px: None,
        })
        .await
        .expect_err("zero-width viewport must be rejected");
    assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
}

/// Three back-to-back invocations with identical inputs MUST produce
/// byte-identical structured payloads — the determinism contract from
/// PRD §16.
#[tokio::test]
async fn compare_viewports_is_deterministic_across_runs() {
    let server = server();
    let args = || CompareViewportsArgs {
        url: "plumb-fake://hello".to_owned(),
        viewports: vec![
            viewport("mobile", 375, 800, 2.0),
            viewport("desktop", 1280, 800, 1.0),
            viewport("widescreen", 1920, 1080, 1.0),
        ],
        size_threshold_px: Some(8),
    };
    let r1 = server
        .compare_viewports(args())
        .await
        .expect("call 1 must succeed");
    let r2 = server
        .compare_viewports(args())
        .await
        .expect("call 2 must succeed");
    let r3 = server
        .compare_viewports(args())
        .await
        .expect("call 3 must succeed");
    let s1 = serde_json::to_string(&r1.structured_content).expect("serialize 1");
    let s2 = serde_json::to_string(&r2.structured_content).expect("serialize 2");
    let s3 = serde_json::to_string(&r3.structured_content).expect("serialize 3");
    assert_eq!(s1, s2, "compare_viewports must be deterministic");
    assert_eq!(s2, s3, "compare_viewports must be deterministic");
}
