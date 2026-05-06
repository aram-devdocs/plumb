//! Pure deterministic viewport-comparison for the `compare_viewports` MCP tool.
//!
//! Given two or more [`PlumbSnapshot`]s of the same URL captured at
//! different viewports, this module produces a sorted list of
//! per-node diffs: nodes missing on one side, size changes that exceed
//! a configurable pixel threshold, document-order reordering, and
//! computed-style differences.
//!
//! No I/O, no async, no wall-clock. The output is a pure function of
//! the inputs and is byte-identical across runs.

// Items are intentionally `pub(crate)` so the parent `lib.rs` can
// invoke them. The module is private; `unreachable_pub` would flag
// these without the explicit visibility, and clippy's
// `redundant_pub_crate` would flag the visibility as redundant. We
// match the pattern used in `explain.rs`.
#![allow(clippy::redundant_pub_crate)]

use std::collections::BTreeMap;

use plumb_core::PlumbSnapshot;
use plumb_core::snapshot::SnapshotNode;
use serde::Serialize;
use serde_json::Value;

/// Default pixel threshold for size-change diffs.
///
/// A node is reported as "size changed" only when either its width or
/// its height differs by more than this many CSS pixels between two
/// viewports. Sub-pixel reflows from font metrics or rounding fall
/// below the threshold by design.
pub(crate) const DEFAULT_SIZE_THRESHOLD_PX: u32 = 4;

/// Hard cap on the number of diff entries returned in
/// `structuredContent`. Aggregation happens server-side: the payload
/// always reports the full counts in `summary`, but the `diffs` list
/// is capped to keep agents under the 10 KB structured-content budget.
const DIFF_ENTRY_CAP: usize = 200;

/// Computed-style properties tracked for cross-viewport diffs.
///
/// Limited to the set with the highest signal-to-noise for
/// mobile/desktop regressions. Adding properties here grows the diff
/// payload, so any addition needs to be justified against the 10 KB
/// budget.
const TRACKED_STYLE_PROPERTIES: &[&str] = &[
    "display",
    "flex-direction",
    "grid-template-columns",
    "font-size",
    "color",
    "background-color",
    "visibility",
    "position",
];

/// Kind of diff entry. Encoded as a discriminator string so JSON
/// consumers can switch on it without parsing field combinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DiffKind {
    /// Node exists in some viewports but not others.
    Missing,
    /// Node bounding-box size changed by more than the threshold.
    SizeChange,
    /// Node `dom_order` differs across viewports.
    Reordered,
    /// One of the tracked computed-style properties differs.
    StyleChange,
}

impl DiffKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::SizeChange => "size_change",
            Self::Reordered => "reordered",
            Self::StyleChange => "style_change",
        }
    }
}

/// Aggregate counts shown to the caller alongside the diff list.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub(crate) struct DiffSummary {
    /// Total number of diff entries discovered (before capping).
    pub(crate) total: usize,
    /// Nodes present in some viewports but missing in others.
    pub(crate) missing: usize,
    /// Nodes whose bounding box differs above the threshold.
    pub(crate) size_changes: usize,
    /// Nodes whose `dom_order` differs across viewports.
    pub(crate) reordered: usize,
    /// Nodes whose tracked computed styles differ.
    pub(crate) style_changes: usize,
}

/// A single diff entry. Field availability depends on `kind`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct DiffEntry {
    kind: DiffKind,
    selector: String,
    /// Property name for `StyleChange`, otherwise empty.
    property: String,
    /// Viewports the node is present in (for `Missing`); both
    /// viewports for binary diffs (otherwise sorted).
    present_in: Vec<String>,
    absent_in: Vec<String>,
    viewport_a: String,
    viewport_b: String,
    value_a: String,
    value_b: String,
    width_a: u32,
    height_a: u32,
    width_b: u32,
    height_b: u32,
    delta_px: u32,
    dom_order_a: u64,
    dom_order_b: u64,
}

