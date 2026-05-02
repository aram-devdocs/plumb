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
use plumb_core::{Config, PlumbSnapshot, TextBox, ViewportKey, run};

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

#[test]
fn baseline_rhythm_skips_when_base_line_px_is_zero() {
    let snapshot = fixture_snapshot();
    let config = Config {
        rhythm: RhythmSpec {
            base_line_px: 0,
            tolerance_px: 2,
            cap_height_fallback_px: 0,
        },
        ..Config::default()
    };
    assert!(
        !run(&snapshot, &config)
            .into_iter()
            .any(|v| v.rule_id == "baseline/rhythm"),
        "base_line_px=0 must skip the rule entirely"
    );
}

#[test]
fn baseline_rhythm_uses_cap_height_fallback() {
    // With cap_height_fallback_px = 12, cap_height = 12 (not 16*0.7=11.2).
    // off_grid node: rect.y=5, half_leading=(24-16)/2=4, baseline_y=5+4+12=21.
    // nearest_grid = round(21/24)*24 = 24, distance = 3 > tolerance 2 → violation.
    let snapshot = fixture_snapshot();
    let config = Config {
        rhythm: RhythmSpec {
            base_line_px: 24,
            tolerance_px: 2,
            cap_height_fallback_px: 12,
        },
        ..Config::default()
    };
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "baseline/rhythm")
        .collect();
    assert_eq!(
        violations.len(),
        1,
        "cap_height_fallback_px=12 must still flag the off-grid node"
    );
    assert!(
        violations[0].message.contains("21.0px"),
        "baseline should use fallback cap-height: {}",
        violations[0].message,
    );
}

#[test]
fn baseline_rhythm_falls_back_on_line_height_normal() {
    // Node with no line-height style → falls back to font_size * 1.2.
    // font_size=16, line_height=16*1.2=19.2, half_leading=(19.2-16)/2=1.6
    // cap_height=16*0.7=11.2, baseline_y=0+1.6+11.2=12.8
    // nearest_grid = round(12.8/24)*24 = 24, distance=11.2 > tolerance → violation.
    let node_no_lh = text_node(
        2,
        "html > body > p:nth-child(1)",
        "p",
        &[("font-size", "16px")],
        Some(Rect {
            x: 0,
            y: 0,
            width: 600,
            height: 20,
        }),
    );
    let snapshot = PlumbSnapshot {
        url: "plumb-fake://baseline-rhythm-lh-normal".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![root_html(), body_node(), node_no_lh],
        text_boxes: Vec::new(),
    };
    let config = Config {
        rhythm: RhythmSpec {
            base_line_px: 24,
            tolerance_px: 2,
            cap_height_fallback_px: 0,
        },
        ..Config::default()
    };
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "baseline/rhythm")
        .collect();
    assert_eq!(
        violations.len(),
        1,
        "missing line-height must fall back to 1.2×font-size"
    );
    assert!(
        violations[0].message.contains("12.8px"),
        "baseline should use 1.2× fallback: {}",
        violations[0].message,
    );
}

#[test]
fn baseline_rhythm_multiline_text_boxes() {
    // One <p> with two text box lines at different Y positions.
    // font_size=16, line_height=24, half_leading=4, cap_height=11.2.
    //
    // Line 1: text_box y=9 → baseline_y = 9+4+11.2 = 24.2 → nearest 24 → dist 0.2 < 2 (on-grid).
    // Line 2: text_box y=38 → baseline_y = 38+4+11.2 = 53.2 → nearest 48 → dist 5.2 > 2 (off-grid!).
    let node = text_node(
        2,
        "html > body > p:nth-child(1)",
        "p",
        &[("font-size", "16px"), ("line-height", "24px")],
        Some(Rect {
            x: 0,
            y: 9,
            width: 600,
            height: 60,
        }),
    );
    let snapshot = PlumbSnapshot {
        url: "plumb-fake://baseline-rhythm-multiline".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![root_html(), body_node(), node],
        text_boxes: vec![
            TextBox {
                dom_order: 2,
                bounds: Rect {
                    x: 0,
                    y: 9,
                    width: 500,
                    height: 24,
                },
                start: 0,
                length: 40,
            },
            TextBox {
                dom_order: 2,
                bounds: Rect {
                    x: 0,
                    y: 38,
                    width: 500,
                    height: 24,
                },
                start: 40,
                length: 35,
            },
        ],
    };
    let config = fixture_config();
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "baseline/rhythm")
        .collect();
    // Only line 2 is off-grid; line 1 is within tolerance.
    assert_eq!(
        violations.len(),
        1,
        "multi-line: only the off-grid line should produce a violation",
    );
    assert!(
        violations[0].message.contains("53.2px"),
        "violation should reference the second line's baseline: {}",
        violations[0].message,
    );
}

