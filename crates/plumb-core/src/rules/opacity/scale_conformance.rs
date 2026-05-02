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

            let nearest = nearest_opacity(value, scale);

            let mut metadata: IndexMap<String, serde_json::Value> = IndexMap::new();
            metadata.insert(
                "opacity".to_owned(),
                serde_json::Value::from(value),
            );
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
#[allow(clippy::float_cmp)]
fn nearest_opacity(value: f64, scale: &[f32]) -> f32 {
    let mut best = scale[0];
    let mut best_delta = (value - f64::from(best)).abs();
    for &candidate in &scale[1..] {
        let delta = (value - f64::from(candidate)).abs();
        if delta < best_delta || (delta == best_delta && candidate < best) {
            best = candidate;
            best_delta = delta;
        }
    }
    best
}
