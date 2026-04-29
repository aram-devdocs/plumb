//! End-to-end protocol test: spawn `plumb mcp` as a subprocess and speak
//! JSON-RPC 2.0 over stdio, the way a real MCP client does.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::missing_panics_doc)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

fn bin() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("plumb")
}

fn send_and_read(requests: Vec<Value>) -> Vec<Value> {
    let mut child = Command::new(bin())
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn plumb mcp");

    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");

    // Count responses we expect: skip JSON-RPC notifications (no `id`).
    let expected = requests.iter().filter(|r| r.get("id").is_some()).count();

    std::thread::spawn(move || {
        let mut stdin = stdin;
        for req in requests {
            let bytes = serde_json::to_vec(&req).expect("serialize");
            stdin.write_all(&bytes).expect("write");
            stdin.write_all(b"\n").expect("newline");
        }
        stdin.flush().expect("flush");
        // Hold stdin open until the server has had time to process + flush
        // the final response. 1 s is generous on local + CI.
        std::thread::sleep(Duration::from_secs(1));
    });

    let mut reader = BufReader::new(stdout);
    let mut responses = Vec::with_capacity(expected);
    let mut line = String::new();
    while responses.len() < expected {
        line.clear();
        let n = reader.read_line(&mut line).expect("read");
        if n == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
            // log lines from tracing are skipped silently
            responses.push(v);
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    responses
}

fn init_request(id: u32) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "plumb-test", "version": "0.0.0" }
        }
    })
}

fn initialized_notification() -> Value {
    // Notifications have no `id` — server must not respond.
    json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    })
}

fn lint_url_request(id: u32, url: &str, detail: Option<&str>) -> Value {
    let mut arguments = json!({ "url": url });
    if let Some(detail) = detail {
        arguments["detail"] = Value::String(detail.to_owned());
    }

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": "lint_url", "arguments": arguments }
    })
}

#[test]
fn mcp_initialize_and_tools_list() {
    let tools_list = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    });
    let responses = send_and_read(vec![
        init_request(1),
        initialized_notification(),
        tools_list,
    ]);
    assert!(!responses.is_empty(), "expected responses, got none");

    let tools_resp = responses
        .iter()
        .find(|r| r["id"] == 2)
        .unwrap_or_else(|| panic!("tools/list response missing: got {responses:?}"));
    let tools = tools_resp["result"]["tools"]
        .as_array()
        .expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"echo"));
    assert!(names.contains(&"lint_url"));
    assert!(names.contains(&"explain_rule"));
    assert!(names.contains(&"list_rules"));
    assert!(names.contains(&"get_config"));

    let echo = tools
        .iter()
        .find(|tool| tool["name"] == "echo")
        .unwrap_or_else(|| panic!("echo tool missing: got {tools:?}"));
    assert_eq!(
        echo["description"],
        "Echo a message — smoke test the MCP transport."
    );
    assert_eq!(
        echo["inputSchema"]["properties"]["message"]["type"],
        "string"
    );

    let lint_url = tools
        .iter()
        .find(|tool| tool["name"] == "lint_url")
        .unwrap_or_else(|| panic!("lint_url tool missing: got {tools:?}"));
    assert_eq!(
        lint_url["description"],
        "Lint a URL with Plumb. Accepts http(s):// and plumb-fake:// URLs."
    );
    assert_eq!(
        lint_url["inputSchema"]["properties"]["url"]["type"],
        "string"
    );
    assert!(
        lint_url["inputSchema"]["properties"]["detail"].is_object(),
        "lint_url detail property missing from schema: {lint_url:?}"
    );
    assert_eq!(
        lint_url["inputSchema"]["required"].as_array(),
        Some(&vec![Value::String("url".to_owned())]),
        "lint_url must require only url: {lint_url:?}"
    );
    let detail_variants: Vec<&str> = lint_url["inputSchema"]["$defs"]["LintUrlDetail"]["oneOf"]
        .as_array()
        .expect("detail oneOf variants")
        .iter()
        .map(|variant| variant["const"].as_str().expect("detail variant const"))
        .collect();
    assert_eq!(
        detail_variants,
        vec!["compact", "full"],
        "lint_url detail enum must expose compact/full: {lint_url:?}"
    );

    let get_config = tools
        .iter()
        .find(|tool| tool["name"] == "get_config")
        .unwrap_or_else(|| panic!("get_config tool missing: got {tools:?}"));
    assert_eq!(
        get_config["description"],
        "Return the resolved plumb.toml for a working directory as JSON. Memoized per (path, mtime)."
    );
    assert_eq!(
        get_config["inputSchema"]["properties"]["working_dir"]["type"],
        "string"
    );
}

#[test]
fn mcp_echo_round_trip() {
    let echo = json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "echo", "arguments": { "message": "hi plumb" } }
    });
    let responses = send_and_read(vec![init_request(1), initialized_notification(), echo]);
    let echo_resp = responses
        .iter()
        .find(|r| r["id"] == 2)
        .unwrap_or_else(|| panic!("echo response missing: got {responses:?}"));
    let text = echo_resp["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    assert!(text.contains("hi plumb"), "unexpected text: {text}");
}

