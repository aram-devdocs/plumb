//! Snapshot tests for every formatter against the canned walking-skeleton
//! violation set.

use plumb_core::{Config, PlumbSnapshot, run};
use plumb_format::{json, mcp_compact, pretty, sarif};

fn fixture() -> Vec<plumb_core::Violation> {
    let snapshot = PlumbSnapshot::canned();
    let config = Config::default();
    run(&snapshot, &config)
}

#[test]
fn pretty_snapshot() {
    insta::assert_snapshot!("pretty", pretty(&fixture()));
}

#[test]
fn json_snapshot() {
    let out = json(&fixture()).expect("json serialize");
    insta::assert_snapshot!("json", out);
}

#[test]
fn sarif_snapshot() {
    let out = sarif(&fixture()).expect("sarif serialize");
    insta::assert_snapshot!("sarif", out);
}

#[test]
fn mcp_compact_snapshot() {
    let (text, structured) = mcp_compact(&fixture());
    insta::assert_snapshot!("mcp_compact_text", text);
    insta::assert_json_snapshot!("mcp_compact_structured", structured);
}
