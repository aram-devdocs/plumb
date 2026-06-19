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

fn visible_rect() -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: 200,
        height: 100,
    }
}

fn card_child(
    dom_order: u64,
    tag: &str,
    selector: &str,
    styles: &[(&str, &str)],
    rect: Option<Rect>,
) -> SnapshotNode {
    let mut computed_styles = IndexMap::new();
    for (prop, value) in styles {
        computed_styles.insert((*prop).to_owned(), (*value).to_owned());
    }
    SnapshotNode {
        dom_order,
        selector: selector.to_owned(),
        tag: tag.to_owned(),
        attrs: IndexMap::new(),
        computed_styles,
        rect,
        parent: Some(2),
        children: Vec::new(),
    }
}

/// Wrap a set of card children (each with `parent: Some(2)`) under a
/// single `div.cards` parent below `<body>`.
fn snapshot_with_card_children(children: Vec<SnapshotNode>) -> PlumbSnapshot {
    let child_orders: Vec<u64> = children.iter().map(|c| c.dom_order).collect();
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
        children: child_orders,
    };
    let mut nodes = vec![root_html(), body_node(), parent];
    nodes.extend(children);
    PlumbSnapshot {
        url: "plumb-fake://sibling-padding-guard".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes,
        text_boxes: Vec::new(),
    }
}

fn padding_violation_selectors(snapshot: &PlumbSnapshot) -> Vec<String> {
    run(snapshot, &Config::default())
        .into_iter()
        .filter(|v| v.rule_id == "sibling/padding-consistency")
        .map(|v| v.selector)
        .collect()
}

#[test]
fn sibling_padding_consistency_groups_by_parent_tag() {
    // Mixed-tag siblings are no longer compared: a `<p>` (16px) and a
    // `<section>` (0px) under one parent fall into separate
    // `(parent, tag)` groups, each with a single member, so neither
    // fires — even though their padding differs by 16px.
    let mixed = snapshot_with_card_children(vec![
        card_child(
            3,
            "p",
            "html > body > div.cards > p",
            &[
                ("padding-top", "16px"),
                ("padding-right", "16px"),
                ("padding-bottom", "16px"),
                ("padding-left", "16px"),
            ],
            Some(visible_rect()),
        ),
        card_child(
            4,
            "section",
            "html > body > div.cards > section",
            &[
                ("padding-top", "0"),
                ("padding-right", "0"),
                ("padding-bottom", "0"),
                ("padding-left", "0"),
            ],
            Some(visible_rect()),
        ),
    ]);
    let mixed_selectors = padding_violation_selectors(&mixed);
    assert!(
        mixed_selectors.is_empty(),
        "different-tag siblings must not be compared: {mixed_selectors:?}"
    );

    // Same-tag peers still compare: three `<div>`s at 16/16/4 px — the
    // 4px outlier drifts 12px past the median and fires; the 16px pair
    // stays silent.
    let same_tag = snapshot_with_card_children(vec![
        card_child(
            3,
            "div",
            "html > body > div.cards > div:nth-child(1)",
            &[
                ("padding-top", "16px"),
                ("padding-right", "16px"),
                ("padding-bottom", "16px"),
                ("padding-left", "16px"),
            ],
            Some(visible_rect()),
        ),
        card_child(
            4,
            "div",
            "html > body > div.cards > div:nth-child(2)",
            &[
                ("padding-top", "16px"),
                ("padding-right", "16px"),
                ("padding-bottom", "16px"),
                ("padding-left", "16px"),
            ],
            Some(visible_rect()),
        ),
        card_child(
            5,
            "div",
            "html > body > div.cards > div:nth-child(3)",
            &[
                ("padding-top", "4px"),
                ("padding-right", "4px"),
                ("padding-bottom", "4px"),
                ("padding-left", "4px"),
            ],
            Some(visible_rect()),
        ),
    ]);
    let same_selectors = padding_violation_selectors(&same_tag);
    assert!(
        same_selectors.contains(&"html > body > div.cards > div:nth-child(3)".to_owned()),
        "the 4px outlier must fire: {same_selectors:?}"
    );
    assert!(
        !same_selectors.contains(&"html > body > div.cards > div:nth-child(1)".to_owned())
            && !same_selectors.contains(&"html > body > div.cards > div:nth-child(2)".to_owned()),
        "the 16px siblings must stay silent: {same_selectors:?}"
    );
}

#[test]
fn sibling_padding_consistency_skips_invisible_siblings() {
    // Four same-tag `<div>` peers. Three are visible (padding-top
    // 4/16/16); one is invisible (`rect: None`) with padding-top 4.
    //
    // Visible-only, the median is 16, so the lone 4px div drifts 12px
    // and fires while the 16px pair stays silent. If the invisible 4px
    // node were counted, the median would drop to 4 and the two 16px
    // divs would fire instead — so the outcome below proves the invisible
    // node neither shifts the median nor fires.
    let snapshot = snapshot_with_card_children(vec![
        card_child(
            3,
            "div",
            "html > body > div.cards > div:nth-child(1)",
            &[("padding-top", "4px")],
            Some(visible_rect()),
        ),
        card_child(
            4,
            "div",
            "html > body > div.cards > div:nth-child(2)",
            &[("padding-top", "16px")],
            Some(visible_rect()),
        ),
        card_child(
            5,
            "div",
            "html > body > div.cards > div:nth-child(3)",
            &[("padding-top", "16px")],
            Some(visible_rect()),
        ),
        card_child(
            6,
            "div",
            "html > body > div.cards > div.hidden",
            &[("padding-top", "4px")],
            None,
        ),
    ]);
    let selectors = padding_violation_selectors(&snapshot);
    assert_eq!(
        selectors,
        vec!["html > body > div.cards > div:nth-child(1)".to_owned()],
        "only the visible 4px outlier fires; the invisible sibling is skipped"
    );
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
