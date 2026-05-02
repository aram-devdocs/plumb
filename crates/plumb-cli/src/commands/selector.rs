//! `--selector` filter for `plumb lint`.
//!
//! Restricts a [`PlumbSnapshot`] to elements matching a CSS selector and
//! their descendants. The filter is applied between snapshot collection
//! and rule dispatch, so rules see exactly the subtree the user asked to
//! lint.
//!
//! ## How it works
//!
//! 1. Serialize the snapshot's node tree into an HTML document, tagging
//!    each emitted element with its `dom_order` in a
//!    `data-plumb-dom-order` attribute.
//! 2. Parse the HTML and apply the user's CSS selector via
//!    [`scraper::Selector`].
//! 3. Read the `data-plumb-dom-order` attribute from each match to map
//!    back to snapshot nodes, then walk `node.children` to expand to
//!    every descendant.
//! 4. Return a new snapshot whose `nodes` vector contains only the kept
//!    set, with `parent`/`children` references rewritten to stay
//!    consistent.
//!
//! ## Determinism
//!
//! The HTML serialization is a deterministic in-order walk; matched
//! `dom_order` values are sorted and deduplicated; the kept set is a
//! `BTreeSet`. Two runs over the same snapshot and selector produce
//! byte-identical output.

use std::collections::{BTreeSet, VecDeque};

use indexmap::IndexMap;
use plumb_core::PlumbSnapshot;
use plumb_core::snapshot::SnapshotNode;
use thiserror::Error;

/// HTML5 void elements — emitted without a closing tag during snapshot
/// serialization. Every other tag receives a matching close.
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];

/// Attribute name carrying each element's snapshot `dom_order` through
/// the serialize-then-parse round trip. Chosen for its `data-` prefix so
/// it survives every spec-compliant HTML parser.
const DOM_ORDER_ATTR: &str = "data-plumb-dom-order";

/// Errors raised by [`filter_snapshot`].
///
/// Both variants surface through the CLI as exit code 2 (CLI /
/// infrastructure failure, per PRD §13.3).
#[derive(Debug, Error)]
pub enum SelectorError {
    /// The user-supplied CSS selector failed to parse. The message comes
    /// straight from `scraper::SelectorErrorKind::Display`, converted to
    /// an owned `String` so the upstream `scraper` types stay private to
    /// this module.
    #[error("invalid --selector `{selector}`: {message}")]
    ParseError {
        /// The selector string the user passed.
        selector: String,
        /// Human-readable parse error message.
        message: String,
    },
    /// The selector parsed but matched no elements in the snapshot.
    #[error("--selector `{selector}` matched no elements in the snapshot")]
    NoMatch {
        /// The selector string the user passed.
        selector: String,
    },
}

/// Filter `snapshot` to elements matching `selector` and their
/// descendants.
///
/// # Errors
///
/// Returns [`SelectorError::ParseError`] if `selector` is not a valid
/// CSS selector, and [`SelectorError::NoMatch`] if it parses but no
/// element in the snapshot matches it.
pub fn filter_snapshot(
    snapshot: PlumbSnapshot,
    selector: &str,
) -> Result<PlumbSnapshot, SelectorError> {
    let parsed = scraper::Selector::parse(selector).map_err(|err| SelectorError::ParseError {
        selector: selector.to_owned(),
        message: err.to_string(),
    })?;

    let html = serialize(&snapshot);
    let document = scraper::Html::parse_document(&html);

    let mut matched: Vec<u64> = document
        .select(&parsed)
        .filter_map(|elem| elem.value().attr(DOM_ORDER_ATTR))
        .filter_map(|s| s.parse::<u64>().ok())
        .collect();
    matched.sort_unstable();
    matched.dedup();

    if matched.is_empty() {
        return Err(SelectorError::NoMatch {
            selector: selector.to_owned(),
        });
    }

    let index = build_index(&snapshot);
    let kept = expand_to_descendants(&snapshot, &index, &matched);
    Ok(rewrite(snapshot, &kept))
}

/// Map `dom_order` to the index into `snapshot.nodes`. Snapshots are
/// produced in document order, but rebuilding the index avoids relying
/// on `dom_order == position`.
fn build_index(snapshot: &PlumbSnapshot) -> IndexMap<u64, usize> {
    snapshot
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.dom_order, i))
        .collect()
}

/// Walk every matched node's subtree and collect the union into a
/// sorted set.
fn expand_to_descendants(
    snapshot: &PlumbSnapshot,
    index: &IndexMap<u64, usize>,
    matched: &[u64],
) -> BTreeSet<u64> {
    let mut kept = BTreeSet::new();
    let mut queue: VecDeque<u64> = matched.iter().copied().collect();
    while let Some(dom_order) = queue.pop_front() {
        if !kept.insert(dom_order) {
            continue;
        }
        if let Some(&i) = index.get(&dom_order) {
            for &child in &snapshot.nodes[i].children {
                if !kept.contains(&child) {
                    queue.push_back(child);
                }
            }
        }
    }
    kept
}