impl DiffEntry {
    fn into_json(self) -> Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "kind".to_string(),
            Value::String(self.kind.as_str().to_string()),
        );
        map.insert("selector".to_string(), Value::String(self.selector));
        match self.kind {
            DiffKind::Missing => {
                map.insert(
                    "present_in".to_string(),
                    Value::Array(self.present_in.into_iter().map(Value::String).collect()),
                );
                map.insert(
                    "absent_in".to_string(),
                    Value::Array(self.absent_in.into_iter().map(Value::String).collect()),
                );
            }
            DiffKind::SizeChange => {
                map.insert("viewport_a".to_string(), Value::String(self.viewport_a));
                map.insert("viewport_b".to_string(), Value::String(self.viewport_b));
                map.insert("width_a".to_string(), Value::Number(self.width_a.into()));
                map.insert("height_a".to_string(), Value::Number(self.height_a.into()));
                map.insert("width_b".to_string(), Value::Number(self.width_b.into()));
                map.insert("height_b".to_string(), Value::Number(self.height_b.into()));
                map.insert("delta_px".to_string(), Value::Number(self.delta_px.into()));
            }
            DiffKind::Reordered => {
                map.insert("viewport_a".to_string(), Value::String(self.viewport_a));
                map.insert("viewport_b".to_string(), Value::String(self.viewport_b));
                map.insert(
                    "dom_order_a".to_string(),
                    Value::Number(self.dom_order_a.into()),
                );
                map.insert(
                    "dom_order_b".to_string(),
                    Value::Number(self.dom_order_b.into()),
                );
            }
            DiffKind::StyleChange => {
                map.insert("property".to_string(), Value::String(self.property));
                map.insert("viewport_a".to_string(), Value::String(self.viewport_a));
                map.insert("viewport_b".to_string(), Value::String(self.viewport_b));
                map.insert("value_a".to_string(), Value::String(self.value_a));
                map.insert("value_b".to_string(), Value::String(self.value_b));
            }
        }
        Value::Object(map)
    }
}

/// Result of comparing two-or-more snapshots.
pub(crate) struct CompareResult {
    /// Aggregate counts (counts every diff, even those clipped by the cap).
    pub(crate) summary: DiffSummary,
    /// Sorted diff entries, capped at [`DIFF_ENTRY_CAP`].
    pub(crate) diffs: Vec<Value>,
    /// `true` when `summary.total > diffs.len()` — the caller may want
    /// to surface "+N more" in their text block.
    pub(crate) truncated: bool,
}

