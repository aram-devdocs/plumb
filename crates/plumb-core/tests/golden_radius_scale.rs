//! Golden snapshot for the `radius/scale-conformance` rule.
//!
//! Hand-built fixture mirroring the spacing-scale-conformance test:
//! three `<div>` siblings under `<html> > <body>`. One node carries
//! fully in-scale corner radii (no violations); two carry off-scale
//! values on several properties.

use indexmap::IndexMap;
use plumb_core::config::RadiusSpec;
use plumb_core::report::Rect;
use plumb_core::snapshot::SnapshotNode;
use plumb_core::{Config, PlumbSnapshot, ViewportKey, run};

fn fixture_snapshot() -> PlumbSnapshot {
    let in_scale = node(
        2,
        "html > body > div:nth-child(1)",
        &[
            ("border-top-left-radius", "0"),
            ("border-top-right-radius", "4px"),
            ("border-bottom-right-radius", "8px"),
            ("border-bottom-left-radius", "12px"),
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
            // 5 and 13 are off-scale against [0, 4, 8, 12, 16, 24].
            ("border-top-left-radius", "5px"),
            ("border-top-right-radius", "4px"),
            ("border-bottom-right-radius", "13px"),
            ("border-bottom-left-radius", "0"),
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
            ("border-top-left-radius", "0"),
            ("border-top-right-radius", "0"),
            // 20 sits between 16 and 24; tie-break favours the lower
            // value (16).
            ("border-bottom-right-radius", "20px"),
            ("border-bottom-left-radius", "0"),
        ],
        Some(Rect {
            x: 0,
            y: 200,
            width: 200,
            height: 100,
        }),
    );

    PlumbSnapshot {
        url: "plumb-fake://radius-scale".into(),
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
        radius: RadiusSpec {
            scale: vec![0, 4, 8, 12, 16, 24],
        },
        ..Config::default()
    }
}

#[test]
fn radius_scale_conformance_golden() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "radius/scale-conformance")
        .collect();
    let json = serde_json::to_string_pretty(&violations)?;
    insta::assert_snapshot!("radius_scale_conformance", json);
    Ok(())
}

#[test]
fn radius_scale_conformance_run_is_deterministic() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let a = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let b = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let c = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    assert_eq!(a, b);
    assert_eq!(b, c);
    Ok(())
}
