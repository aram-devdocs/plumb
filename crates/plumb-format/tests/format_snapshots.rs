//! Snapshot tests for every formatter against the canned walking-skeleton
//! violation set.

use indexmap::IndexMap;
use plumb_core::{
    Config, PlumbSnapshot, Severity, ViewportKey, Violation, builtin_rule_metadata, run,
};
use plumb_format::{json, mcp_compact, pretty, sarif_with_rules};

fn fixture() -> Vec<plumb_core::Violation> {
    let snapshot = PlumbSnapshot::canned();
    let config = Config::default();
    run(&snapshot, &config)
}

fn grouped_fixture() -> Vec<Violation> {
    vec![
        Violation {
            rule_id: "spacing/grid-conformance".to_owned(),
            severity: Severity::Warning,
            message: "Hero spacing drifts off the spacing grid.".to_owned(),
            selector: "main > section.hero".to_owned(),
            viewport: ViewportKey::new("mobile"),
            rect: None,
            dom_order: 8,
            fix: None,
            doc_url: "https://plumb.aramhammoudeh.com/rules/spacing-grid-conformance".to_owned(),
            metadata: IndexMap::new(),
        },
        Violation {
            rule_id: "a11y/touch-target".to_owned(),
            severity: Severity::Error,
            message: "CTA touch target is smaller than the minimum size.".to_owned(),
            selector: "button.cta".to_owned(),
            viewport: ViewportKey::new("desktop"),
            rect: None,
            dom_order: 3,
            fix: None,
            doc_url: "https://plumb.aramhammoudeh.com/rules/a11y-touch-target".to_owned(),
            metadata: IndexMap::new(),
        },
        Violation {
            rule_id: "spacing/grid-conformance".to_owned(),
            severity: Severity::Info,
            message: "Nav spacing is close to the grid but still non-canonical.".to_owned(),
            selector: "nav.primary > a".to_owned(),
            viewport: ViewportKey::new("desktop"),
            rect: None,
            dom_order: 2,
            fix: None,
            doc_url: "https://plumb.aramhammoudeh.com/rules/spacing-grid-conformance".to_owned(),
            metadata: IndexMap::new(),
        },
        Violation {
            rule_id: "spacing/grid-conformance".to_owned(),
            severity: Severity::Warning,
            message: "Nav container gap is off-grid.".to_owned(),
            selector: "nav.primary".to_owned(),
            viewport: ViewportKey::new("desktop"),
            rect: None,
            dom_order: 1,
            fix: None,
            doc_url: "https://plumb.aramhammoudeh.com/rules/spacing-grid-conformance".to_owned(),
            metadata: IndexMap::new(),
        },
    ]
}

#[test]
fn pretty_snapshot() {
    insta::assert_snapshot!("pretty", pretty(&fixture()));
}

#[test]
fn pretty_grouped_snapshot() {
    insta::assert_snapshot!("pretty_grouped", pretty(&grouped_fixture()));
}

#[test]
fn json_snapshot() {
    let out = json(&fixture()).expect("json serialize");
    insta::assert_snapshot!("json", out);
}

#[test]
fn json_stats_snapshot() {
    let out = json(&grouped_fixture()).expect("json serialize grouped");
    insta::assert_snapshot!("json_stats", out);
}

#[test]
fn sarif_snapshot() {
    let out = sarif_with_rules(&fixture(), &builtin_rule_metadata()).expect("sarif serialize");
    insta::assert_snapshot!("sarif", out);
}

#[test]
fn mcp_compact_snapshot() {
    let (text, structured) = mcp_compact(&fixture());
    insta::assert_snapshot!("mcp_compact_text", text);
    insta::assert_json_snapshot!("mcp_compact_structured", structured);
}
