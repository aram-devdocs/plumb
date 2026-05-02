//! Golden snapshot for the `baseline/rhythm` rule.
//!
//! Hand-built fixture with three nodes:
//! - A `<p>` ON the 24px rhythm grid (no violation).
//! - A `<p>` OFF the 24px rhythm grid (violation expected).
//! - A `<div>` (non-text element, skipped).

use indexmap::IndexMap;
use plumb_core::config::RhythmSpec;
use plumb_core::report::Rect;
use plumb_core::snapshot::SnapshotNode;
use plumb_core::{Config, PlumbSnapshot, ViewportKey, run};

fn fixture_snapshot() -> PlumbSnapshot {
    // Node on-grid: font-size 16px, line-height 24px, rect.y = 0.
    // cap_height = 16 * 0.7 = 11.2
    // half_leading = (24 - 16) / 2 = 4
    // baseline_y = 0 + 4 + 11.2 = 15.2
    // nearest grid = round(15.2 / 24) * 24 = 1 * 24 = 24
    // distance = |15.2 - 24| = 8.8 ... that's off-grid.
    //
    // To get ON grid: we need baseline_y to be a multiple of 24 within
    // tolerance 2. Let's set rect.y = 9 so baseline_y = 9 + 4 + 11.2 = 24.2.
    // distance = |24.2 - 24| = 0.2 < tolerance 2. On-grid.
    let on_grid = text_node(
        2,
        "html > body > p:nth-child(1)",
        "p",
        &[("font-size", "16px"), ("line-height", "24px")],
        Some(Rect {
            x: 0,
            y: 9,
            width: 600,
            height: 24,
        }),
    );

    // Node off-grid: font-size 16px, line-height 24px, rect.y = 5.
    // baseline_y = 5 + 4 + 11.2 = 20.2
    // nearest grid = round(20.2 / 24) * 24 = 1 * 24 = 24
    // distance = |20.2 - 24| = 3.8 > tolerance 2. Off-grid!
    let off_grid = text_node(
        3,
        "html > body > p:nth-child(2)",
        "p",
        &[("font-size", "16px"), ("line-height", "24px")],
        Some(Rect {
            x: 0,
            y: 5,
            width: 600,
            height: 24,
        }),
    );

    // Non-text node (div) — should be skipped entirely.
    let non_text = SnapshotNode {
        dom_order: 4,
        selector: "html > body > div".to_owned(),
        tag: "div".to_owned(),
        attrs: IndexMap::new(),
        computed_styles: {
            let mut s = IndexMap::new();
            s.insert("font-size".to_owned(), "16px".to_owned());
            s.insert("line-height".to_owned(), "24px".to_owned());
            s
        },
        rect: Some(Rect {
            x: 0,
            y: 5,
            width: 600,
            height: 24,
        }),
        parent: Some(1),
        children: Vec::new(),
    };

    PlumbSnapshot {
        url: "plumb-fake://baseline-rhythm".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![root_html(), body_node(), on_grid, off_grid, non_text],
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

fn text_node(
    dom_order: u64,
    selector: &str,
    tag: &str,
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
        tag: tag.to_owned(),
        attrs: IndexMap::new(),
        computed_styles,
        rect,
        parent: Some(1),
        children: Vec::new(),
    }
}

fn fixture_config() -> Config {
    Config {
        rhythm: RhythmSpec {
            base_line_px: 24,
            tolerance_px: 2,
            cap_height_fallback_px: 0,
        },
        ..Config::default()
    }
}

#[test]
fn baseline_rhythm_golden() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "baseline/rhythm")
        .collect();
    let json = serde_json::to_string_pretty(&violations)?;
    insta::assert_snapshot!("baseline_rhythm", json);
    Ok(())
}

#[test]
fn baseline_rhythm_run_is_deterministic() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = fixture_config();
    let a = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let b = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let c = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    assert_eq!(a, b);
    assert_eq!(b, c);
    Ok(())
}
