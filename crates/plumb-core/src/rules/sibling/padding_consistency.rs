//! `sibling/padding-consistency` — flag sibling elements with
//! inconsistent padding.

use indexmap::IndexMap;

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::rules::util::parse_px;
use crate::snapshot::SnapshotCtx;

/// The padding longhands checked for consistency.
const PADDING_PROPERTIES: &[&str] = &[
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
];

/// Padding this far from the sibling median (in CSS pixels) triggers a
/// violation.
const PADDING_DEVIATION_PX: u32 = 4;

/// Flags sibling elements with inconsistent padding.
#[derive(Debug, Clone, Copy)]
pub struct PaddingConsistency;

impl Rule for PaddingConsistency {
    fn id(&self) -> &'static str {
        "sibling/padding-consistency"
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn summary(&self) -> &'static str {
        "Flags sibling elements with inconsistent padding."
    }

    fn check(&self, ctx: &SnapshotCtx<'_>, _config: &Config, sink: &mut ViolationSink<'_>) {
        // Group nodes by parent dom_order.
        let mut groups: IndexMap<u64, Vec<usize>> = IndexMap::new();
        for (idx, node) in ctx.snapshot().nodes.iter().enumerate() {
            let Some(parent) = node.parent else { continue };
            groups.entry(parent).or_default().push(idx);
        }

        let nodes = &ctx.snapshot().nodes;

        for siblings in groups.values() {
            if siblings.len() < 2 {
                continue;
            }

            for prop in PADDING_PROPERTIES {
                // Collect (index, parsed px value) for siblings that have
                // the property and it parses.
                let parsed: Vec<(usize, f64)> = siblings
                    .iter()
                    .filter_map(|&idx| {
                        let raw = nodes[idx].computed_styles.get(*prop)?;
                        let val = parse_px(raw)?;
                        Some((idx, val))
                    })
                    .collect();

                if parsed.len() < 2 {
                    continue;
                }

                let median = median_f64(&parsed.iter().map(|(_, v)| *v).collect::<Vec<_>>());

                for &(idx, val) in &parsed {
                    let dev = (val - median).abs();
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let dev_u32 = dev.round() as u32;
                    if dev_u32 <= PADDING_DEVIATION_PX {
                        continue;
                    }

                    let node = &nodes[idx];
                    let mut metadata: IndexMap<String, serde_json::Value> = IndexMap::new();
                    metadata.insert(
                        "property".to_owned(),
                        serde_json::Value::String((*prop).to_owned()),
                    );
                    metadata.insert(
                        "rendered_padding_px".to_owned(),
                        serde_json::Value::from(val),
                    );
                    metadata.insert(
                        "sibling_median_px".to_owned(),
                        serde_json::Value::from(median),
                    );
                    metadata.insert(
                        "deviation_px".to_owned(),
                        serde_json::Value::from(dev_u32),
                    );

                    sink.push(Violation {
                        rule_id: self.id().to_owned(),
                        severity: self.default_severity(),
                        message: format!(
                            "`{selector}` has {prop} {val}px; sibling median is {median}px ({dev_u32}px drift).",
                            selector = node.selector,
                        ),
                        selector: node.selector.clone(),
                        viewport: ctx.snapshot().viewport.clone(),
                        rect: ctx.rect_for(node.dom_order),
                        dom_order: node.dom_order,
                        fix: Some(Fix {
                            kind: FixKind::Description {
                                text: format!(
                                    "Match sibling {prop} ({median}px) to keep padding consistent. Drift: {dev_u32}px.",
                                ),
                            },
                            description: format!(
                                "Bring `{selector}` {prop} in line with its siblings ({median}px).",
                                selector = node.selector,
                            ),
                            confidence: Confidence::Low,
                        }),
                        doc_url: "https://plumb.aramhammoudeh.com/rules/sibling-padding-consistency"
                            .to_owned(),
                        metadata,
                    });
                }
            }
        }
    }
}

/// Median of a slice of f64 values.
///
/// For even counts, the lower of the two middle values wins (same
/// deterministic tie-break as `sibling/height-consistency`).
fn median_f64(values: &[f64]) -> f64 {
    let mut sorted: Vec<f64> = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        sorted[mid - 1]
    } else {
        sorted[mid]
    }
}
