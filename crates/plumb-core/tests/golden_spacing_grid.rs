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

/// Run the engine over a single `<div>` carrying one `margin-top`
/// value and return how many `spacing/grid-conformance` violations it
/// produces. Used by the tolerance-band test to exercise the `0.5px`
/// snap window from both sides.
fn grid_violations_for_margin_top(value: &str) -> usize {
    let only = node(
        2,
        "html > body > div:nth-child(1)",
        &[("margin-top", value)],
        Some(Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 100,
        }),
    );
    let snapshot = PlumbSnapshot {
        url: "plumb-fake://spacing-grid-tolerance".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![root_html_with_body(), body_node(), only],
        text_boxes: Vec::new(),
    };
    run(&snapshot, &fixture_config())
        .into_iter()
        .filter(|v| v.rule_id == "spacing/grid-conformance")
        .count()
}

#[test]
fn spacing_grid_conformance_tolerance_band() {
    // `base_unit = 4`. A value within 0.5px of the nearest multiple is
    // on-grid (UA-stylesheet residue like a `16.08px` margin snaps to
    // 16); an honest off-grid value still fires.
    assert_eq!(
        grid_violations_for_margin_top("16.08px"),
        0,
        "16.08px is within 0.5 of 16 — on-grid"
    );
    assert_eq!(
        grid_violations_for_margin_top("13px"),
        1,
        "13px is 1px off the nearest multiple (12) — off-grid"
    );
    // Boundary: exactly 0.5px off is still on-grid (the test is `<=`).
    assert_eq!(
        grid_violations_for_margin_top("16.5px"),
        0,
        "16.5px is exactly 0.5 off 16 — still on-grid"
    );
    // Just past the boundary fires.
    assert_eq!(
        grid_violations_for_margin_top("16.6px"),
        1,
        "16.6px is 0.6 off 16 — off-grid"
    );
}

/// Build a one-node snapshot carrying a single spacing property and
/// count `spacing/grid-conformance` violations under `config`. Lets the
/// scale-deferral tests vary both the property value and the configured
/// `spacing.scale`.
fn grid_violations_with(prop: &str, value: &str, config: &Config) -> usize {
    let only = node(
        2,
        "html > body > div:nth-child(1)",
        &[(prop, value)],
        Some(Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 100,
        }),
    );
    let snapshot = PlumbSnapshot {
        url: "plumb-fake://spacing-grid-scale".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![root_html_with_body(), body_node(), only],
        text_boxes: Vec::new(),
    };
    run(&snapshot, config)
        .into_iter()
        .filter(|v| v.rule_id == "spacing/grid-conformance")
        .count()
}

/// Config with an explicit `base_unit` and `spacing.scale`, everything
/// else default. Used by the scale-deferral tests.
fn config_with_scale(base_unit: u32, scale: Vec<u32>) -> Config {
    Config {
        spacing: SpacingSpec {
            base_unit,
            scale,
            tokens: IndexMap::new(),
        },
        ..Config::default()
    }
}

#[test]
fn spacing_grid_conformance_defers_to_configured_scale() {
    // Tailwind-style scale with 2px half-steps. `base_unit` stays 4, so
    // 10px is off the base-4 grid yet explicitly on the design system's
    // scale — it must NOT fire.
    let config = config_with_scale(4, vec![0, 2, 4, 6, 8, 10, 12, 14, 16]);

    assert_eq!(
        grid_violations_with("padding-top", "10px", &config),
        0,
        "10px is on the configured scale (off base-4 grid) — no violation"
    );
    assert_eq!(
        grid_violations_with("padding-top", "13px", &config),
        1,
        "13px is neither on the scale nor on the base-4 grid — off-grid"
    );
    assert_eq!(
        grid_violations_with("padding-top", "10.4px", &config),
        0,
        "10.4px is within 0.5 of the scale member 10 — on-scale"
    );
    assert_eq!(
        grid_violations_with("padding-top", "10.6px", &config),
        1,
        "10.6px is more than 0.5 from every scale member — off-grid"
    );
}

#[test]
fn spacing_grid_conformance_empty_scale_uses_base_unit() {
    // Default-style config: an empty scale means the rule falls back to
    // the pure base-unit grid. 10px is off the base-4 grid and there is
    // no scale member to defer to.
    let config = config_with_scale(4, Vec::new());
    assert_eq!(
        grid_violations_with("padding-top", "10px", &config),
        1,
        "empty scale → pure base-4 grid; 10px is off-grid"
    );
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
