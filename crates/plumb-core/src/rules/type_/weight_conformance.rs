//! `type/weight-conformance` — flag elements whose `font-weight` is
//! not in `type.weights`.

use indexmap::IndexMap;

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::snapshot::SnapshotCtx;

/// The single property this rule inspects.
const FONT_WEIGHT: &str = "font-weight";

/// Flags elements whose `font-weight` is not in `type.weights`.
#[derive(Debug, Clone, Copy)]
pub struct WeightConformance;

impl Rule for WeightConformance {
    fn id(&self) -> &'static str {
        "type/weight-conformance"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "Flags elements whose `font-weight` is not in `type.weights`."
    }

    fn check(&self, ctx: &SnapshotCtx<'_>, config: &Config, sink: &mut ViolationSink<'_>) {
        let weights = &config.type_scale.weights;
        if weights.is_empty() {
            return;
        }

        for node in ctx.nodes() {
            let Some(raw) = node.computed_styles.get(FONT_WEIGHT) else {
                continue;
            };
            let Ok(value) = raw.trim().parse::<u16>() else {
                continue;
            };

            if weights.contains(&value) {
                continue;
            }

            let nearest = nearest_weight(value, weights);

            let mut metadata: IndexMap<String, serde_json::Value> = IndexMap::new();
            metadata.insert("font_weight".to_owned(), serde_json::Value::from(value));
            metadata.insert("nearest".to_owned(), serde_json::Value::from(nearest));

            sink.push(Violation {
                rule_id: self.id().to_owned(),
                severity: self.default_severity(),
                message: format!(
                    "`{selector}` has off-scale font-weight {value}; expected a value from type.weights.",
                    selector = node.selector,
                ),
                selector: node.selector.clone(),
                viewport: ctx.snapshot().viewport.clone(),
                rect: ctx.rect_for(node.dom_order),
                dom_order: node.dom_order,
                fix: Some(Fix {
                    kind: FixKind::CssPropertyReplace {
                        property: FONT_WEIGHT.to_owned(),
                        from: raw.clone(),
                        to: nearest.to_string(),
                    },
                    description: format!(
                        "Snap `font-weight` to the nearest type-scale weight ({nearest}).",
                    ),
                    confidence: Confidence::Medium,
                }),
                doc_url: "https://plumb.aramhammoudeh.com/rules/type-weight-conformance".to_owned(),
                metadata,
            });
        }
    }
}

/// Find the nearest weight in the scale. Ties: lower wins.
fn nearest_weight(value: u16, scale: &[u16]) -> u16 {
    let mut best = scale[0];
    let mut best_delta = value.abs_diff(best);
    for &candidate in &scale[1..] {
        let delta = value.abs_diff(candidate);
        if delta < best_delta || (delta == best_delta && candidate < best) {
            best = candidate;
            best_delta = delta;
        }
    }
    best
}
