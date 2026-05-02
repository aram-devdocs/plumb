//! In-memory snapshot of a rendered page at a single viewport.
//!
//! The real `PlumbSnapshot` is populated by `plumb-cdp` via the Chromium
//! DevTools Protocol. For the walking skeleton and tests, a canned
//! constructor is available behind the `test-fake` feature.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::report::{Rect, ViewportKey};

/// A single post-layout text box within a node.
///
/// CDP returns one text box per rendered line fragment. Multi-line text
/// generates multiple boxes for the same `dom_order`. The bounds are
/// absolute viewport coordinates for that line fragment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextBox {
    /// `dom_order` of the owning element node.
    pub dom_order: u64,
    /// Absolute bounding rect of this text fragment.
    pub bounds: Rect,
    /// Starting character index (UTF-16 code units).
    pub start: u32,
    /// Character count (UTF-16 code units).
    pub length: u32,
}

/// A single DOM node as the engine sees it.
///
/// This is intentionally a narrow view: just enough to identify the element
/// and evaluate rules against its computed styles and geometry. The full
/// DOM tree is reconstructable via the `parent`/`children` indices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotNode {
    /// Stable document-order index.
    pub dom_order: u64,
    /// CSS selector path from the document root.
    pub selector: String,
    /// HTML tag name (lowercase).
    pub tag: String,
    /// Attributes as an ordered map — preserves parse order.
    pub attrs: IndexMap<String, String>,
    /// Computed styles relevant to any rule — ordered alphabetically on
    /// insertion.
    pub computed_styles: IndexMap<String, String>,
    /// Bounding rect.
    pub rect: Option<Rect>,
    /// Parent `dom_order`, or `None` for the root.
    pub parent: Option<u64>,
    /// `dom_order` of direct children, in document order.
    pub children: Vec<u64>,
}

/// The full snapshot at a single viewport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlumbSnapshot {
    /// Source URL (may be a `plumb-fake://` URL in tests).
    pub url: String,
    /// The viewport this snapshot was taken at.
    pub viewport: ViewportKey,
    /// Viewport width in CSS pixels.
    pub viewport_width: u32,
    /// Viewport height in CSS pixels.
    pub viewport_height: u32,
    /// All nodes, ordered by `dom_order`.
    pub nodes: Vec<SnapshotNode>,
    /// Post-layout text boxes, sorted by `(dom_order, start)` for determinism.
    pub text_boxes: Vec<TextBox>,
}

impl PlumbSnapshot {
    /// Build an in-memory `hello, world` snapshot. Available only in tests
    /// and in the `plumb-fake://` CLI code path.
    ///
    /// The shape is intentionally minimal: one `<html>` root with two
    /// children (`<head>`, `<body>`). Rules that run against this snapshot
    /// should produce deterministic output.
    #[cfg(any(test, feature = "test-fake"))]
    #[must_use]
    pub fn canned() -> Self {
        let mut html_attrs = IndexMap::new();
        html_attrs.insert("lang".into(), "en".into());

        let mut body_styles = IndexMap::new();
        // Longhands match what `getComputedStyle` returns in the real
        // CDP driver (PRD §10.3). `padding-top: 13px` is deliberately
        // off-grid against the default `spacing.base_unit = 4`, so the
        // walking-skeleton smoke path produces one violation from
        // `spacing/grid-conformance`.
        body_styles.insert("margin-top".into(), "0".into());
        body_styles.insert("margin-right".into(), "0".into());
        body_styles.insert("margin-bottom".into(), "0".into());
        body_styles.insert("margin-left".into(), "0".into());
        body_styles.insert("padding-top".into(), "13px".into());
        body_styles.insert("padding-right".into(), "0".into());
        body_styles.insert("padding-bottom".into(), "0".into());
        body_styles.insert("padding-left".into(), "0".into());

        Self {
            url: "plumb-fake://hello".into(),
            viewport: ViewportKey::new("desktop"),
            viewport_width: 1280,
            viewport_height: 800,
            text_boxes: Vec::new(),
            nodes: vec![
                SnapshotNode {
                    dom_order: 0,
                    selector: "html".into(),
                    tag: "html".into(),
                    attrs: html_attrs,
                    computed_styles: IndexMap::new(),
                    rect: Some(Rect {
                        x: 0,
                        y: 0,
                        width: 1280,
                        height: 800,
                    }),
                    parent: None,
                    children: vec![1, 2],
                },
                SnapshotNode {
                    dom_order: 1,
                    selector: "html > head".into(),
                    tag: "head".into(),
                    attrs: IndexMap::new(),
                    computed_styles: IndexMap::new(),
                    rect: None,
                    parent: Some(0),
                    children: vec![],
                },
                SnapshotNode {
                    dom_order: 2,
                    selector: "html > body".into(),
                    tag: "body".into(),
                    attrs: IndexMap::new(),
                    computed_styles: body_styles,
                    rect: Some(Rect {
                        x: 0,
                        y: 0,
                        width: 1280,
                        height: 800,
                    }),
                    parent: Some(0),
                    children: vec![],
                },
            ],
        }
    }
}

