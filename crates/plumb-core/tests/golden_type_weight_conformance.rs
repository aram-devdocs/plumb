//! Golden snapshot for the `type/weight-conformance` rule.
//!
//! Three elements with `font-weight`: one on-scale, one off-scale,
//! one with a non-numeric weight (skipped).

use indexmap::IndexMap;
use plumb_core::config::TypeScaleSpec;
use plumb_core::report::Rect;
use plumb_core::snapshot::SnapshotNode;
use plumb_core::{Config, PlumbSnapshot, ViewportKey, run};

fn fixture_snapshot() -> PlumbSnapshot {
    let on_scale = node(
        2,
        "html > body > div:nth-child(1)",
        &[("font-weight", "400")],
        Some(Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 24,
        }),
    );
    let off_scale = node(
        3,
        "html > body > div:nth-child(2)",
        &[("font-weight", "450")],
        Some(Rect {
            x: 0,
            y: 24,
            width: 200,
            height: 24,
        }),
    );
    let non_numeric = node(
        4,
        "html > body > div:nth-child(3)",
        &[("font-weight", "bold")],
        Some(Rect {
            x: 0,
            y: 48,
            width: 200,
            height: 24,
        }),
    );

    PlumbSnapshot {
        url: "plumb-fake://type-weight".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![root_html(), body_node(), on_scale, off_scale, non_numeric],
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
            weights: vec![100, 300, 400, 500, 700, 900],
            scale: Vec::new(),
            tokens: IndexMap::new(),
        },
        ..Config::default()
    }
}

#[test]
fn type_weight_conformance_golden() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "type/weight-conformance")
        .collect();
    let json = serde_json::to_string_pretty(&violations)?;
    insta::assert_snapshot!("type_weight_conformance", json);
    Ok(())
}

#[test]
fn type_weight_conformance_run_is_deterministic() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let a = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let b = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let c = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    assert_eq!(a, b);
    assert_eq!(b, c);
    Ok(())
}
