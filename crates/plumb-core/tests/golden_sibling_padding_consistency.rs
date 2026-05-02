//! Golden snapshot for the `sibling/padding-consistency` rule.
//!
//! Three sibling divs under a parent: two share padding, one drifts
//! by more than the 4px threshold.

use indexmap::IndexMap;
use plumb_core::report::Rect;
use plumb_core::snapshot::SnapshotNode;
use plumb_core::{Config, PlumbSnapshot, ViewportKey, run};

fn fixture_snapshot() -> PlumbSnapshot {
    let parent = SnapshotNode {
        dom_order: 2,
        selector: "html > body > div.cards".into(),
        tag: "div".into(),
        attrs: IndexMap::from_iter([("class".into(), "cards".into())]),
        computed_styles: IndexMap::new(),
        rect: Some(Rect {
            x: 0,
            y: 0,
            width: 800,
            height: 200,
        }),
        parent: Some(1),
        children: vec![3, 4, 5],
    };
    let child_a = node(
        3,
        2,
        "html > body > div.cards > div:nth-child(1)",
        &[
            ("padding-top", "16px"),
            ("padding-right", "16px"),
            ("padding-bottom", "16px"),
            ("padding-left", "16px"),
        ],
    );
    let child_b = node(
        4,
        2,
        "html > body > div.cards > div:nth-child(2)",
        &[
            ("padding-top", "16px"),
            ("padding-right", "16px"),
            ("padding-bottom", "16px"),
            ("padding-left", "16px"),
        ],
    );
    // Drifts: padding-top 28px vs median 16px = 12px drift
    let child_c = node(
        5,
        2,
        "html > body > div.cards > div:nth-child(3)",
        &[
            ("padding-top", "28px"),
            ("padding-right", "16px"),
            ("padding-bottom", "16px"),
            ("padding-left", "16px"),
        ],
    );

    PlumbSnapshot {
        url: "plumb-fake://sibling-padding".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![root_html(), body_node(), parent, child_a, child_b, child_c],
    }
}

fn root_html() -> SnapshotNode {
    SnapshotNode {
        dom_order: 0,
        selector: "html".into(),
        tag: "html".into(),
        attrs: IndexMap::new(),
        computed_styles: IndexMap::new(),
        rect: Some(Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 800,
        }),
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
        rect: Some(Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 800,
        }),
        parent: Some(0),
        children: vec![2],
    }
}

fn node(dom_order: u64, parent: u64, selector: &str, styles: &[(&str, &str)]) -> SnapshotNode {
    let mut computed_styles = IndexMap::new();
    for (prop, value) in styles {
        computed_styles.insert((*prop).to_owned(), (*value).to_owned());
    }
    SnapshotNode {
        dom_order,
        selector: selector.to_owned(),
        tag: "div".into(),
        attrs: IndexMap::new(),
        computed_styles,
        rect: Some(Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 100,
        }),
        parent: Some(parent),
        children: Vec::new(),
    }
}

#[test]
fn sibling_padding_consistency_golden() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = Config::default();
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "sibling/padding-consistency")
        .collect();
    let json = serde_json::to_string_pretty(&violations)?;
    insta::assert_snapshot!("sibling_padding_consistency", json);
    Ok(())
}

#[test]
fn sibling_padding_consistency_run_is_deterministic() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = Config::default();
    let a = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let b = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let c = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    assert_eq!(a, b);
    assert_eq!(b, c);
    Ok(())
}