#[test]
fn mcp_lint_url_returns_structured_content() {
    let lint_url = lint_url_request(2, "plumb-fake://hello", None);
    let responses = send_and_read(vec![init_request(1), initialized_notification(), lint_url]);
    let lint_resp = responses
        .iter()
        .find(|r| r["id"] == 2)
        .unwrap_or_else(|| panic!("lint_url response missing: got {responses:?}"));
    let result = &lint_resp["result"];

    assert_eq!(result["isError"].as_bool(), Some(false));

    let content = result["content"].as_array().expect("content array");
    assert_eq!(
        content.len(),
        1,
        "lint_url must not return structured payload as extra text: {result}"
    );
    assert_eq!(content[0]["type"].as_str(), Some("text"));
    let text = content[0]["text"].as_str().expect("text content");
    assert!(
        text.contains("warning spacing/grid-conformance @ html > body [desktop]"),
        "unexpected lint_url text: {text}"
    );

    let structured = result["structuredContent"]
        .as_object()
        .expect("structuredContent object");
    assert_eq!(structured["counts"]["total"].as_u64(), Some(1));
    assert_eq!(
        structured["violations"][0]["rule_id"].as_str(),
        Some("spacing/grid-conformance")
    );
}

#[test]
fn mcp_lint_url_explicit_compact_matches_default() {
    let responses = send_and_read(vec![
        init_request(1),
        initialized_notification(),
        lint_url_request(2, "plumb-fake://hello", None),
        lint_url_request(3, "plumb-fake://hello", Some("compact")),
    ]);
    let default_resp = responses
        .iter()
        .find(|r| r["id"] == 2)
        .unwrap_or_else(|| panic!("default lint_url response missing: got {responses:?}"));
    let compact_resp = responses
        .iter()
        .find(|r| r["id"] == 3)
        .unwrap_or_else(|| panic!("compact lint_url response missing: got {responses:?}"));

    assert_eq!(default_resp["result"], compact_resp["result"]);
}

#[test]
fn mcp_lint_url_full_returns_json_envelope() {
    let responses = send_and_read(vec![
        init_request(1),
        initialized_notification(),
        lint_url_request(2, "plumb-fake://hello", Some("full")),
    ]);
    let lint_resp = responses
        .iter()
        .find(|r| r["id"] == 2)
        .unwrap_or_else(|| panic!("full lint_url response missing: got {responses:?}"));
    let result = &lint_resp["result"];

    assert_eq!(result["isError"].as_bool(), Some(false));
    assert!(
        result["content"][0]["text"]
            .as_str()
            .expect("text content")
            .contains("warning spacing/grid-conformance @ html > body [desktop]")
    );

    let structured = result["structuredContent"]
        .as_object()
        .expect("structuredContent object");
    assert_eq!(
        structured["plumb_version"].as_str(),
        Some(env!("CARGO_PKG_VERSION"))
    );
    assert!(
        structured["run_id"]
            .as_str()
            .expect("run_id string")
            .starts_with("sha256:")
    );
    assert_eq!(structured["summary"]["total"].as_u64(), Some(1));
    assert_eq!(
        structured["violations"][0]["doc_url"].as_str(),
        Some("https://plumb.aramhammoudeh.com/rules/spacing-grid-conformance")
    );
}

#[test]
fn mcp_get_config_returns_default_when_no_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let working_dir = tmp.path().to_string_lossy().into_owned();

    let get_config = json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "get_config", "arguments": { "working_dir": working_dir } }
    });
    let responses = send_and_read(vec![
        init_request(1),
        initialized_notification(),
        get_config,
    ]);
    let resp = responses
        .iter()
        .find(|r| r["id"] == 2)
        .unwrap_or_else(|| panic!("get_config response missing: got {responses:?}"));
    let result = &resp["result"];
    assert_eq!(result["isError"].as_bool(), Some(false));

    let text = result["content"][0]["text"].as_str().expect("text content");
    assert!(text.contains("no plumb.toml"), "unexpected text: {text}");

    let structured = result["structuredContent"]
        .as_object()
        .expect("structuredContent object");
    assert_eq!(structured["source"].as_str(), Some("default"));
    assert!(
        structured["config"].is_object(),
        "config field must be an object: {structured:?}"
    );
    assert!(
        structured["config"]["viewports"].is_object(),
        "default config must include viewports map: {structured:?}"
    );
}

#[test]
fn mcp_list_rules_returns_every_rule() {
    let list_rules = json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "list_rules", "arguments": {} }
    });
    let responses = send_and_read(vec![
        init_request(1),
        initialized_notification(),
        list_rules,
    ]);
    let resp = responses
        .iter()
        .find(|r| r["id"] == 2)
        .unwrap_or_else(|| panic!("list_rules response missing: got {responses:?}"));
    let result = &resp["result"];

    assert_eq!(result["isError"].as_bool(), Some(false));

    let structured = result["structuredContent"]
        .as_object()
        .expect("structuredContent object");
    let count = structured["count"].as_u64().expect("count must be a u64");
    assert!(count > 0, "list_rules must return at least one rule");
    let rules = structured["rules"]
        .as_array()
        .expect("rules must be an array");
    assert_eq!(rules.len() as u64, count);
    let first_id = rules[0]["id"]
        .as_str()
        .expect("first rule must carry an id string");
    assert!(!first_id.is_empty(), "rule id must not be empty");
}
