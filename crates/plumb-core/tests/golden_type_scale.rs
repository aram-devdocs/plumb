//! Golden snapshot for the `type/scale-conformance` rule.
//!
//! Hand-built fixture: three `<div>` siblings with varying
//! `font-size` values. One is in-scale, two are off-scale.

use indexmap::IndexMap;
use plumb_core::config::TypeScaleSpec;
use plumb_core::report::Rect;
use plumb_core::snapshot::SnapshotNode;
use plumb_core::{Config, PlumbSnapshot, ViewportKey, run};

fn fixture_snapshot() -> PlumbSnapshot {
    let in_scale = node(
        2,
        "html > body > div:nth-child(1)",
        &[("font-size", "16px")],
        Some(Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 24,
        }),
    );
    let off_scale_a = node(
        3,
        "html > body > div:nth-child(2)",
        &[("font-size", "15px")],
        Some(Rect {
            x: 0,
            y: 24,
            width: 200,
            height: 24,
        }),
    );
    let off_scale_b = node(
        4,
        "html > body > div:nth-child(3)",
        &[("font-size", "22px")],
        Some(Rect {
            x: 0,
            y: 48,
            width: 200,
            height: 32,
        }),
    );

    PlumbSnapshot {
        url: "plumb-fake://type-scale".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![root_html(), body_node(), in_scale, off_scale_a, off_scale_b],
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
        type_scale: TypeScaleSpec {
            families: Vec::new(),
            weights: Vec::new(),
            scale: vec![12, 14, 16, 18, 20, 24, 30, 36, 48],
            tokens: IndexMap::new(),
        },
        ..Config::default()
    }
}

#[test]
fn type_scale_conformance_golden() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "type/scale-conformance")
        .collect();
    let json = serde_json::to_string_pretty(&violations)?;
    insta::assert_snapshot!("type_scale_conformance", json);
    Ok(())
}

#[test]
fn type_scale_conformance_run_is_deterministic() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let a = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let b = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let c = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    assert_eq!(a, b);
    assert_eq!(b, c);
    Ok(())
}