/// Compare two-or-more snapshots and produce a deterministic delta.
///
/// `snapshots` MUST be aligned with `viewport_names` — the i-th entry
/// describes the i-th viewport. The caller is expected to have already
/// validated that there are at least two viewports.
///
/// Determinism: outputs depend only on the inputs. Ordering is by
/// `(kind, selector, property, viewport_a, viewport_b)`.
#[must_use]
#[allow(clippy::too_many_lines)]
pub(crate) fn compare_snapshots(
    snapshots: &[PlumbSnapshot],
    viewport_names: &[String],
    size_threshold_px: u32,
) -> CompareResult {
    debug_assert_eq!(snapshots.len(), viewport_names.len());

    // Build per-viewport per-selector indexes. BTreeMap so iteration
    // is deterministic regardless of node insertion order.
    let mut by_selector: BTreeMap<&str, Vec<Option<NodeView<'_>>>> = BTreeMap::new();
    for (i, snap) in snapshots.iter().enumerate() {
        for node in &snap.nodes {
            let entry = by_selector.entry(node.selector.as_str()).or_insert_with(
                || -> Vec<Option<NodeView<'_>>> {
                    let mut v = Vec::with_capacity(snapshots.len());
                    for _ in 0..snapshots.len() {
                        v.push(None);
                    }
                    v
                },
            );
            entry[i] = Some(NodeView::from_node(node));
        }
    }

    let mut entries: Vec<DiffEntry> = Vec::new();
    let mut summary = DiffSummary::default();

    for (selector, views) in &by_selector {
        // Missing-in-viewport detection: any selector not present in at
        // least one viewport but present in another.
        let present: Vec<String> = views
            .iter()
            .zip(viewport_names.iter())
            .filter_map(|(view, name)| view.as_ref().map(|_| name.clone()))
            .collect();
        let absent: Vec<String> = views
            .iter()
            .zip(viewport_names.iter())
            .filter_map(|(view, name)| match view {
                None => Some(name.clone()),
                Some(_) => None,
            })
            .collect();

        if !absent.is_empty() {
            summary.missing += 1;
            entries.push(DiffEntry {
                kind: DiffKind::Missing,
                selector: (*selector).to_string(),
                present_in: present.clone(),
                absent_in: absent.clone(),
                ..DiffEntry::blank()
            });
            // Skip downstream binary comparisons for this selector
            // when it isn't present in every viewport — the missing
            // entry already captures the divergence.
            continue;
        }

        // Pairwise comparison against viewport 0 (the "baseline").
        // For 2 viewports this collapses to a single A↔B comparison.
        // For N viewports it keeps the diff count linear in N rather
        // than quadratic.
        let baseline_idx = 0_usize;
        let Some(baseline) = views[baseline_idx].as_ref() else {
            continue;
        };

        for (i, other) in views.iter().enumerate().skip(1) {
            let Some(other) = other.as_ref() else {
                continue;
            };

            // Reorder
            if baseline.dom_order != other.dom_order {
                summary.reordered += 1;
                entries.push(DiffEntry {
                    kind: DiffKind::Reordered,
                    selector: (*selector).to_string(),
                    viewport_a: viewport_names[baseline_idx].clone(),
                    viewport_b: viewport_names[i].clone(),
                    dom_order_a: baseline.dom_order,
                    dom_order_b: other.dom_order,
                    ..DiffEntry::blank()
                });
            }

            // Size change
            if let (Some(a), Some(b)) = (baseline.size, other.size) {
                let dw = a.0.abs_diff(b.0);
                let dh = a.1.abs_diff(b.1);
                let delta = dw.max(dh);
                if delta > size_threshold_px {
                    summary.size_changes += 1;
                    entries.push(DiffEntry {
                        kind: DiffKind::SizeChange,
                        selector: (*selector).to_string(),
                        viewport_a: viewport_names[baseline_idx].clone(),
                        viewport_b: viewport_names[i].clone(),
                        width_a: a.0,
                        height_a: a.1,
                        width_b: b.0,
                        height_b: b.1,
                        delta_px: delta,
                        ..DiffEntry::blank()
                    });
                }
            }

            // Style changes — only over the tracked property allowlist.
            for property in TRACKED_STYLE_PROPERTIES {
                let value_a = baseline.style(property);
                let value_b = other.style(property);
                if value_a != value_b
                    && let (Some(va), Some(vb)) = (value_a, value_b)
                {
                    summary.style_changes += 1;
                    entries.push(DiffEntry {
                        kind: DiffKind::StyleChange,
                        selector: (*selector).to_string(),
                        property: (*property).to_string(),
                        viewport_a: viewport_names[baseline_idx].clone(),
                        viewport_b: viewport_names[i].clone(),
                        value_a: va.to_string(),
                        value_b: vb.to_string(),
                        ..DiffEntry::blank()
                    });
                }
            }
        }
    }

    summary.total =
        summary.missing + summary.size_changes + summary.reordered + summary.style_changes;

    entries.sort();

    let truncated = entries.len() > DIFF_ENTRY_CAP;
    if truncated {
        entries.truncate(DIFF_ENTRY_CAP);
    }

    let diffs: Vec<Value> = entries.into_iter().map(DiffEntry::into_json).collect();

    CompareResult {
        summary,
        diffs,
        truncated,
    }
}

#[derive(Debug)]
struct NodeView<'a> {
    dom_order: u64,
    size: Option<(u32, u32)>,
    node: &'a SnapshotNode,
}

impl<'a> NodeView<'a> {
    fn from_node(node: &'a SnapshotNode) -> Self {
        Self {
            dom_order: node.dom_order,
            size: node.rect.map(|r| (r.width, r.height)),
            node,
        }
    }

    fn style(&self, property: &str) -> Option<&str> {
        self.node.computed_styles.get(property).map(String::as_str)
    }
}

