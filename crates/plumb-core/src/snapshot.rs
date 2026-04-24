//! In-memory snapshot of a rendered page at a single viewport.
//!
//! The real `PlumbSnapshot` is populated by `plumb-cdp` via the Chromium
//! DevTools Protocol. For the walking skeleton and tests, a canned
//! constructor is available behind the `test-fake` feature.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::report::{Rect, ViewportKey};

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
        body_styles.insert("margin".into(), "0".into());
        body_styles.insert("padding".into(), "13px".into()); // odd — the placeholder rule keys off this

        Self {
            url: "plumb-fake://hello".into(),
            viewport: ViewportKey::new("desktop"),
            viewport_width: 1280,
            viewport_height: 800,
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
/// precomputed selector indexes) without breaking the [`Rule`] trait.
#[derive(Debug)]
pub struct SnapshotCtx<'a> {
    snapshot: &'a PlumbSnapshot,
}

impl<'a> SnapshotCtx<'a> {
    /// Wrap a borrowed snapshot.
    #[must_use]
    pub fn new(snapshot: &'a PlumbSnapshot) -> Self {
        Self { snapshot }
    }

    /// The underlying snapshot.
    #[must_use]
    pub fn snapshot(&self) -> &'a PlumbSnapshot {
        self.snapshot
    }

    /// Iterate nodes in document order.
    pub fn nodes(&self) -> impl Iterator<Item = &SnapshotNode> {
        self.snapshot.nodes.iter()
    }
}
