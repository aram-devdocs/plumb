//! Dogfood phase-gate coverage for the V0 Phase 2 MVP rule set.
//!
//! Per-rule golden tests prove each rule's diagnostic shape. This test
//! runs all eight built-ins together against one curated fixture and
//! treats any missing rule, unexpected rule, or unexpected count as a
//! phase-gate failure. The fixture intentionally includes one clean
//! control for each configured design-system scale so cross-rule noise
//! shows up here instead of being hidden by per-rule filtering.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use plumb_core::config::{AlignmentSpec, ColorSpec, RadiusSpec, SpacingSpec, TypeScaleSpec};
use plumb_core::report::Rect;
use plumb_core::snapshot::SnapshotNode;
use plumb_core::{Config, PlumbSnapshot, ViewportKey, run};

#[allow(clippy::too_many_lines)]
fn dogfood_snapshot() -> PlumbSnapshot {
    let nodes = vec![
        node(0, "html", "html", None, None, &[], &[], &[1]),
        node(1, "html > body", "body", Some(0), None, &[], &[], &[]),
        node(
            10,
            "#spacing-grid",
            "section",
            Some(1),
            None,
            &[],
            &[],
            &[11],
        ),
        node(
            11,
            "#spacing-grid > .off-grid",
            "div",
            Some(10),
            Some(rect(0, 0, 100, 40)),
            &[("padding-left", "5px")],
            &[],
            &[],
        ),
        node(
            20,
            "#spacing-scale",
            "section",
            Some(1),
            None,
            &[],
            &[],
            &[21],
        ),
        node(
            21,
            "#spacing-scale > .off-scale",
            "div",
            Some(20),
            Some(rect(0, 0, 100, 40)),
            &[("margin-right", "20px")],
            &[],
            &[],
        ),
        node(30, "#type", "section", Some(1), None, &[], &[], &[31]),
        node(
            31,
            "#type > .off-scale",
            "p",
            Some(30),
            Some(rect(0, 0, 100, 24)),
            &[("font-size", "15px")],
            &[],
            &[],
        ),
        node(40, "#color", "section", Some(1), None, &[], &[], &[41]),
        node(
            41,
            "#color > .off-palette",
            "div",
            Some(40),
            Some(rect(0, 0, 100, 40)),
            &[("color", "rgb(255, 0, 153)")],
            &[],
            &[],
        ),
        node(50, "#radius", "section", Some(1), None, &[], &[], &[51]),
        node(
            51,
            "#radius > .off-scale",
            "div",
            Some(50),
            Some(rect(0, 0, 100, 40)),
            &[("border-top-left-radius", "5px")],
            &[],
            &[],
        ),
        node(60, "#a11y", "section", Some(1), None, &[], &[], &[61]),
        node(
            61,
            "#a11y > button",
            "button",
            Some(60),
            Some(rect(0, 0, 20, 20)),
            &[],
            &[],
            &[],
        ),
        node(
            70,
            "#siblings",
            "section",
            Some(1),
            None,
            &[],
            &[],
            &[71, 72, 73],
        ),
        node(
            71,
            "#siblings > .card:nth-child(1)",
            "article",
            Some(70),
            Some(rect(0, 100, 100, 80)),
            &[],
            &[],
            &[],
        ),
        node(
            72,
            "#siblings > .card:nth-child(2)",
            "article",
            Some(70),
            Some(rect(120, 100, 100, 100)),
            &[],
            &[],
            &[],
        ),
        node(
            73,
            "#siblings > .card:nth-child(3)",
            "article",
            Some(70),
            Some(rect(240, 100, 100, 100)),
            &[],
            &[],
            &[],
        ),
        node(80, "#edges", "section", Some(1), None, &[], &[], &[81, 82]),
        node(
            81,
            "#edges > .tile:nth-child(1)",
            "div",
            Some(80),
            Some(rect(0, 220, 100, 40)),
            &[],
            &[],
            &[],
        ),
        node(
            82,
            "#edges > .tile:nth-child(2)",
            "div",
            Some(80),
            Some(rect(2, 300, 140, 40)),
            &[],
            &[],
            &[],
        ),
        node(
            90,
            "#controls",
            "section",
            Some(1),
            None,
            &[],
            &[],
            &[91, 92, 93],
        ),
        node(
            91,
            "#controls > .spacing-clean",
            "div",
            Some(90),
            Some(rect(0, 0, 100, 40)),
            &[("padding-left", "16px")],
            &[],
            &[],
        ),
        node(
            92,
            "#controls > .type-clean",
            "p",
            Some(90),
            Some(rect(120, 0, 100, 40)),
            &[("font-size", "16px")],
            &[],
            &[],
        ),
        node(
            93,
            "#controls > .radius-color-clean",
            "div",
            Some(90),
            Some(rect(240, 0, 100, 40)),
            &[("border-top-left-radius", "4px"), ("color", "#0b7285")],
            &[],
            &[],
        ),
    ];

    PlumbSnapshot {
        url: "plumb-fake://phase-2-dogfood".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes,
        text_boxes: Vec::new(),
    }
}

