//! `sibling/height-consistency` — flag sibling elements whose height
//! drifts from the row's median.
//!
//! ## Heuristic
//!
//! 1. Group nodes by `parent` `dom_order`.
//! 2. Within each parent, cluster the siblings into "visual rows":
//!    two siblings share a row when their `top` edges are within
//!    `ROW_TOP_TOLERANCE_PX` AND their bounding rects overlap
//!    horizontally by at least 50% of the smaller width.
//!    The clustering walks siblings in DOM order and assigns each one
//!    to the first row it fits, opening a new row otherwise.
//! 3. If every sibling ends up in its own row, fall back to a single
//!    DOM-sibling group — this catches cases like absolutely-positioned
//!    cards where the row geometry doesn't pan out.
//! 4. For each row of size ≥ 2, compute the median height. Any element
//!    whose height deviates from the median by more than
//!    `HEIGHT_DEVIATION_PX` fires a violation.
//!
//! Sibling iteration uses `parent` `dom_order` rather than the full
//! DOM tree, so the rule only ever fires once per offending node per
//! viewport.

use indexmap::IndexMap;

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Rect, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::snapshot::{SnapshotCtx, SnapshotNode};

/// Maximum vertical offset (in CSS pixels) between two sibling tops
/// that still counts as the "same row".
const ROW_TOP_TOLERANCE_PX: i32 = 2;

/// Heights this far from the row median (in CSS pixels) trigger a
/// violation. Smaller drift is treated as subpixel noise.
const HEIGHT_DEVIATION_PX: u32 = 4;

/// Minimum horizontal overlap (as a fraction of the smaller width) for
/// two siblings to share a row.
const MIN_HORIZONTAL_OVERLAP: f64 = 0.5;

/// Flags sibling elements in the same visual row whose heights drift
/// from the row's median.
#[derive(Debug, Clone, Copy)]
pub struct HeightConsistency;

impl Rule for HeightConsistency {
    fn id(&self) -> &'static str {
        "sibling/height-consistency"
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn summary(&self) -> &'static str {
        "Flags sibling elements in the same visual row whose heights drift from the row's median."
    }

    fn check(&self, ctx: &SnapshotCtx<'_>, _config: &Config, sink: &mut ViolationSink<'_>) {
        // Group siblings by `parent`. Siblings with no rect are skipped
        // — height clustering needs geometry.
        let mut groups: IndexMap<u64, Vec<SiblingEntry<'_>>> = IndexMap::new();
        for node in ctx.nodes() {
            let Some(parent) = node.parent else { continue };
            let Some(rect) = ctx.rect_for(node.dom_order) else {
                continue;
            };
            groups
                .entry(parent)
                .or_default()
                .push(SiblingEntry { node, rect });
        }

        for siblings in groups.values() {
            if siblings.len() < 2 {
                continue;
            }
            let rows = cluster_into_rows(siblings);
            for row in &rows {
                emit_for_row(self.id(), self.default_severity(), ctx, row, sink);
            }
        }
    }
}

/// One sibling, paired with its rect for cheap geometry math.
#[derive(Debug, Clone, Copy)]
struct SiblingEntry<'a> {
    node: &'a SnapshotNode,
    rect: Rect,
}

/// Cluster siblings into visual rows.
///
/// Walks in DOM order; each entry joins the first existing row whose
/// representative shares its top (within tolerance) and overlaps it
/// horizontally by ≥ [`MIN_HORIZONTAL_OVERLAP`]. Otherwise a new row
/// opens.
///
/// If clustering produces only singleton rows, fall back to a single
/// DOM-sibling group. The fallback keeps the rule useful for layouts
/// where row geometry is unreliable (absolute positioning, transforms)
/// while the median-deviation check still rejects single-element groups.
fn cluster_into_rows<'a>(siblings: &[SiblingEntry<'a>]) -> Vec<Vec<SiblingEntry<'a>>> {
    let mut rows: Vec<Vec<SiblingEntry<'a>>> = Vec::new();
    for entry in siblings {
        let mut placed = false;
        for row in &mut rows {
            // Compare against the row's first member — a stable
            // representative since rows grow in DOM order.
            if let Some(first) = row.first()
                && shares_row(first, entry)
            {
                row.push(*entry);
                placed = true;
                break;
            }
        }
        if !placed {
            rows.push(vec![*entry]);
        }
    }

    let any_multi = rows.iter().any(|row| row.len() >= 2);
    if any_multi {
        rows
    } else {
        // Fallback: treat every sibling as one DOM group.
        vec![siblings.to_vec()]
    }
}

