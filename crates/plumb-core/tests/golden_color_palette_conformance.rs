//! Golden snapshot for the `color/palette-conformance` rule.
//!
//! Hand-built fixture covering the four behaviours that matter:
//!
//! 1. A node whose `color` matches a palette token exactly — no
//!    violation.
//! 2. A node whose `color` is off-palette by less than the
//!    `delta_e_tolerance` — no violation.
//! 3. A node whose `color` is off-palette by more than the tolerance
//!    — one violation.
//! 4. A node with `color: rgba(...)` carrying alpha < 1 over a
//!    fully-opaque ancestor `background-color`. The composited
//!    foreground sits well outside the palette, so one violation
//!    fires and the violation message references the original raw
//!    value (not the composited result).

use indexmap::IndexMap;
use plumb_core::config::{ColorSpec, Config};
use plumb_core::report::Rect;
use plumb_core::snapshot::SnapshotNode;
use plumb_core::{PlumbSnapshot, ViewportKey, run};

fn fixture_snapshot() -> PlumbSnapshot {
    let exact_match = node(
        2,
        "html > body > div:nth-child(1)",
        &[("color", "rgb(11, 114, 133)")],
        Some(Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 24,
        }),
    );

    // Within tolerance: a slight nudge away from #0b7285.
    let within_tolerance = node(
        3,
        "html > body > div:nth-child(2)",
        &[("color", "rgb(12, 115, 134)")],
        Some(Rect {
            x: 0,
            y: 24,
            width: 200,
            height: 24,
        }),
    );

    // Way off-palette: bright pink against a teal/black/white palette.
    let off_palette = node(
        4,
        "html > body > div:nth-child(3)",
        &[("color", "rgb(255, 0, 153)")],
        Some(Rect {
            x: 0,
            y: 48,
            width: 200,
            height: 24,
        }),
    );

    // Translucent foreground over an opaque ancestor `background-color`.
    // The body's `background-color: #ffffff` is the resolved backdrop.
    // `rgba(0, 0, 0, 0.4)` blended over white lands near a mid-gray
    // that's > 2 ΔE00 from any palette token.
    let translucent = node(
        5,
        "html > body > div:nth-child(4)",
        &[("color", "rgba(0, 0, 0, 0.4)")],
        Some(Rect {
            x: 0,
            y: 72,
            width: 200,
            height: 24,
        }),
    );

    PlumbSnapshot {
        url: "plumb-fake://color-palette".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![
            root_html(),
            body_node(),
            exact_match,
            within_tolerance,
            off_palette,
            translucent,
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
    let mut styles = IndexMap::new();
    // The body declares a fully-opaque white background, so the
    // alpha-blending path in `palette-conformance` resolves the
    // backdrop to `#ffffff` for translucent descendants.
    styles.insert("background-color".into(), "rgb(255, 255, 255)".into());
    SnapshotNode {
        dom_order: 1,
        selector: "html > body".into(),
        tag: "body".into(),
        attrs: IndexMap::new(),
        computed_styles: styles,
        rect: Some(Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 800,
        }),
        parent: Some(0),
        children: vec![2, 3, 4, 5],
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
    let mut tokens = IndexMap::new();
    tokens.insert("white".into(), "#ffffff".into());
    tokens.insert("black".into(), "#000000".into());
    tokens.insert("primary".into(), "#0b7285".into());
    Config {
        color: ColorSpec {
            tokens,
            delta_e_tolerance: 2.0,
        },
        ..Config::default()
    }
}

#[test]
fn color_palette_conformance_golden() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "color/palette-conformance")
        .collect();
    let json = serde_json::to_string_pretty(&violations)?;
    insta::assert_snapshot!("color_palette_conformance", json);
    Ok(())
}

#[test]
fn color_palette_conformance_run_is_deterministic() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let a = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let b = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let c = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    assert_eq!(a, b);
    assert_eq!(b, c);
    Ok(())
}

#[test]
fn color_palette_conformance_skips_when_palette_empty() {
    let snapshot = fixture_snapshot();
    let config = Config::default();
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "color/palette-conformance")
        .collect();
    assert!(
        violations.is_empty(),
        "expected zero violations with empty palette, got {violations:?}"
    );
}
