//! `z/scale-conformance` — flag `z-index` values that aren't in
//! `z_index.scale`.

use indexmap::IndexMap;

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::snapshot::SnapshotCtx;

/// The single property this rule inspects.
const Z_INDEX: &str = "z-index";

/// Flags `z-index` values that aren't in `z_index.scale`.
#[derive(Debug, Clone, Copy)]
pub struct ScaleConformance;

impl Rule for ScaleConformance {
    fn id(&self) -> &'static str {
        "z/scale-conformance"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "Flags `z-index` values that aren't in `z_index.scale`."
    }

    fn check(&self, ctx: &SnapshotCtx<'_>, config: &Config, sink: &mut ViolationSink<'_>) {
        let scale = &config.z_index.scale;
        if scale.is_empty() {
            return;
        }

        for node in ctx.nodes() {
            let Some(raw) = node.computed_styles.get(Z_INDEX) else {
                continue;
            };
            let trimmed = raw.trim();
            if trimmed.eq_ignore_ascii_case("auto") {
                continue;
            }
            let Ok(value) = trimmed.parse::<i32>() else {
                continue;
            };

            if scale.contains(&value) {
                continue;
            }

            let nearest = nearest_z(value, scale);

            let mut metadata: IndexMap<String, serde_json::Value> = IndexMap::new();
            metadata.insert("z_index".to_owned(), serde_json::Value::from(value));
            metadata.insert("nearest".to_owned(), serde_json::Value::from(nearest));

            sink.push(Violation {
                rule_id: self.id().to_owned(),
                severity: self.default_severity(),
                message: format!(
                    "`{selector}` has off-scale z-index {value}; expected a value from z_index.scale.",
                    selector = node.selector,
                ),
                selector: node.selector.clone(),
                viewport: ctx.snapshot().viewport.clone(),
                rect: ctx.rect_for(node.dom_order),
                dom_order: node.dom_order,
                fix: Some(Fix {
                    kind: FixKind::CssPropertyReplace {
                        property: Z_INDEX.to_owned(),
                        from: raw.clone(),
                        to: nearest.to_string(),
                    },
                    description: format!(
                        "Snap `z-index` to the nearest scale value ({nearest}).",
                    ),
                    confidence: Confidence::Medium,
                }),
                doc_url: "https://plumb.aramhammoudeh.com/rules/z-scale-conformance".to_owned(),
                metadata,
            });
        }
    }
}

/// Find the nearest z-index in the scale.
///
/// Ties: toward lower absolute value, then toward the value closer to zero.
fn nearest_z(value: i32, scale: &[i32]) -> i32 {
    let mut best = scale[0];
    let mut best_delta = value.abs_diff(best);
    for &candidate in &scale[1..] {
        let delta = value.abs_diff(candidate);
        if delta < best_delta
            || (delta == best_delta && candidate.unsigned_abs() < best.unsigned_abs())
            || (delta == best_delta
                && candidate.unsigned_abs() == best.unsigned_abs()
                && candidate > best)
        {
            best = candidate;
            best_delta = delta;
        }
    }
    best
}