fn dogfood_config() -> Config {
    let mut colors = IndexMap::new();
    colors.insert("brand/primary".to_owned(), "#0b7285".to_owned());
    colors.insert("white".to_owned(), "#ffffff".to_owned());
    colors.insert("black".to_owned(), "#000000".to_owned());

    Config {
        spacing: SpacingSpec {
            base_unit: 4,
            scale: vec![0, 4, 5, 8, 12, 16, 24, 32, 48],
            tokens: IndexMap::new(),
        },
        type_scale: TypeScaleSpec {
            families: Vec::new(),
            weights: Vec::new(),
            scale: vec![12, 14, 16, 18, 20, 24, 30, 36, 48],
            tokens: IndexMap::new(),
        },
        color: ColorSpec {
            tokens: colors,
            delta_e_tolerance: 2.0,
        },
        radius: RadiusSpec {
            scale: vec![0, 4, 8, 12, 16],
        },
        alignment: AlignmentSpec {
            grid_columns: None,
            gutter_px: None,
            tolerance_px: 3,
        },
        ..Config::default()
    }
}

fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn node(
    dom_order: u64,
    selector: &str,
    tag: &str,
    parent: Option<u64>,
    rect: Option<Rect>,
    styles: &[(&str, &str)],
    attrs: &[(&str, &str)],
    children: &[u64],
) -> SnapshotNode {
    let computed_styles = styles
        .iter()
        .map(|(prop, value)| ((*prop).to_owned(), (*value).to_owned()))
        .collect();
    let attrs = attrs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();

    SnapshotNode {
        dom_order,
        selector: selector.to_owned(),
        tag: tag.to_owned(),
        attrs,
        computed_styles,
        rect,
        parent,
        children: children.to_vec(),
    }
}

#[test]
fn phase_2_dogfood_fixture_exercises_all_mvp_rules_without_extra_findings() {
    let violations = run(&dogfood_snapshot(), &dogfood_config());
    let by_rule =
        violations
            .into_iter()
            .fold(BTreeMap::<String, usize>::new(), |mut counts, violation| {
                *counts.entry(violation.rule_id).or_default() += 1;
                counts
            });

    let expected = BTreeMap::from([
        ("a11y/touch-target".to_owned(), 1),
        ("color/palette-conformance".to_owned(), 1),
        ("edge/near-alignment".to_owned(), 2),
        ("radius/scale-conformance".to_owned(), 1),
        ("sibling/height-consistency".to_owned(), 1),
        ("spacing/grid-conformance".to_owned(), 1),
        ("spacing/scale-conformance".to_owned(), 1),
        ("type/scale-conformance".to_owned(), 1),
    ]);

    assert_eq!(by_rule, expected);
}