impl DiffEntry {
    fn blank() -> Self {
        Self {
            kind: DiffKind::Missing,
            selector: String::new(),
            property: String::new(),
            present_in: Vec::new(),
            absent_in: Vec::new(),
            viewport_a: String::new(),
            viewport_b: String::new(),
            value_a: String::new(),
            value_b: String::new(),
            width_a: 0,
            height_a: 0,
            width_b: 0,
            height_b: 0,
            delta_px: 0,
            dom_order_a: 0,
            dom_order_b: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use indexmap::IndexMap;
    use plumb_core::snapshot::SnapshotNode;
    use plumb_core::{PlumbSnapshot, Rect, ViewportKey};

    use super::*;

    fn snap(viewport: &str, width: u32, nodes: Vec<SnapshotNode>) -> PlumbSnapshot {
        PlumbSnapshot {
            url: "plumb-fake://hello".into(),
            viewport: ViewportKey::new(viewport),
            viewport_width: width,
            viewport_height: 800,
            nodes,
            text_boxes: Vec::new(),
        }
    }

    fn node(
        dom_order: u64,
        selector: &str,
        rect: Option<Rect>,
        styles: &[(&str, &str)],
    ) -> SnapshotNode {
        let mut computed = IndexMap::new();
        for (k, v) in styles {
            computed.insert((*k).to_string(), (*v).to_string());
        }
        SnapshotNode {
            dom_order,
            selector: selector.into(),
            tag: selector
                .rsplit_once('>')
                .map_or_else(|| selector.to_string(), |(_, tail)| tail.trim().to_string()),
            attrs: IndexMap::new(),
            computed_styles: computed,
            rect,
            parent: None,
            children: Vec::new(),
        }
    }

    fn rect(w: u32, h: u32) -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: w,
            height: h,
        }
    }

    #[test]
    fn identical_snapshots_yield_no_diffs() {
        let a = snap(
            "mobile",
            375,
            vec![
                node(0, "html", Some(rect(375, 800)), &[]),
                node(1, "html > body", Some(rect(375, 800)), &[("color", "red")]),
            ],
        );
        let b = snap(
            "desktop",
            1280,
            vec![
                node(0, "html", Some(rect(375, 800)), &[]),
                node(1, "html > body", Some(rect(375, 800)), &[("color", "red")]),
            ],
        );
        let result = compare_snapshots(
            &[a, b],
            &["mobile".into(), "desktop".into()],
            DEFAULT_SIZE_THRESHOLD_PX,
        );
        assert_eq!(result.summary.total, 0);
        assert!(result.diffs.is_empty());
        assert!(!result.truncated);
    }

    #[test]
    fn missing_node_at_one_viewport_emits_missing_diff() {
        let mobile = snap(
            "mobile",
            375,
            vec![node(0, "html", Some(rect(375, 800)), &[])],
        );
        let desktop = snap(
            "desktop",
            1280,
            vec![
                node(0, "html", Some(rect(1280, 800)), &[]),
                node(1, "html > nav", Some(rect(1280, 60)), &[]),
            ],
        );
        let result = compare_snapshots(
            &[mobile, desktop],
            &["mobile".into(), "desktop".into()],
            DEFAULT_SIZE_THRESHOLD_PX,
        );
        // One missing (nav) + one size change (html) — html went from
        // 375 wide to 1280 wide.
        assert_eq!(result.summary.missing, 1);
        assert!(
            result
                .diffs
                .iter()
                .any(|d| d["kind"].as_str() == Some("missing"))
        );
        let missing = result
            .diffs
            .iter()
            .find(|d| d["kind"] == "missing")
            .unwrap();
        assert_eq!(missing["selector"], "html > nav");
        assert_eq!(missing["present_in"], serde_json::json!(["desktop"]));
        assert_eq!(missing["absent_in"], serde_json::json!(["mobile"]));
    }

    #[test]
    fn size_change_below_threshold_is_ignored() {
        let mobile = snap(
            "mobile",
            375,
            vec![node(0, "html > body", Some(rect(100, 100)), &[])],
        );
        let desktop = snap(
            "desktop",
            1280,
            vec![node(0, "html > body", Some(rect(102, 100)), &[])],
        );
        let result = compare_snapshots(
            &[mobile, desktop],
            &["mobile".into(), "desktop".into()],
            DEFAULT_SIZE_THRESHOLD_PX,
        );
        assert_eq!(result.summary.size_changes, 0);
    }

