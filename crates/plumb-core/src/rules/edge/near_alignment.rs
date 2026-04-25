//! `edge/near-alignment` — flag sibling edges that almost-but-not-quite
//! line up.
//!
//! ## Heuristic
//!
//! For each parent group of siblings (with rects), the rule processes
//! the four edge axes independently — `left`, `right`, `top`, `bottom`
//! — and runs a greedy 1-D clustering pass on each:
//!
//! 1. Sort the parent group's edge values.
//! 2. Walk the sorted list; an edge joins the active cluster when it
//!    is within `alignment.tolerance_px` of the cluster's lowest
//!    member, otherwise it opens a new cluster.
//! 3. For each cluster of ≥ 2 members, compute the integer mean
//!    (truncated; `sum / len`).
//! 4. Any member whose distance from the centroid is **strictly
//!    positive** AND **at most `tolerance_px`** fires a violation.
//!    Pixel-perfect alignments (delta == 0) are deliberately silent.
//!
//! Each rule pass emits at most one violation per (node, axis) pair;
//! a node with several near-aligned edges may be flagged once per axis.

use indexmap::IndexMap;

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Rect, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::snapshot::{SnapshotCtx, SnapshotNode};

/// One of the four edge axes the rule inspects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Left,
    Right,
    Top,
    Bottom,
}

impl Axis {
    /// All four axes, in the order the rule processes them.
    const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Top, Self::Bottom];

    /// Lowercase identifier used in violation messages and metadata.
    const fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }

    /// Edge value for a given rect, in CSS pixels.
    fn edge(self, rect: Rect) -> i32 {
        match self {
            Self::Left => rect.x,
            Self::Right => rect.x.saturating_add_unsigned(rect.width),
            Self::Top => rect.y,
            Self::Bottom => rect.y.saturating_add_unsigned(rect.height),
        }
    }
}

/// Flags element edges that almost-but-not-quite line up with sibling
/// edges.
#[derive(Debug, Clone, Copy)]
pub struct NearAlignment;

impl Rule for NearAlignment {
    fn id(&self) -> &'static str {
        "edge/near-alignment"
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn summary(&self) -> &'static str {
        "Flags element edges that almost-but-not-quite line up with sibling edges."
    }

    fn check(&self, ctx: &SnapshotCtx<'_>, config: &Config, sink: &mut ViolationSink<'_>) {
        let tolerance = config.alignment.tolerance_px;
        if tolerance == 0 {
            // No tolerance configured — every miss is "perfect or
            // off"; the rule has nothing to say.
            return;
        }

        let mut groups: IndexMap<u64, Vec<EdgeEntry<'_>>> = IndexMap::new();
        for node in ctx.nodes() {
            let Some(parent) = node.parent else { continue };
            let Some(rect) = ctx.rect_for(node.dom_order) else {
                continue;
            };
            groups
                .entry(parent)
                .or_default()
                .push(EdgeEntry { node, rect });
        }

        for siblings in groups.values() {
            if siblings.len() < 2 {
                continue;
            }
            for axis in Axis::ALL {
                emit_for_axis(
                    self.id(),
                    self.default_severity(),
                    ctx,
                    axis,
                    tolerance,
                    siblings,
                    sink,
                );
            }
        }
    }
}

/// One sibling, paired with its rect for cheap geometry math.
#[derive(Debug, Clone, Copy)]
struct EdgeEntry<'a> {
    node: &'a SnapshotNode,
    rect: Rect,
}

