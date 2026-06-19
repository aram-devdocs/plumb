//! Golden snapshot for the `sibling/height-consistency` rule.
//!
//! Two parents under `<html> > <body>`:
//!
//! - A row of three `<div>` cards spaced 220px apart. They are
//!   non-interactive, so the interactivity gate (PRD §6/§11.3) drops
//!   them before grouping — proving the rule no longer flags generic
//!   layout boxes. The unit test
//!   `cluster_groups_siblings_with_close_tops` still covers the
//!   row-clustering path directly.
//! - A vertical stack of three `<button>`s (no row pairs) — interactive
//!   peers that the rule keeps. The third is 16px taller than the row
//!   median, so it is the only violation.

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
    // Row container at dom_order=2, three card siblings at 3..=5.
    // Card heights 100, 100, 130 — last one drifts by 30px. The cards
    // are plain `<div>`s, so the interactivity gate (PRD §6/§11.3)
    // correctly drops them: height-consistency only judges interactive
    // button-like peers, not generic layout boxes.
    let row_card_a = node(
        3,
        2,
        "html > body > div.row > div:nth-child(1)",
        "div",
        rect(0, 0, 200, 100),
    );
    let row_card_b = node(
        4,
        2,
        "html > body > div.row > div:nth-child(2)",
        "div",
        rect(220, 0, 200, 100),
    );
    let row_card_c = node(
        5,
        2,
        "html > body > div.row > div:nth-child(3)",
        "div",
        rect(440, 1, 200, 130),
    );
    let row_container = SnapshotNode {
        dom_order: 2,
        selector: "html > body > div.row".into(),
        tag: "div".into(),
        attrs: IndexMap::from_iter([("class".into(), "row".into())]),
        computed_styles: IndexMap::new(),
        rect: Some(rect(0, 0, 800, 130)),
        parent: Some(1),
        children: vec![3, 4, 5],
    };

    // Stacked buttons at dom_order 7, 8, 9 — no row pairs, so the
    // fallback fires. Heights 32, 32, 48 — the third is 16px taller.
    // These are real `<button>`s, so the interactivity gate keeps them.
    let stack_btn_a = node(
        7,
        6,
        "html > body > div.stack > button:nth-child(1)",
        "button",
        rect(0, 200, 120, 32),
    );
    let stack_btn_b = node(
        8,
        6,
        "html > body > div.stack > button:nth-child(2)",
        "button",
        rect(0, 240, 120, 32),
    );
    let stack_btn_c = node(
        9,
        6,
        "html > body > div.stack > button:nth-child(3)",
        "button",
        rect(0, 280, 120, 48),
    );
    let stack_container = SnapshotNode {
        dom_order: 6,
        selector: "html > body > div.stack".into(),
        tag: "div".into(),
        attrs: IndexMap::from_iter([("class".into(), "stack".into())]),
        computed_styles: IndexMap::new(),
        rect: Some(rect(0, 200, 120, 130)),
        parent: Some(1),
        children: vec![7, 8, 9],
    };

    PlumbSnapshot {
        url: "plumb-fake://sibling-height-consistency".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![
            root_html(),
            body_node(),
            row_container,
            row_card_a,
            row_card_b,
            row_card_c,
            stack_container,
            stack_btn_a,
            stack_btn_b,
            stack_btn_c,
        ],
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
        children: vec![2, 6],
    }
}

fn node(dom_order: u64, parent: u64, selector: &str, tag: &str, rect_value: Rect) -> SnapshotNode {
    SnapshotNode {
        dom_order,
        selector: selector.to_owned(),
        tag: tag.to_owned(),
        attrs: IndexMap::new(),
        computed_styles: IndexMap::new(),
        rect: Some(rect_value),
        parent: Some(parent),
        children: Vec::new(),
    }
}

#[test]
fn sibling_height_consistency_golden() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = Config::default();
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "sibling/height-consistency")
        .collect();
    let json = serde_json::to_string_pretty(&violations)?;
    insta::assert_snapshot!("sibling_height_consistency", json);
    Ok(())
}

#[test]
fn sibling_height_consistency_run_is_deterministic() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = Config::default();
    let a = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let b = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let c = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    assert_eq!(a, b);
    assert_eq!(b, c);
    Ok(())
}
