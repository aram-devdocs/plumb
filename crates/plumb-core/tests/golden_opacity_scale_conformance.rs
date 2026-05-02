//! Golden snapshot for the `opacity/scale-conformance` rule.
//!
//! Three elements: one on-scale, one off-scale, one fully opaque
//! (`1` — on-scale).

use indexmap::IndexMap;
use plumb_core::config::OpacitySpec;
use plumb_core::report::Rect;
use plumb_core::snapshot::SnapshotNode;
use plumb_core::{Config, PlumbSnapshot, ViewportKey, run};

fn fixture_snapshot() -> PlumbSnapshot {
    let on_scale = node(
        2,
        "html > body > div:nth-child(1)",
        &[("opacity", "0.5")],
        Some(Rect { x: 0, y: 0, width: 200, height: 24 }),
    );
    let off_scale = node(
        3,
        "html > body > div:nth-child(2)",
        &[("opacity", "0.35")],
        Some(Rect { x: 0, y: 24, width: 200, height: 24 }),
    );
    let opaque = node(
        4,
        "html > body > div:nth-child(3)",
        &[("opacity", "1")],
        Some(Rect { x: 0, y: 48, width: 200, height: 24 }),
    );

    PlumbSnapshot {
        url: "plumb-fake://opacity-scale".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![root_html(), body_node(), on_scale, off_scale, opaque],
    }
}

fn root_html() -> SnapshotNode {
    SnapshotNode {
        dom_order: 0,
        selector: "html".into(),
        tag: "html".into(),
        attrs: IndexMap::new(),
        computed_styles: IndexMap::new(),
        rect: Some(Rect { x: 0, y: 0, width: 1280, height: 800 }),
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
        rect: Some(Rect { x: 0, y: 0, width: 1280, height: 800 }),
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
        opacity: OpacitySpec {
            scale: vec![0.0, 0.25, 0.5, 0.75, 1.0],
        },
        ..Config::default()
    }
}

#[test]
fn opacity_scale_conformance_golden() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "opacity/scale-conformance")
        .collect();
    let json = serde_json::to_string_pretty(&violations)?;
    insta::assert_snapshot!("opacity_scale_conformance", json);
    Ok(())
}

#[test]
fn opacity_scale_conformance_run_is_deterministic() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let a = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let b = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let c = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    assert_eq!(a, b);
    assert_eq!(b, c);
    Ok(())
}