    #[test]
    fn size_change_above_threshold_is_reported() {
        let mobile = snap(
            "mobile",
            375,
            vec![node(0, "html > body", Some(rect(100, 100)), &[])],
        );
        let desktop = snap(
            "desktop",
            1280,
            vec![node(0, "html > body", Some(rect(200, 100)), &[])],
        );
        let result = compare_snapshots(
            &[mobile, desktop],
            &["mobile".into(), "desktop".into()],
            DEFAULT_SIZE_THRESHOLD_PX,
        );
        assert_eq!(result.summary.size_changes, 1);
        let entry = &result.diffs[0];
        assert_eq!(entry["kind"], "size_change");
        assert_eq!(entry["delta_px"], 100);
    }

    #[test]
    fn dom_reorder_is_reported() {
        let mobile = snap(
            "mobile",
            375,
            vec![
                node(0, "html > body > a", Some(rect(50, 30)), &[]),
                node(1, "html > body > b", Some(rect(50, 30)), &[]),
            ],
        );
        let desktop = snap(
            "desktop",
            1280,
            vec![
                node(0, "html > body > b", Some(rect(50, 30)), &[]),
                node(1, "html > body > a", Some(rect(50, 30)), &[]),
            ],
        );
        let result = compare_snapshots(
            &[mobile, desktop],
            &["mobile".into(), "desktop".into()],
            DEFAULT_SIZE_THRESHOLD_PX,
        );
        assert_eq!(result.summary.reordered, 2);
    }

    #[test]
    fn style_change_on_tracked_property_is_reported() {
        let mobile = snap(
            "mobile",
            375,
            vec![node(
                0,
                "html > body",
                Some(rect(375, 800)),
                &[("display", "block")],
            )],
        );
        let desktop = snap(
            "desktop",
            1280,
            vec![node(
                0,
                "html > body",
                Some(rect(375, 800)),
                &[("display", "flex")],
            )],
        );
        let result = compare_snapshots(
            &[mobile, desktop],
            &["mobile".into(), "desktop".into()],
            DEFAULT_SIZE_THRESHOLD_PX,
        );
        assert_eq!(result.summary.style_changes, 1);
        let entry = &result.diffs[0];
        assert_eq!(entry["kind"], "style_change");
        assert_eq!(entry["property"], "display");
        assert_eq!(entry["value_a"], "block");
        assert_eq!(entry["value_b"], "flex");
    }

    #[test]
    fn output_is_byte_identical_across_runs() {
        let a = snap(
            "mobile",
            375,
            vec![
                node(0, "html", Some(rect(375, 800)), &[]),
                node(1, "html > body", Some(rect(375, 800)), &[("color", "red")]),
                node(2, "html > body > nav", Some(rect(375, 60)), &[]),
            ],
        );
        let b = snap(
            "desktop",
            1280,
            vec![
                node(0, "html", Some(rect(1280, 800)), &[]),
                node(
                    1,
                    "html > body",
                    Some(rect(1280, 800)),
                    &[("color", "blue")],
                ),
            ],
        );

        let viewport_names = vec!["mobile".to_string(), "desktop".to_string()];
        let r1 = compare_snapshots(
            &[a.clone(), b.clone()],
            &viewport_names,
            DEFAULT_SIZE_THRESHOLD_PX,
        );
        let r2 = compare_snapshots(
            &[a.clone(), b.clone()],
            &viewport_names,
            DEFAULT_SIZE_THRESHOLD_PX,
        );
        let r3 = compare_snapshots(&[a, b], &viewport_names, DEFAULT_SIZE_THRESHOLD_PX);
        let s1 = serde_json::to_string(&r1.diffs).unwrap();
        let s2 = serde_json::to_string(&r2.diffs).unwrap();
        let s3 = serde_json::to_string(&r3.diffs).unwrap();
        assert_eq!(s1, s2);
        assert_eq!(s2, s3);
    }
}
