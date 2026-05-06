//! `opacity/scale-conformance` — flag `opacity` values that aren't in
//! `opacity.scale`.

use indexmap::IndexMap;

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::snapshot::SnapshotCtx;

/// The single property this rule inspects.
const OPACITY: &str = "opacity";

/// Tolerance for matching against scale values.
const OPACITY_TOLERANCE: f64 = 0.005;

/// Flags `opacity` values that aren't in `opacity.scale`.
#[derive(Debug, Clone, Copy)]
pub struct ScaleConformance;

impl Rule for ScaleConformance {
    fn id(&self) -> &'static str {
        "opacity/scale-conformance"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "Flags `opacity` values that aren't in `opacity.scale`."
    }

    fn check(&self, ctx: &SnapshotCtx<'_>, config: &Config, sink: &mut ViolationSink<'_>) {
        let scale = &config.opacity.scale;
        if scale.is_empty() {
            return;
        }

        for node in ctx.nodes() {
            let Some(raw) = node.computed_styles.get(OPACITY) else {
                continue;
            };
            let Ok(value) = raw.trim().parse::<f64>() else {
                continue;
            };

            let matches = scale
                .iter()
                .any(|&s| (value - f64::from(s)).abs() < OPACITY_TOLERANCE);
            if matches {
                continue;
            }

            // The early `scale.is_empty()` guard above ensures
            // `nearest_opacity` always returns `Some`. The `?` keeps
            // the rule total even if a future refactor relaxes the
            // outer guard.
            let Some(nearest) = nearest_opacity(value, scale) else {
                continue;
            };

            let mut metadata: IndexMap<String, serde_json::Value> = IndexMap::new();
            metadata.insert("opacity".to_owned(), serde_json::Value::from(value));
            metadata.insert(
                "nearest".to_owned(),
                serde_json::Value::from(f64::from(nearest)),
            );

            sink.push(Violation {
                rule_id: self.id().to_owned(),
                severity: self.default_severity(),
                message: format!(
                    "`{selector}` has off-scale opacity {value}; expected a value from opacity.scale.",
                    selector = node.selector,
                ),
                selector: node.selector.clone(),
                viewport: ctx.snapshot().viewport.clone(),
                rect: ctx.rect_for(node.dom_order),
                dom_order: node.dom_order,
                fix: Some(Fix {
                    kind: FixKind::CssPropertyReplace {
                        property: OPACITY.to_owned(),
                        from: raw.clone(),
                        to: format!("{nearest}"),
                    },
                    description: format!(
                        "Snap `opacity` to the nearest scale value ({nearest}).",
                    ),
                    confidence: Confidence::Medium,
                }),
                doc_url: "https://plumb.aramhammoudeh.com/rules/opacity-scale-conformance"
                    .to_owned(),
                metadata,
            });
        }
    }
}

/// Find the nearest opacity in the scale. Ties: lower value wins.
///
/// Returns `None` only when `scale` is empty. Encoding the empty-scale
/// case in the return type keeps the helper total: callers do not
/// have to repeat an `is_empty()` precondition to avoid a panic, and
/// the rule stays sound if a future caller forgets the outer guard.
#[allow(clippy::float_cmp)]
fn nearest_opacity(value: f64, scale: &[f32]) -> Option<f32> {
    scale.iter().copied().fold(None, |best, candidate| {
        let candidate_delta = (value - f64::from(candidate)).abs();
        match best {
            None => Some(candidate),
            Some(current) => {
                let current_delta = (value - f64::from(current)).abs();
                // Equality on `f64` deltas is intentional: each delta
                // is computed the same way for every candidate, so a
                // true tie is exactly representable in `f64`.
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
    use super::nearest_opacity;

    #[test]
    fn empty_scale_returns_none() {
        assert_eq!(nearest_opacity(0.5, &[]), None);
    }

    #[test]
    fn picks_closest_opacity_in_scale() {
        let scale: [f32; 3] = [0.0, 0.5, 1.0];
        assert_eq!(nearest_opacity(0.4, &scale), Some(0.5));
        assert_eq!(nearest_opacity(0.1, &scale), Some(0.0));
        assert_eq!(nearest_opacity(0.9, &scale), Some(1.0));
    }
}