/// Whether two siblings share a row.
fn shares_row(a: &SiblingEntry<'_>, b: &SiblingEntry<'_>) -> bool {
    if (a.rect.y - b.rect.y).abs() > ROW_TOP_TOLERANCE_PX {
        return false;
    }
    horizontal_overlap_fraction(&a.rect, &b.rect) >= MIN_HORIZONTAL_OVERLAP
}

/// Fraction of the smaller width covered by the horizontal intersection.
fn horizontal_overlap_fraction(a: &Rect, b: &Rect) -> f64 {
    let a_left = a.x;
    let b_left = b.x;
    let a_right = a.x.saturating_add_unsigned(a.width);
    let b_right = b.x.saturating_add_unsigned(b.width);

    let overlap_left = a_left.max(b_left);
    let overlap_right = a_right.min(b_right);
    let overlap = (overlap_right - overlap_left).max(0);
    let smaller_width = a.width.min(b.width);
    if smaller_width == 0 {
        return 0.0;
    }
    f64::from(overlap) / f64::from(smaller_width)
}

/// Emit a violation for every member of `row` whose height deviates
/// from the row's median by more than `HEIGHT_DEVIATION_PX`.
fn emit_for_row(
    rule_id: &str,
    severity: Severity,
    ctx: &SnapshotCtx<'_>,
    row: &[SiblingEntry<'_>],
    sink: &mut ViolationSink<'_>,
) {
    if row.len() < 2 {
        return;
    }
    let median = median_height(row);
    for entry in row {
        let dev = entry.rect.height.abs_diff(median);
        if dev <= HEIGHT_DEVIATION_PX {
            continue;
        }
        let mut metadata: IndexMap<String, serde_json::Value> = IndexMap::new();
        metadata.insert("rendered_height_px".to_owned(), entry.rect.height.into());
        metadata.insert("row_median_height_px".to_owned(), median.into());
        metadata.insert("row_size".to_owned(), row.len().into());
        metadata.insert("deviation_px".to_owned(), dev.into());

        sink.push(Violation {
            rule_id: rule_id.to_owned(),
            severity,
            message: format!(
                "`{selector}` is {h}px tall; its row median is {median}px ({dev}px drift).",
                selector = entry.node.selector,
                h = entry.rect.height,
            ),
            selector: entry.node.selector.clone(),
            viewport: ctx.snapshot().viewport.clone(),
            rect: Some(entry.rect),
            dom_order: entry.node.dom_order,
            fix: Some(Fix {
                kind: FixKind::Description {
                    text: format!(
                        "Match the row's height ({median}px) by adjusting `height` / `min-height` or aligning the inner content. Drift: {dev}px."
                    ),
                },
                description: format!(
                    "Bring `{selector}` in line with its row's height ({median}px).",
                    selector = entry.node.selector,
                ),
                confidence: Confidence::Low,
            }),
            doc_url: "https://plumb.aramhammoudeh.com/rules/sibling-height-consistency".to_owned(),
            metadata,
        });
    }
}

/// Median height across a row's entries.
///
/// `row` is non-empty by construction (caller guards with `len < 2`).
/// For an even count, the lower of the two middle values wins — a
/// deterministic, integer-only choice that matches "snap toward the
/// shorter neighbour" rather than introducing floating-point math.
fn median_height(row: &[SiblingEntry<'_>]) -> u32 {
    let mut heights: Vec<u32> = row.iter().map(|e| e.rect.height).collect();
    heights.sort_unstable();
    let mid = heights.len() / 2;
    if heights.len().is_multiple_of(2) {
        heights[mid - 1]
    } else {
        heights[mid]
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HEIGHT_DEVIATION_PX, ROW_TOP_TOLERANCE_PX, SiblingEntry, cluster_into_rows,
        horizontal_overlap_fraction, median_height,
    };
    use crate::report::Rect;
    use crate::snapshot::SnapshotNode;
    use indexmap::IndexMap;

    fn make_node(dom_order: u64) -> SnapshotNode {
        SnapshotNode {
            dom_order,
            selector: format!("n{dom_order}"),
            tag: "div".to_owned(),
            attrs: IndexMap::new(),
            computed_styles: IndexMap::new(),
            rect: None,
            parent: Some(0),
            children: Vec::new(),
        }
    }

    fn rect_at(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn horizontal_overlap_smoke() {
        let a = rect_at(0, 0, 100, 10);
        let b = rect_at(50, 0, 100, 10);
        // 50px overlap / min(100, 100) = 0.5 → exactly the threshold.
        assert!((horizontal_overlap_fraction(&a, &b) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn median_picks_lower_middle_for_even_count() {
        let nodes: Vec<SnapshotNode> = (0..4).map(make_node).collect();
        let row: Vec<SiblingEntry<'_>> = nodes
            .iter()
            .zip([10_u32, 20, 30, 40])
            .map(|(node, h)| SiblingEntry {
                node,
                rect: rect_at(0, 0, 10, h),
            })
            .collect();
        // Sorted heights are [10, 20, 30, 40]; lower-middle = 20.
        assert_eq!(median_height(&row), 20);
    }

    #[test]
    fn cluster_groups_siblings_with_close_tops() {
        // Two entries sit in row 1 (y=0 / y=1, with full horizontal
        // overlap on a stacked-width column). A third entry drops to
        // y=100 and forms its own row. The clusterer should produce
        // two rows of sizes 2 and 1.
        let nodes: Vec<SnapshotNode> = (1_u64..=3).map(make_node).collect();
        let entries: Vec<SiblingEntry<'_>> = vec![
            SiblingEntry {
                node: &nodes[0],
                rect: rect_at(0, 0, 100, 30),
            },
            SiblingEntry {
                node: &nodes[1],
                rect: rect_at(20, 1, 100, 40),
            },
            SiblingEntry {
                node: &nodes[2],
                rect: rect_at(0, 100, 100, 30),
            },
        ];
        let clusters = cluster_into_rows(&entries);
        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters[0].len(), 2);
        assert_eq!(clusters[1].len(), 1);
    }

    #[test]
    fn cluster_falls_back_when_no_row_pairs() {
        // Three siblings stacked vertically — no two share a row.
        let nodes: Vec<SnapshotNode> = (1_u64..=3).map(make_node).collect();
        let entries: Vec<SiblingEntry<'_>> = vec![
            SiblingEntry {
                node: &nodes[0],
                rect: rect_at(0, 0, 100, 30),
            },
            SiblingEntry {
                node: &nodes[1],
                rect: rect_at(0, 100, 100, 40),
            },
            SiblingEntry {
                node: &nodes[2],
                rect: rect_at(0, 200, 100, 30),
            },
        ];
        let clusters = cluster_into_rows(&entries);
        // Fallback: a single DOM-sibling group.
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].len(), 3);
    }

    #[test]
    fn constants_are_what_the_docs_say() {
        // Pin the documented thresholds so doc drift is caught at
        // compile time.
        assert_eq!(ROW_TOP_TOLERANCE_PX, 2);
        assert_eq!(HEIGHT_DEVIATION_PX, 4);
    }
}