/// Build a new snapshot containing only nodes in `kept`, with
/// `parent`/`children` references rewritten to refer only to other kept
/// nodes.
fn rewrite(snapshot: PlumbSnapshot, kept: &BTreeSet<u64>) -> PlumbSnapshot {
    let PlumbSnapshot {
        url,
        viewport,
        viewport_width,
        viewport_height,
        nodes,
        text_boxes,
    } = snapshot;

    let new_nodes: Vec<SnapshotNode> = nodes
        .into_iter()
        .filter(|node| kept.contains(&node.dom_order))
        .map(|mut node| {
            if let Some(parent) = node.parent
                && !kept.contains(&parent)
            {
                node.parent = None;
            }
            node.children.retain(|c| kept.contains(c));
            node
        })
        .collect();

    // Keep only text boxes whose owning node was retained.
    let new_text_boxes = text_boxes
        .into_iter()
        .filter(|tb| kept.contains(&tb.dom_order))
        .collect();

    PlumbSnapshot {
        url,
        viewport,
        viewport_width,
        viewport_height,
        nodes: new_nodes,
        text_boxes: new_text_boxes,
    }
}

/// Serialize the snapshot's node tree into a single HTML document.
///
/// Each emitted element carries a `data-plumb-dom-order="<u64>"`
/// attribute so the parsed-back document can map matches to snapshot
/// nodes. Attribute values are HTML-escaped; void elements emit no
/// closing tag.
fn serialize(snapshot: &PlumbSnapshot) -> String {
    let index = build_index(snapshot);
    let mut out = String::from("<!doctype html>");
    for node in &snapshot.nodes {
        if node.parent.is_none() {
            write_node(&mut out, snapshot, &index, node);
        }
    }
    out
}

fn write_node(
    out: &mut String,
    snapshot: &PlumbSnapshot,
    index: &IndexMap<u64, usize>,
    node: &SnapshotNode,
) {
    out.push('<');
    out.push_str(&node.tag);
    for (k, v) in &node.attrs {
        // Skip attributes whose names collide with the dom-order
        // marker; the marker added below is always authoritative.
        if k == DOM_ORDER_ATTR {
            continue;
        }
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        push_escaped_attr(out, v);
        out.push('"');
    }
    out.push(' ');
    out.push_str(DOM_ORDER_ATTR);
    out.push_str("=\"");
    out.push_str(&node.dom_order.to_string());
    out.push('"');
    out.push('>');

    if VOID_ELEMENTS.contains(&node.tag.as_str()) {
        return;
    }

    for &child_order in &node.children {
        if let Some(&i) = index.get(&child_order) {
            write_node(out, snapshot, index, &snapshot.nodes[i]);
        }
    }

    out.push_str("</");
    out.push_str(&node.tag);
    out.push('>');
}

