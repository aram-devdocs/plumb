//! Golden snapshot for the `a11y/touch-target` rule.
//!
//! The fixture exercises the full interactivity matrix:
//!
//! - one comfortably-sized `<button>` (no violation),
//! - a too-small `<button>`,
//! - a too-small anchor with `href` (interactive),
//! - a too-small anchor without `href` (not interactive — skipped),
//! - a too-small `<input type="submit">`,
//! - a too-small `<div role="button">`,
//! - a too-small but non-interactive `<span>` (skipped).

use indexmap::IndexMap;
use plumb_core::report::Rect;
use plumb_core::snapshot::SnapshotNode;
use plumb_core::{Config, PlumbSnapshot, ViewportKey, run};

struct FixtureSpec {
    dom_order: u64,
    selector: &'static str,
    tag: &'static str,
    attrs: &'static [(&'static str, &'static str)],
    rect: Rect,
}

const FIXTURE_NODES: &[FixtureSpec] = &[
    FixtureSpec {
        dom_order: 2,
        selector: "html > body > button:nth-child(1)",
        tag: "button",
        attrs: &[],
        rect: rect(0, 0, 48, 32),
    },
    FixtureSpec {
        dom_order: 3,
        selector: "html > body > button:nth-child(2)",
        tag: "button",
        attrs: &[],
        rect: rect(0, 40, 16, 16),
    },
    FixtureSpec {
        dom_order: 4,
        selector: "html > body > a:nth-child(3)",
        tag: "a",
        attrs: &[("href", "/page")],
        rect: rect(0, 60, 20, 20),
    },
    FixtureSpec {
        dom_order: 5,
        selector: "html > body > a:nth-child(4)",
        tag: "a",
        attrs: &[],
        rect: rect(0, 80, 12, 12),
    },
    FixtureSpec {
        dom_order: 6,
        selector: "html > body > input:nth-child(5)",
        tag: "input",
        attrs: &[("type", "submit"), ("value", "Go")],
        rect: rect(0, 100, 22, 22),
    },
    FixtureSpec {
        dom_order: 7,
        selector: "html > body > div:nth-child(6)",
        tag: "div",
        attrs: &[("role", "button")],
        rect: rect(0, 130, 18, 18),
    },
    FixtureSpec {
        dom_order: 8,
        selector: "html > body > span:nth-child(7)",
        tag: "span",
        attrs: &[],
        rect: rect(0, 160, 8, 8),
    },
];

const fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn fixture_snapshot() -> PlumbSnapshot {
    let mut nodes = vec![root_html(), body_node()];
    nodes.extend(FIXTURE_NODES.iter().map(|spec| {
        node_with_attrs(
            spec.dom_order,
            spec.selector,
            spec.tag,
            spec.attrs,
            Some(spec.rect),
        )
    }));
    PlumbSnapshot {
        url: "plumb-fake://a11y-touch-target".into(),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes,
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
        children: FIXTURE_NODES.iter().map(|spec| spec.dom_order).collect(),
    }
}

fn node_with_attrs(
    dom_order: u64,
    selector: &str,
    tag: &str,
    attrs: &[(&str, &str)],
    rect: Option<Rect>,
) -> SnapshotNode {
    let mut attr_map = IndexMap::new();
    for (k, v) in attrs {
        attr_map.insert((*k).to_owned(), (*v).to_owned());
    }
    SnapshotNode {
        dom_order,
        selector: selector.to_owned(),
        tag: tag.to_owned(),
        attrs: attr_map,
        computed_styles: IndexMap::new(),
        rect,
        parent: Some(1),
        children: Vec::new(),
    }
}

#[test]
fn a11y_touch_target_golden() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = Config::default();
    let violations: Vec<plumb_core::Violation> = run(&snapshot, &config)
        .into_iter()
        .filter(|v| v.rule_id == "a11y/touch-target")
        .collect();
    let json = serde_json::to_string_pretty(&violations)?;
    insta::assert_snapshot!("a11y_touch_target", json);
    Ok(())
}

#[test]
fn a11y_touch_target_run_is_deterministic() -> Result<(), serde_json::Error> {
    let snapshot = fixture_snapshot();
    let config = Config::default();
    let a = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let b = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let c = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    assert_eq!(a, b);
    assert_eq!(b, c);
    Ok(())
}
