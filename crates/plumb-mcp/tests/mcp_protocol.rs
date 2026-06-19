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
    CompareViewport, CompareViewportsArgs, EchoArgs, ExplainRuleArgs, LintPageHtmlArgs,
    LintUrlArgs, LintUrlDetail, PlumbServer, documented_rule_ids,
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

/// `echo` is the transport smoke test, but it still has to obey the
/// `mcp-tool-patterns.md` contract: every tool response carries
/// **both** a `content` block (human summary) and `structuredContent`
/// (machine payload). Older versions returned only the text content,
/// which forced tool-using agents to re-parse the text — exactly what
/// the contract is designed to avoid.
#[tokio::test]
async fn echo_returns_both_content_and_structured_content() {
    let server = server();
    let result = server
        .echo(EchoArgs {
            message: "ping".to_owned(),
        })
        .await
        .expect("echo must succeed for any string");

    // `content[0]` carries the human-readable echo.
    let text = result
        .content
        .iter()
        .find_map(|content| content.as_text().map(|text| text.text.clone()))
        .expect("echo response must include a text content block");
    assert_eq!(text, "ping", "text block must echo the input verbatim");

    // `structured_content` carries the machine-readable echo. The
    // `echoed` key is the contract a tool-using agent reads.
    let structured = result
        .structured_content
        .expect("echo response must include structured_content");
    assert_eq!(
        structured.get("echoed").and_then(|v| v.as_str()),
        Some("ping"),
        "structuredContent.echoed must equal the echoed message",
    );
}