/// A borrowed view over a snapshot, handed to rules during evaluation.
///
/// Keeping this a distinct type (rather than handing `&PlumbSnapshot`
/// directly) lets us extend the engine with cross-cutting context (e.g.
/// precomputed selector indexes) without breaking the [`crate::rules::Rule`] trait.
#[derive(Debug)]
pub struct SnapshotCtx<'a> {
    snapshot: &'a PlumbSnapshot,
    viewports: Vec<ViewportKey>,
    rects_by_dom_order: IndexMap<u64, Rect>,
    /// Maps `dom_order` → `(start_index, count)` into `snapshot.text_boxes`.
    text_box_ranges: IndexMap<u64, (usize, usize)>,
}

impl<'a> SnapshotCtx<'a> {
    /// Wrap a borrowed snapshot.
    #[must_use]
    pub fn new(snapshot: &'a PlumbSnapshot) -> Self {
        Self::with_viewports(snapshot, [snapshot.viewport.clone()])
    }

    /// Wrap a borrowed snapshot with the full viewport set for this run.
    ///
    /// The caller-provided order is preserved.
    #[must_use]
    pub fn with_viewports(
        snapshot: &'a PlumbSnapshot,
        viewports: impl IntoIterator<Item = ViewportKey>,
    ) -> Self {
        Self {
            snapshot,
            viewports: viewports.into_iter().collect(),
            rects_by_dom_order: rect_index(snapshot),
            text_box_ranges: text_box_index(snapshot),
        }
    }

    /// The underlying snapshot.
    #[must_use]
    pub fn snapshot(&self) -> &'a PlumbSnapshot {
        self.snapshot
    }

    /// The viewports included in the current engine run.
    #[must_use]
    pub fn viewports(&self) -> &[ViewportKey] {
        &self.viewports
    }

    /// Return the bounding rect for a node by document-order index.
    #[must_use]
    pub fn rect_for(&self, dom_order: u64) -> Option<Rect> {
        self.rects_by_dom_order.get(&dom_order).copied()
    }

    /// Return text boxes for a node by document-order index.
    ///
    /// Returns an empty slice when no text boxes exist for `dom_order`.
    #[must_use]
    pub fn text_boxes_for(&self, dom_order: u64) -> &[TextBox] {
        match self.text_box_ranges.get(&dom_order) {
            Some(&(start, count)) => &self.snapshot.text_boxes[start..start + count],
            None => &[],
        }
    }

    /// Iterate nodes in document order.
    pub fn nodes(&self) -> impl Iterator<Item = &SnapshotNode> {
        self.snapshot.nodes.iter()
    }
}

fn rect_index(snapshot: &PlumbSnapshot) -> IndexMap<u64, Rect> {
    snapshot
        .nodes
        .iter()
        .filter_map(|node| node.rect.map(|rect| (node.dom_order, rect)))
        .collect()
}

/// Build a `(dom_order → (start_idx, count))` index over text boxes.
///
/// Requires `text_boxes` to be sorted by `(dom_order, start)`.
fn text_box_index(snapshot: &PlumbSnapshot) -> IndexMap<u64, (usize, usize)> {
    debug_assert!(
        snapshot
            .text_boxes
            .windows(2)
            .all(|w| { (w[0].dom_order, w[0].start) <= (w[1].dom_order, w[1].start) }),
        "text_boxes must be sorted by (dom_order, start)"
    );
    let mut index: IndexMap<u64, (usize, usize)> = IndexMap::new();
    let boxes = &snapshot.text_boxes;
    let mut i = 0;
    while i < boxes.len() {
        let dom_order = boxes[i].dom_order;
        let start = i;
        while i < boxes.len() && boxes[i].dom_order == dom_order {
            i += 1;
        }
        index.insert(dom_order, (start, i - start));
    }
    index
}
