//! Golden snapshot for selector-scoped runtime suppression
//! (`[[ignore]]` in `plumb.toml`).
//!
//! Hand-built fixture: 4 `<div>` siblings under `<html> > <body>`,
//! all carrying `padding-top: 13px`. The default config fires both
//! `spacing/grid-conformance` (off the base-unit grid) and
//! `spacing/scale-conformance` (off the discrete scale) on each div,
//! producing eight raw violations.
//!
//! Two ignore entries exercise both match shapes:
//!
//! - selector-only on `:nth-child(1)` suppresses every rule there
//!   (both `grid` and `scale` violations are partitioned out).
//! - selector + `rule_id` on `:nth-child(2)` suppresses only the
//!   `spacing/grid-conformance` violation there; the
//!   `spacing/scale-conformance` violation still surfaces.
//!
//! Net partition: five reported, three ignored — all sorted
//! deterministically by `Violation::sort_key`.

use indexmap::IndexMap;
use plumb_core::config::{IgnoreRule, SpacingSpec};
use plumb_core::report::Rect;
use plumb_core::snapshot::SnapshotNode;
use plumb_core::{Config, PlumbSnapshot, ViewportKey, run_report};

fn fixture_snapshot() -> PlumbSnapshot {
    let mut nodes = vec![root_html(), body_node()];
    for i in 1..=4_u64 {
        nodes.push(off_grid_div(i));
    }
    PlumbSnapshot {
        url: "plumb-fake://ignore-filter".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes,
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
        children: vec![2, 3, 4, 5],
    }
}

fn off_grid_div(nth: u64) -> SnapshotNode {
    let selector = format!("html > body > div:nth-child({nth})");
    let mut computed_styles = IndexMap::new();
    // Off-grid against base unit 4.
    computed_styles.insert("padding-top".into(), "13px".into());
    SnapshotNode {
        dom_order: nth + 1,
        selector,
        tag: "div".into(),
        attrs: IndexMap::new(),
        computed_styles,
        rect: Some(Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 100,
        }),
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
        ignore: vec![
            IgnoreRule {
                selector: "html > body > div:nth-child(1)".into(),
                rule_id: None,
                reason: "first card is theme chrome".into(),
            },
            IgnoreRule {
                selector: "html > body > div:nth-child(2)".into(),
                rule_id: Some("spacing/grid-conformance".into()),
                reason: "second card spacing is intentionally off-grid".into(),
            },
        ],
        ..Config::default()
    }
}

#[test]
fn golden_ignore_filter_partitions_reported_and_ignored() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let report = run_report([&snapshot], &config);

    // The four off-grid divs each emit one `spacing/grid-conformance`
    // violation. The ignore list suppresses two of them, leaving two
    // reported and two ignored. Plus the body itself fires (it has no
    // padding-top in its computed_styles) — actually no, the body has
    // no styles in this fixture, so it shouldn't fire.
    let payload = serde_json::json!({
        "reported": report.reported,
        "ignored": report.ignored,
    });
    let json = serde_json::to_string_pretty(&payload)?;
    insta::assert_snapshot!("golden_ignore_filter", json);
    Ok(())
}

#[test]
fn golden_ignore_filter_is_deterministic() {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let a = run_report([&snapshot], &config);
    let b = run_report([&snapshot], &config);
    let c = run_report([&snapshot], &config);
    assert_eq!(a, b);
    assert_eq!(b, c);
}
