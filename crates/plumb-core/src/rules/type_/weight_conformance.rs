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

            // The early `weights.is_empty()` guard above ensures
            // `nearest_weight` always returns `Some`. The `?` keeps
            // the rule total even if a future refactor relaxes the
            // outer guard.
            let Some(nearest) = nearest_weight(value, weights) else {
                continue;
            };

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
///
/// Returns `None` only when `scale` is empty. Encoding the empty-scale
/// case in the return type keeps the helper total: callers do not
/// have to repeat an `is_empty()` precondition to avoid a panic, and
/// the rule stays sound if a future caller forgets the outer guard.
fn nearest_weight(value: u16, scale: &[u16]) -> Option<u16> {
    scale.iter().copied().fold(None, |best, candidate| {
        let candidate_delta = value.abs_diff(candidate);
        match best {
            None => Some(candidate),
            Some(current) => {
                let current_delta = value.abs_diff(current);
                if candidate_delta < current_delta
                    || (candidate_delta == current_delta && candidate < current)
                {
                    Some(candidate)
                } else {
                    Some(current)
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::nearest_weight;

    #[test]
    fn empty_scale_returns_none() {
        assert_eq!(nearest_weight(500, &[]), None);
    }

    #[test]
    fn picks_closest_weight_in_scale() {
        let scale = [400, 500, 700];
        assert_eq!(nearest_weight(450, &scale), Some(400));
        assert_eq!(nearest_weight(600, &scale), Some(500));
        assert_eq!(nearest_weight(800, &scale), Some(700));
    }

    #[test]
    fn breaks_ties_toward_lower_value() {
        let scale = [400, 600];
        // Equidistant: 500 - 400 == 600 - 500. Lower wins.
        assert_eq!(nearest_weight(500, &scale), Some(400));
    }
}
