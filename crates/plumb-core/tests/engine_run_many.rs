//! Multi-snapshot orchestration: `run_many` aggregates per-viewport
//! snapshots into one deterministic, sorted, deduped violation list.

use plumb_core::{Config, PlumbSnapshot, ViewportKey, Violation, run_many};

fn snapshot_for(viewport: &str, width: u32, height: u32) -> PlumbSnapshot {
    let mut snap = PlumbSnapshot::canned();
    snap.viewport = ViewportKey::new(viewport);
    snap.viewport_width = width;
    snap.viewport_height = height;
    snap
}

fn keys(violations: &[Violation]) -> Vec<(&str, &str, &str, u64)> {
    violations.iter().map(Violation::sort_key).collect()
}

#[test]
fn run_many_collects_violations_from_every_snapshot() {
    let mobile = snapshot_for("mobile", 375, 812);
    let desktop = snapshot_for("desktop", 1280, 800);
    let config = Config::default();

    let violations = run_many([&mobile, &desktop], &config);

    // Placeholder rule emits exactly one violation per snapshot — we
    // expect both viewports represented after sort + dedup.
    assert_eq!(violations.len(), 2);

    let viewports: Vec<&str> = violations.iter().map(|v| v.viewport.as_str()).collect();
    assert!(viewports.contains(&"mobile"));
    assert!(viewports.contains(&"desktop"));
}

#[test]
fn run_many_sorts_by_rule_id_viewport_selector_dom_order() {
    let mobile = snapshot_for("mobile", 375, 812);
    let desktop = snapshot_for("desktop", 1280, 800);
    let config = Config::default();

    let violations = run_many([&mobile, &desktop], &config);

    // Sort key is (rule_id, viewport, selector, dom_order). Both
    // violations share rule_id and selector, so viewport breaks the
    // tie alphabetically: `desktop` < `mobile`.
    assert_eq!(
        keys(&violations),
        vec![
            ("placeholder/hello-world", "desktop", "html > body", 2_u64,),
            ("placeholder/hello-world", "mobile", "html > body", 2_u64,),
        ],
    );
}

#[test]
fn run_many_is_input_order_independent() {
    let mobile = snapshot_for("mobile", 375, 812);
    let desktop = snapshot_for("desktop", 1280, 800);
    let config = Config::default();

    let forward = run_many([&mobile, &desktop], &config);
    let reverse = run_many([&desktop, &mobile], &config);

    assert_eq!(forward, reverse);
}