/// Regression guard: an empty input string still produces a
/// well-formed dual response. A future input gate that rejects empty
/// messages would break the smoke-test promise.
#[tokio::test]
async fn echo_handles_empty_message_with_structured_block() {
    let server = server();
    let result = server
        .echo(EchoArgs {
            message: String::new(),
        })
        .await
        .expect("echo must accept any UTF-8 string, including empty");

    assert!(
        !result.content.is_empty(),
        "echo response must include at least one content block"
    );
    let structured = result
        .structured_content
        .expect("echo response must include structured_content for empty input too");
    assert_eq!(
        structured.get("echoed").and_then(|v| v.as_str()),
        Some(""),
        "structuredContent.echoed must round-trip the empty string",
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
            working_dir: None,
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
        structured.get("findings").is_some()
            && structured.get("counts").is_some()
            && structured.get("by_rule").is_some()
            && structured.get("truncated").is_some(),
        "structured payload follows the aggregated mcp_compact shape (findings + counts + by_rule + truncated)"
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
                working_dir: None,
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

/// Self-contained defect HTML — an embedded `<style>` setting off-grid
/// padding — must never produce a false "clean" result. The static
/// parser this tool used to call found nothing, so it always returned
/// zero violations on defect-filled HTML; now the document is rendered
/// in Chromium.
///
/// This test is environment-robust by design: with Chromium present the
/// render produces non-empty `findings`; without it the tool returns a
/// clear `isError = true` driver error. The one outcome it forbids is a
/// successful response with zero findings.
#[tokio::test]
async fn lint_page_html_defect_html_never_returns_false_clean() {
    let server = server();
    let html = r"<!doctype html><html><head><style>div { padding: 13px }</style></head><body><div>x</div></body></html>";
    let result = server
        .lint_page_html(LintPageHtmlArgs {
            html: html.to_owned(),
            base_url: "https://example.com/".to_owned(),
            working_dir: None,
        })
        .await
        .expect("lint_page_html must not raise a JSON-RPC error for valid input");

    if result.is_error.unwrap_or(false) {
        // Chromium unavailable (or another driver failure): the response
        // must be a clear error, not a misleading clean.
        let text = result
            .content
            .iter()
            .find_map(|content| content.as_text().map(|text| text.text.clone()))
            .expect("error response must include a text content block");
        assert!(
            text.contains("lint_page_html failed"),
            "driver failure must be explicit, got: {text}"
        );
    } else {
        let structured = result
            .structured_content
            .expect("successful response must include structured_content");
        let findings = structured
            .get("findings")
            .and_then(serde_json::Value::as_array)
            .expect("structuredContent.findings must be an array");
        assert!(
            !findings.is_empty(),
            "rendered defect HTML must produce at least one finding, not a false clean: {structured}",
        );
    }

    server.shutdown().await.expect("shutdown must succeed");
}

/// With Chromium present, the embedded `<style>` off-grid padding renders
/// into real computed styles and surfaces a `spacing/grid-conformance`
/// finding. Gated behind `e2e-chromium`, mirroring the plumb-cdp suite;
/// on a host where the launch fails the test skips rather than failing.
#[cfg(feature = "e2e-chromium")]
#[tokio::test]
async fn lint_page_html_renders_embedded_style_into_grid_finding() {
    let server = server();
    let html = r"<!doctype html><html><head><style>div { padding: 13px }</style></head><body><div>x</div></body></html>";
    let result = server
        .lint_page_html(LintPageHtmlArgs {
            html: html.to_owned(),
            base_url: "https://example.com/".to_owned(),
            working_dir: None,
        })
        .await
        .expect("lint_page_html must not raise a JSON-RPC error for valid input");

    if result.is_error.unwrap_or(false) {
        // No usable Chromium on this host — skip, same as the cdp e2e suite.
        return;
    }

    let structured = result
        .structured_content
        .expect("successful response must include structured_content");
    let by_rule = structured
        .get("by_rule")
        .and_then(serde_json::Value::as_object)
        .expect("structuredContent.by_rule must be an object");
    assert!(
        by_rule.contains_key("spacing/grid-conformance"),
        "embedded off-grid padding must render into a grid-conformance finding: {structured}",
    );

    server.shutdown().await.expect("shutdown must succeed");
}

/// `lint_page_html` rejects empty `html` with a JSON-RPC `invalid_params`
/// error so the agent gets a clear contract failure rather than a
/// silently-empty snapshot. Validated before any rendering, so it runs
/// without Chromium.
#[tokio::test]
async fn lint_page_html_empty_html_returns_invalid_params() {
    let server = server();
    let err = server
        .lint_page_html(LintPageHtmlArgs {
            html: String::new(),
            base_url: "https://example.com/".to_owned(),
            working_dir: None,
        })
        .await
        .expect_err("empty html must fail");
    assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
}

/// `lint_page_html` rejects oversized HTML at the 1 MiB input cap. The
/// cap is validated before rendering and surfaces as `invalid_params`,
/// so a chatty agent can react without warming Chromium.
#[tokio::test]
async fn lint_page_html_above_byte_cap_returns_invalid_params() {
    let server = server();
    let big_body = "x".repeat(1024 * 1024 + 1);
    let html = format!("<!doctype html><html><body><p>{big_body}</p></body></html>");
    let err = server
        .lint_page_html(LintPageHtmlArgs {
            html,
            base_url: "https://example.com/".to_owned(),
            working_dir: None,
        })
        .await
        .expect_err("oversized html must fail");
    assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
}

/// `lint_page_html` rejects HTML past the 10 000-element cap before
/// rendering, mirroring the byte-cap guard. Built from many empty
/// elements so the element estimate — not the byte count — trips first.
#[tokio::test]
async fn lint_page_html_above_element_cap_returns_invalid_params() {
    let server = server();
    let mut html = String::from("<!doctype html><html><body>");
    for _ in 0..10_001 {
        html.push_str("<i></i>");
    }
    html.push_str("</body></html>");
    let err = server
        .lint_page_html(LintPageHtmlArgs {
            html,
            base_url: "https://example.com/".to_owned(),
            working_dir: None,
        })
        .await
        .expect_err("over-element-cap html must fail");
    assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
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
