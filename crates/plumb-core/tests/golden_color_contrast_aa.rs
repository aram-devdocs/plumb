//! Golden snapshot for the `color/contrast-aa` rule.
//!
//! The fixture covers the key WCAG AA branches:
//!
//! - normal 16px body text that passes comfortably,
//! - normal 16px text that fails the 4.5:1 threshold,
//! - large 24px text that passes the relaxed 3.0:1 threshold,
//! - bold 18px text that still counts as normal (below the 14pt-bold cutoff),
//! - bold 19px text that counts as large,
//! - text inside a dark section whose nearest ancestor background is not white.

use indexmap::IndexMap;
use plumb_core::report::Rect;
use plumb_core::snapshot::SnapshotNode;
use plumb_core::{Config, PlumbSnapshot, ViewportKey, run};

fn fixture_snapshot() -> PlumbSnapshot {
    PlumbSnapshot {
        url: "plumb-fake://color-contrast-aa".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes: vec![
            root_html(),
            body_node(),
            text_node(
                2,
                "html > body > div:nth-child(1)",
                &[("color", "rgb(0, 0, 0)"), ("font-size", "16px")],
                Some(rect(0, 0, 320, 24)),
                1,
            ),
            text_node(
                3,
                "html > body > div:nth-child(2)",
                &[("color", "rgb(119, 119, 119)"), ("font-size", "16px")],
                Some(rect(0, 32, 320, 24)),
                1,
            ),
            text_node(
                4,
                "html > body > div:nth-child(3)",
                &[("color", "rgb(148, 148, 148)"), ("font-size", "24px")],
                Some(rect(0, 64, 320, 32)),
                1,
            ),
            text_node(
                5,
                "html > body > div:nth-child(4)",
                &[
                    ("color", "rgb(148, 148, 148)"),
                    ("font-size", "18px"),
                    ("font-weight", "700"),
                ],
                Some(rect(0, 104, 320, 28)),
                1,
            ),
            text_node(
                6,
                "html > body > div:nth-child(5)",
                &[
                    ("color", "rgb(148, 148, 148)"),
                    ("font-size", "19px"),
                    ("font-weight", "700"),
                ],
                Some(rect(0, 140, 320, 30)),
                1,
            ),
            section_node(),
            text_node(
                8,
                "html > body > section > p",
                &[("color", "rgb(120, 120, 120)"), ("font-size", "16px")],
                Some(rect(0, 212, 320, 24)),
                7,
            ),
        ],
    }
}

const fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn root_html() -> SnapshotNode {
    SnapshotNode {
        dom_order: 0,
        selector: "html".into(),
        tag: "html".into(),
        attrs: IndexMap::new(),
        computed_styles: IndexMap::new(),
        rect: Some(rect(0, 0, 1280, 800)),
        parent: None,
        children: vec![1],
    }
}

fn body_node() -> SnapshotNode {
    let mut computed_styles = IndexMap::new();
    computed_styles.insert("background-color".into(), "rgb(255, 255, 255)".into());
    SnapshotNode {
        dom_order: 1,
        selector: "html > body".into(),
        tag: "body".into(),
        attrs: IndexMap::new(),
        computed_styles,
        rect: Some(rect(0, 0, 1280, 800)),
        parent: Some(0),
        children: vec![2, 3, 4, 5, 6, 7],
    }
}

fn section_node() -> SnapshotNode {
    let mut computed_styles = IndexMap::new();
    computed_styles.insert("background-color".into(), "rgb(34, 34, 34)".into());
    SnapshotNode {
        dom_order: 7,
        selector: "html > body > section".into(),
        tag: "section".into(),
        attrs: IndexMap::new(),
        computed_styles,
        rect: Some(rect(0, 180, 400, 80)),
        parent: Some(1),
        children: vec![8],
    }
}

fn text_node(
    dom_order: u64,
    selector: &str,
    styles: &[(&str, &str)],
    rect: Option<Rect>,
    parent: u64,
) -> SnapshotNode {
    let mut computed_styles = IndexMap::new();
    for (property, value) in styles {
        computed_styles.insert((*property).to_owned(), (*value).to_owned());
    }
    SnapshotNode {
        dom_order,
        selector: selector.to_owned(),
        tag: "div".into(),
        attrs: IndexMap::new(),
        computed_styles,
        rect,
        parent: Some(parent),
        children: Vec::new(),
    }
}

#[test]
fn color_contrast_aa_golden() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = Config::default();
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|violation| violation.rule_id == "color/contrast-aa")
        .collect();
    let json = serde_json::to_string_pretty(&violations)?;
    insta::assert_snapshot!("color_contrast_aa", json);
    Ok(())
}

#[test]
fn color_contrast_aa_run_is_deterministic() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = Config::default();
    let a = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let b = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let c = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    assert_eq!(a, b);
    assert_eq!(b, c);
    Ok(())
}
