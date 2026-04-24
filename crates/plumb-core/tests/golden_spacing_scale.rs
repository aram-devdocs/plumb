//! Golden snapshot for the `spacing/scale-conformance` rule.
//!
//! Hand-built fixture mirroring the grid-conformance test: three
//! `<div>` siblings under `<html> > <body>`. One node carries fully
//! in-scale values (no violations); two carry off-scale values on
//! several properties.

use indexmap::IndexMap;
use plumb_core::config::SpacingSpec;
use plumb_core::report::Rect;
use plumb_core::snapshot::SnapshotNode;
use plumb_core::{Config, PlumbSnapshot, ViewportKey, run};

fn fixture_snapshot() -> PlumbSnapshot {
    let in_scale = node(
        2,
        "html > body > div:nth-child(1)",
        &[
            ("margin-top", "0"),
            ("margin-right", "8px"),
            ("margin-bottom", "16px"),
            ("margin-left", "24px"),
            ("padding-top", "4px"),
            ("padding-right", "0"),
            ("padding-bottom", "12px"),
            ("padding-left", "32px"),
            ("gap", "48px"),
            ("row-gap", "0"),
            ("column-gap", "16px"),
        ],
        Some(Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 100,
        }),
    );
    let off_scale_a = node(
        3,
        "html > body > div:nth-child(2)",
        &[
            // 20 and 36 are valid pixel multiples of 4 but not in the
            // scale [0,4,8,12,16,24,32,48], so the grid rule passes
            // and only the scale rule fires.
            ("margin-top", "0"),
            ("margin-right", "20px"),
            ("margin-bottom", "16px"),
            ("margin-left", "24px"),
            ("padding-top", "4px"),
            ("padding-right", "0"),
            ("padding-bottom", "36px"),
            ("padding-left", "32px"),
            ("gap", "48px"),
            ("row-gap", "0"),
            ("column-gap", "16px"),
        ],
        Some(Rect {
            x: 0,
            y: 100,
            width: 200,
            height: 100,
        }),
    );
    let off_scale_b = node(
        4,
        "html > body > div:nth-child(3)",
        &[
            ("margin-top", "0"),
            ("margin-right", "8px"),
            ("margin-bottom", "16px"),
            ("margin-left", "24px"),
            ("padding-top", "4px"),
            ("padding-right", "0"),
            ("padding-bottom", "12px"),
            ("padding-left", "32px"),
            // 40 is not in the scale; row-gap 28 isn't either.
            ("gap", "40px"),
            ("row-gap", "28px"),
            ("column-gap", "16px"),
        ],
        Some(Rect {
            x: 0,
            y: 200,
            width: 200,
            height: 100,
        }),
    );

    PlumbSnapshot {
        url: "plumb-fake://spacing-scale".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![root_html(), body_node(), in_scale, off_scale_a, off_scale_b],
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
        children: vec![2, 3, 4],
    }
}

fn node(
    dom_order: u64,
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
        tag: "div".into(),
        attrs: IndexMap::new(),
        computed_styles,
        rect,
        parent: Some(1),
        children: Vec::new(),
    }
}

fn fixture_config() -> Config {
    Config {
        spacing: SpacingSpec {
            base_unit: 4,
            scale: vec![0, 4, 8, 12, 16, 24, 32, 48],
            tokens: IndexMap::new(),
        },
        ..Config::default()
    }
}

#[test]
fn spacing_scale_conformance_golden() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "spacing/scale-conformance")
        .collect();
    let json = serde_json::to_string_pretty(&violations)?;
    insta::assert_snapshot!("spacing_scale_conformance", json);
    Ok(())
}

#[test]
fn spacing_scale_conformance_run_is_deterministic() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let a = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let b = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let c = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    assert_eq!(a, b);
    assert_eq!(b, c);
    Ok(())
}
