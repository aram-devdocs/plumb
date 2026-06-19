//! Golden snapshot for the `edge/near-alignment` rule.
//!
//! Three sibling cards under `<html> > <body>` with left edges at
//! `x = 0, 1, 2`. Default `alignment.tolerance_px = 3` clusters them
//! all together; the centroid is `1`. Cards 0 and 2 are 1px off the
//! centroid → both flagged. Card 1 sits on the centroid → silent
//! (delta = 0).
//!
//! Other axes are tuned so they don't fire (right edges separated by
//! a clear gap, top edges identical, bottom edges identical).

use indexmap::IndexMap;
use plumb_core::report::Rect;
use plumb_core::snapshot::SnapshotNode;
use plumb_core::{Config, PlumbSnapshot, ViewportKey, run};

const fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn fixture_snapshot() -> PlumbSnapshot {
    // Card widths: 100, 99, 98 — right edges at 100, 100, 100 (perfect
    // alignment, silent). Left edges drift across the cluster.
    let card_a = node(2, "html > body > div:nth-child(1)", rect(0, 50, 100, 80));
    let card_b = node(3, "html > body > div:nth-child(2)", rect(1, 200, 99, 80));
    let card_c = node(4, "html > body > div:nth-child(3)", rect(2, 350, 98, 80));

    PlumbSnapshot {
        url: "plumb-fake://edge-near-alignment".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![root_html(), body_node(), card_a, card_b, card_c],
        text_boxes: Vec::new(),
    }
}

fn root_html() -> SnapshotNode {
    SnapshotNode {
        dom_order: 0,
        selector: "html".into(),
        tag: "html".into(),
        attrs: IndexMap::new(),
        computed_styles: IndexMap::new(),
        rect: Some(rect(0, 0, 1280, 800)),
        parent: None,
        children: vec![1],
    }
}

fn body_node() -> SnapshotNode {
    SnapshotNode {
        dom_order: 1,
        selector: "html > body".into(),
        tag: "body".into(),
        attrs: IndexMap::new(),
        computed_styles: IndexMap::new(),
        rect: Some(rect(0, 0, 1280, 800)),
        parent: Some(0),
        children: vec![2, 3, 4],
    }
}

fn node(dom_order: u64, selector: &str, rect_value: Rect) -> SnapshotNode {
    SnapshotNode {
        dom_order,
        selector: selector.to_owned(),
        tag: "div".into(),
        attrs: IndexMap::new(),
        computed_styles: IndexMap::new(),
        rect: Some(rect_value),
        parent: Some(1),
        children: Vec::new(),
    }
}

#[test]
fn edge_near_alignment_golden() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = Config::default();
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "edge/near-alignment")
        .collect();
    let json = serde_json::to_string_pretty(&violations)?;
    insta::assert_snapshot!("edge_near_alignment", json);
    Ok(())
}

fn tagged_node(dom_order: u64, selector: &str, tag: &str, rect_value: Rect) -> SnapshotNode {
    SnapshotNode {
        dom_order,
        selector: selector.to_owned(),
        tag: tag.to_owned(),
        attrs: IndexMap::new(),
        computed_styles: IndexMap::new(),
        rect: Some(rect_value),
        parent: Some(1),
        children: Vec::new(),
    }
}

/// Count `edge/near-alignment` violations for two same-tag siblings with
/// the given rects. Lets one test vary only the tag (layout vs SVG
/// primitive) or the geometry (positive-area vs zero-area).
fn near_alignment_violation_count(tag: &str, rect_a: Rect, rect_b: Rect) -> usize {
    let child_a = tagged_node(2, "html > body > *:nth-child(1)", tag, rect_a);
    let child_b = tagged_node(3, "html > body > *:nth-child(2)", tag, rect_b);
    let snapshot = PlumbSnapshot {
        url: "plumb-fake://edge-near-alignment-guard".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![root_html(), body_node(), child_a, child_b],
        text_boxes: Vec::new(),
    };
    run(&snapshot, &Config::default())
        .into_iter()
        .filter(|v| v.rule_id == "edge/near-alignment")
        .count()
}

#[test]
fn edge_near_alignment_skips_non_layout_children() {
    // Left edges at x = 0 and x = 2 cluster within the default 3px
    // tolerance (centroid 1, each 1px off). As <div> siblings they fire;
    // the identical geometry on <path> SVG primitives is non-layout and
    // MUST be skipped.
    let rect_a = rect(0, 50, 100, 80);
    let rect_b = rect(2, 50, 100, 80);
    assert!(
        near_alignment_violation_count("div", rect_a, rect_b) > 0,
        "near-aligned <div> siblings must fire"
    );
    assert_eq!(
        near_alignment_violation_count("path", rect_a, rect_b),
        0,
        "near-aligned <path> primitives must be skipped as non-layout"
    );
}

#[test]
fn edge_near_alignment_skips_zero_area_boxes() {
    // A zero-width pair (`<br>`-style collapsed boxes) paints nothing,
    // so its edges must never form an alignment cluster — even though
    // the same near-aligned <div> geometry fires.
    let solid_a = rect(0, 50, 100, 80);
    let solid_b = rect(2, 50, 100, 80);
    assert!(
        near_alignment_violation_count("div", solid_a, solid_b) > 0,
        "positive-area <div> siblings must fire"
    );
    let zero_a = rect(0, 50, 0, 80);
    let zero_b = rect(2, 50, 0, 80);
    assert_eq!(
        near_alignment_violation_count("br", zero_a, zero_b),
        0,
        "zero-area <br> pair must be skipped"
    );
}

#[test]
fn edge_near_alignment_run_is_deterministic() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = Config::default();
    let a = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let b = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let c = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    assert_eq!(a, b);
    assert_eq!(b, c);
    Ok(())
}