fn push_escaped_attr(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SelectorError, filter_snapshot};
    use indexmap::IndexMap;
    use plumb_core::snapshot::SnapshotNode;
    use plumb_core::{PlumbSnapshot, ViewportKey};

    /// A small but non-trivial document:
    ///
    /// ```text
    /// html
    /// ├── head
    /// └── body
    ///     ├── header
    ///     ├── main
    ///     │   └── article
    ///     │       └── p
    ///     └── footer
    /// ```
    fn fixture() -> PlumbSnapshot {
        fn node(
            dom_order: u64,
            tag: &str,
            parent: Option<u64>,
            children: Vec<u64>,
        ) -> SnapshotNode {
            SnapshotNode {
                dom_order,
                selector: tag.to_owned(),
                tag: tag.to_owned(),
                attrs: IndexMap::new(),
                computed_styles: IndexMap::new(),
                rect: None,
                parent,
                children,
            }
        }
        PlumbSnapshot {
            url: "plumb-fake://fixture".into(),
            viewport: ViewportKey::new("desktop"),
            viewport_width: 1280,
            viewport_height: 800,
            nodes: vec![
                node(0, "html", None, vec![1, 2]),
                node(1, "head", Some(0), vec![]),
                node(2, "body", Some(0), vec![3, 4, 7]),
                node(3, "header", Some(2), vec![]),
                node(4, "main", Some(2), vec![5]),
                node(5, "article", Some(4), vec![6]),
                node(6, "p", Some(5), vec![]),
                node(7, "footer", Some(2), vec![]),
            ],
            text_boxes: Vec::new(),
        }
    }

    fn dom_orders(snapshot: &PlumbSnapshot) -> Vec<u64> {
        snapshot.nodes.iter().map(|n| n.dom_order).collect()
    }

    #[test]
    fn filter_keeps_matched_node_and_descendants() {
        let snap = fixture();
        let filtered = filter_snapshot(snap, "main").expect("main exists");
        // main, article, p
        assert_eq!(dom_orders(&filtered), vec![4, 5, 6]);
    }

    #[test]
    fn filter_clears_parent_of_matched_root() {
        let snap = fixture();
        let filtered = filter_snapshot(snap, "main").expect("main exists");
        let main = filtered
            .nodes
            .iter()
            .find(|n| n.dom_order == 4)
            .expect("main node retained");
        // body (its parent in the original snapshot) is not in the kept
        // set, so the matched root's parent reference is cleared.
        assert!(main.parent.is_none());
    }

    #[test]
    fn filter_drops_unmatched_siblings() {
        let snap = fixture();
        let filtered = filter_snapshot(snap, "main").expect("main exists");
        // header (3) and footer (7) are siblings of main; they should be
        // gone, and body's `children` references to them should be too.
        let kept: Vec<u64> = dom_orders(&filtered);
        assert!(!kept.contains(&3));
        assert!(!kept.contains(&7));
        assert!(!kept.contains(&2));
    }

    #[test]
    fn filter_with_grouped_selector_keeps_all_matches() {
        let snap = fixture();
        let filtered = filter_snapshot(snap, "head, footer").expect("both exist");
        assert_eq!(dom_orders(&filtered), vec![1, 7]);
    }

    #[test]
    fn filter_no_match_returns_no_match_error() {
        let snap = fixture();
        // `aside` is not present in the fixture.
        let err = filter_snapshot(snap, "aside").expect_err("nothing matches");
        let SelectorError::NoMatch { selector } = err else {
            panic!("expected NoMatch, got {err:?}");
        };
        assert_eq!(selector, "aside");
    }

    #[test]
    fn filter_invalid_selector_returns_parse_error() {
        let snap = fixture();
        let err = filter_snapshot(snap, ">>>").expect_err("`>>>` is not a selector");
        let SelectorError::ParseError { selector, message } = err else {
            panic!("expected ParseError, got {err:?}");
        };
        assert_eq!(selector, ">>>");
        assert!(!message.is_empty(), "parse error must carry a message");
    }

    #[test]
    fn filter_universal_selector_keeps_everything() {
        let snap = fixture();
        let original = dom_orders(&snap);
        let filtered = filter_snapshot(snap, "*").expect("wildcard matches");
        assert_eq!(dom_orders(&filtered), original);
    }

    #[test]
    fn filter_canned_snapshot_to_body_keeps_violation_node() {
        // The canned snapshot exposes one off-grid `padding-top: 13px`
        // on `<body>`. Filtering to `body` must retain the body node so
        // `spacing/grid-conformance` still fires.
        let snap = PlumbSnapshot::canned();
        let filtered = filter_snapshot(snap, "body").expect("body exists");
        assert_eq!(filtered.nodes.len(), 1);
        assert_eq!(filtered.nodes[0].tag, "body");
    }

    #[test]
    fn filter_canned_snapshot_to_head_drops_body() {
        let snap = PlumbSnapshot::canned();
        let filtered = filter_snapshot(snap, "head").expect("head exists");
        assert_eq!(filtered.nodes.len(), 1);
        assert_eq!(filtered.nodes[0].tag, "head");
    }

    #[test]
    fn filter_is_deterministic() {
        let s1 = fixture();
        let s2 = fixture();
        let f1 = filter_snapshot(s1, "main").expect("ok");
        let f2 = filter_snapshot(s2, "main").expect("ok");
        let j1 = serde_json::to_string(&f1).expect("serialize");
        let j2 = serde_json::to_string(&f2).expect("serialize");
        assert_eq!(j1, j2);
    }

    #[test]
    fn filter_preserves_attrs_through_round_trip() {
        // Ensure HTML escaping doesn't drop or corrupt attributes
        // unrelated to the selector match.
        let mut snap = fixture();
        let body = snap
            .nodes
            .iter_mut()
            .find(|n| n.tag == "body")
            .expect("body");
        body.attrs.insert("class".into(), "demo & test".into());
        let filtered = filter_snapshot(snap, "body").expect("body exists");
        let body = filtered
            .nodes
            .iter()
            .find(|n| n.tag == "body")
            .expect("body retained");
        assert_eq!(
            body.attrs.get("class").map(String::as_str),
            Some("demo & test")
        );
    }
}
