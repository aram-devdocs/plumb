//! Golden snapshot for the `spacing/grid-conformance` rule.
//!
//! Hand-built fixture: three `<div>` siblings under `<html>`. One node
//! is fully on-grid (must NOT fire). Two have off-grid values on
//! several properties (each off-grid property must produce one
//! violation, in deterministic sort order).

use indexmap::IndexMap;
use plumb_core::config::SpacingSpec;
use plumb_core::report::Rect;
use plumb_core::snapshot::SnapshotNode;
use plumb_core::{Config, PlumbSnapshot, ViewportKey, run};

fn fixture_snapshot() -> PlumbSnapshot {
    let on_grid = node(
        1,
        "html > body > div:nth-child(1)",
        &[
            ("margin-top", "0"),
            ("margin-right", "8px"),
            ("margin-bottom", "12px"),
            ("margin-left", "16px"),
            ("padding-top", "4px"),
            ("padding-right", "0"),
            ("padding-bottom", "8px"),
            ("padding-left", "0"),
            ("gap", "16px"),
            ("row-gap", "0"),
            ("column-gap", "12px"),
        ],
        Some(Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 100,
        }),
    );
    let off_grid_a = node(
        2,
        "html > body > div:nth-child(2)",
        &[
            // Off-grid on padding-top / margin-bottom; on-grid on the
            // others. Two violations expected.
            ("margin-top", "0"),
            ("margin-right", "8px"),
            ("margin-bottom", "13px"),
            ("margin-left", "16px"),
            ("padding-top", "5px"),
            ("padding-right", "0"),
            ("padding-bottom", "8px"),
            ("padding-left", "0"),
            ("gap", "16px"),
            ("row-gap", "0"),
            ("column-gap", "12px"),
        ],
        Some(Rect {
            x: 0,
            y: 100,
            width: 200,
            height: 100,
        }),
    );
    let off_grid_b = node(
        3,
        "html > body > div:nth-child(3)",
        &[
            // Off-grid on margin-left (-13 against base 4) and gap (7).
            ("margin-top", "0"),
            ("margin-right", "8px"),
            ("margin-bottom", "12px"),
            ("margin-left", "-13px"),
            ("padding-top", "4px"),
            ("padding-right", "0"),
            ("padding-bottom", "8px"),
            ("padding-left", "0"),
            ("gap", "7px"),
            ("row-gap", "0"),
            ("column-gap", "12px"),
        ],
        Some(Rect {
            x: 0,
            y: 200,
            width: 200,
            height: 100,
        }),
    );

    PlumbSnapshot {
        url: "plumb-fake://spacing-grid".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![
            root_html_with_body(),
            body_node(),
            on_grid,
            off_grid_a,
            off_grid_b,
        ],
        text_boxes: Vec::new(),
    }
}

fn root_html_with_body() -> SnapshotNode {
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
fn spacing_grid_conformance_golden() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "spacing/grid-conformance")
        .collect();
    let json = serde_json::to_string_pretty(&violations)?;
    insta::assert_snapshot!("spacing_grid_conformance", json);
    Ok(())
}

#[test]
fn spacing_grid_conformance_run_is_deterministic() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let a = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let b = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let c = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    assert_eq!(a, b);
    assert_eq!(b, c);
    Ok(())
}