/// Cluster siblings on a single edge axis and emit violations.
fn emit_for_axis(
    rule_id: &str,
    severity: Severity,
    ctx: &SnapshotCtx<'_>,
    axis: Axis,
    tolerance: u32,
    siblings: &[EdgeEntry<'_>],
    sink: &mut ViolationSink<'_>,
) {
    // Pair every sibling with its edge value, then sort by edge.
    let mut entries: Vec<(EdgeEntry<'_>, i32)> = siblings
        .iter()
        .map(|entry| (*entry, axis.edge(entry.rect)))
        .collect();
    entries.sort_by_key(|(_, edge)| *edge);

    let tolerance_i32 = i32::try_from(tolerance).unwrap_or(i32::MAX);

    let mut idx = 0;
    while idx < entries.len() {
        // Open a new cluster anchored at `entries[idx]`.
        let cluster_start_edge = entries[idx].1;
        let mut end = idx + 1;
        while end < entries.len() && entries[end].1 - cluster_start_edge <= tolerance_i32 {
            end += 1;
        }
        let cluster = &entries[idx..end];
        if cluster.len() >= 2 {
            // Centroid = rounded mean. Use i64 to avoid overflow with
            // many large coordinates; cluster size is bounded by the
            // sibling count so the cast is safe.
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let sum: i64 = cluster.iter().map(|(_, e)| i64::from(*e)).sum();
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let centroid: i32 = (sum / cluster.len() as i64) as i32;
            for (entry, edge) in cluster {
                let delta = (edge - centroid).abs();
                let delta_u32 = u32::try_from(delta).unwrap_or(0);
                if delta_u32 == 0 || delta_u32 > tolerance {
                    continue;
                }
                emit_violation(
                    rule_id,
                    severity,
                    ctx,
                    axis,
                    entry,
                    *edge,
                    centroid,
                    delta_u32,
                    cluster.len(),
                    tolerance,
                    sink,
                );
            }
        }
        idx = end;
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_violation(
    rule_id: &str,
    severity: Severity,
    ctx: &SnapshotCtx<'_>,
    axis: Axis,
    entry: &EdgeEntry<'_>,
    edge: i32,
    centroid: i32,
    delta: u32,
    cluster_size: usize,
    tolerance: u32,
    sink: &mut ViolationSink<'_>,
) {
    let mut metadata: IndexMap<String, serde_json::Value> = IndexMap::new();
    metadata.insert("axis".to_owned(), axis.name().into());
    metadata.insert("edge_px".to_owned(), edge.into());
    metadata.insert("cluster_centroid_px".to_owned(), centroid.into());
    metadata.insert("delta_px".to_owned(), delta.into());
    metadata.insert("cluster_size".to_owned(), cluster_size.into());
    metadata.insert("tolerance_px".to_owned(), tolerance.into());

    sink.push(Violation {
        rule_id: rule_id.to_owned(),
        severity,
        message: format!(
            "`{selector}` {axis} edge is {edge}px; {cluster_size} sibling(s) cluster at {centroid}px ({delta}px drift, tolerance {tolerance}px).",
            selector = entry.node.selector,
            axis = axis.name(),
        ),
        selector: entry.node.selector.clone(),
        viewport: ctx.snapshot().viewport.clone(),
        rect: Some(entry.rect),
        dom_order: entry.node.dom_order,
        fix: Some(Fix {
            kind: FixKind::Description {
                text: format!(
                    "Snap the {axis} edge to {centroid}px to match the sibling cluster.",
                    axis = axis.name(),
                ),
            },
            description: format!(
                "Align `{selector}`'s {axis} edge with its {cluster_size}-member cluster ({centroid}px).",
                selector = entry.node.selector,
                axis = axis.name(),
            ),
            confidence: Confidence::Low,
        }),
        doc_url: "https://plumb.aramhammoudeh.com/rules/edge-near-alignment".to_owned(),
        metadata,
    });
}

#[cfg(test)]
mod tests {
    use super::Axis;
    use crate::report::Rect;

    fn rect(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn axis_edges_are_correct() {
        let r = rect(10, 20, 30, 40);
        assert_eq!(Axis::Left.edge(r), 10);
        assert_eq!(Axis::Right.edge(r), 40);
        assert_eq!(Axis::Top.edge(r), 20);
        assert_eq!(Axis::Bottom.edge(r), 60);
    }

    #[test]
    fn axis_names_are_lowercase() {
        for (axis, name) in [
            (Axis::Left, "left"),
            (Axis::Right, "right"),
            (Axis::Top, "top"),
            (Axis::Bottom, "bottom"),
        ] {
            assert_eq!(axis.name(), name);
        }
    }

    #[test]
    fn axis_all_lists_every_variant() {
        // Sanity: ALL covers the four named axes exactly.
        let names: Vec<&'static str> = Axis::ALL.iter().map(|a| a.name()).collect();
        assert_eq!(names, vec!["left", "right", "top", "bottom"]);
    }
}