#[test]
fn baseline_rhythm_multifont_text_boxes() {
    // Two <p> nodes with different font sizes, each with a text box.
    // Config: base_line_px=24, tolerance_px=2.
    //
    // Node A: font_size=16, line_height=24, text_box y=9.
    //   half_leading=(24-16)/2=4, cap_height=16*0.7=11.2
    //   baseline_y = 9+4+11.2 = 24.2 → nearest 24 → dist 0.2 < 2 (on-grid).
    //
    // Node B: font_size=20, line_height=28, text_box y=50.
    //   half_leading=(28-20)/2=4, cap_height=20*0.7=14
    //   baseline_y = 50+4+14 = 68.0 → nearest 72 → dist 4.0 > 2 (off-grid!).
    let node_a = text_node(
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
    let node_b = text_node(
        3,
        "html > body > h2:nth-child(2)",
        "h2",
        &[("font-size", "20px"), ("line-height", "28px")],
        Some(Rect {
            x: 0,
            y: 50,
            width: 600,
            height: 28,
        }),
    );
    let snapshot = PlumbSnapshot {
        url: "plumb-fake://baseline-rhythm-multifont".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![root_html(), body_node(), node_a, node_b],
        text_boxes: vec![
            TextBox {
                dom_order: 2,
                bounds: Rect {
                    x: 0,
                    y: 9,
                    width: 500,
                    height: 24,
                },
                start: 0,
                length: 30,
            },
            TextBox {
                dom_order: 3,
                bounds: Rect {
                    x: 0,
                    y: 50,
                    width: 500,
                    height: 28,
                },
                start: 0,
                length: 20,
            },
        ],
    };
    let config = fixture_config();
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "baseline/rhythm")
        .collect();
    // Only node B (h2, 20px font) should be off-grid.
    assert_eq!(
        violations.len(),
        1,
        "multi-font: only the off-grid node should produce a violation",
    );
    assert!(
        violations[0].message.contains("68.0px"),
        "violation should reference the h2 baseline: {}",
        violations[0].message,
    );
    assert!(
        violations[0].selector.contains("h2"),
        "violation should be on the h2 node: {}",
        violations[0].selector,
    );
}

#[test]
fn baseline_rhythm_multiline_both_off_grid() {
    // One <p> with two text box lines, BOTH off the 24px grid.
    // font_size=16, line_height=24, half_leading=4, cap_height=11.2.
    //
    // Line 1: text_box y=0 → baseline_y = 0+4+11.2 = 15.2 → nearest 24 → dist 8.8 > 2 (off-grid!)
    // Line 2: text_box y=38 → baseline_y = 38+4+11.2 = 53.2 → nearest 48 → dist 5.2 > 2 (off-grid!)
    //
    // Expect exactly ONE aggregated violation with the multi-line message format.
    let node = text_node(
        2,
        "html > body > p:nth-child(1)",
        "p",
        &[("font-size", "16px"), ("line-height", "24px")],
        Some(Rect {
            x: 0,
            y: 0,
            width: 600,
            height: 62,
        }),
    );
    let snapshot = PlumbSnapshot {
        url: "plumb-fake://baseline-rhythm-both-off".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![root_html(), body_node(), node],
        text_boxes: vec![
            TextBox {
                dom_order: 2,
                bounds: Rect {
                    x: 0,
                    y: 0,
                    width: 500,
                    height: 24,
                },
                start: 0,
                length: 40,
            },
            TextBox {
                dom_order: 2,
                bounds: Rect {
                    x: 0,
                    y: 38,
                    width: 500,
                    height: 24,
                },
                start: 40,
                length: 35,
            },
        ],
    };
    let config = fixture_config();
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "baseline/rhythm")
        .collect();

    assert_eq!(
        violations.len(),
        1,
        "both off-grid lines must produce exactly one aggregated violation",
    );
    assert!(
        violations[0].message.contains("has 2/2 lines off"),
        "message should use aggregated format: {}",
        violations[0].message,
    );
    // Worst distance is 8.8 (line 1), so that's the primary metadata.
    assert!(
        violations[0].message.contains("8.8px"),
        "message should reference worst distance: {}",
        violations[0].message,
    );
    // Metadata should contain off_grid_lines array with 2 entries.
    let off_grid_lines = violations[0]
        .metadata
        .get("off_grid_lines")
        .expect("metadata must contain off_grid_lines");
    let arr = off_grid_lines
        .as_array()
        .expect("off_grid_lines must be an array");
    assert_eq!(arr.len(), 2, "off_grid_lines must have 2 entries");
}
