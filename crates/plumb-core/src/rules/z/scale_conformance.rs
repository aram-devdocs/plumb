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

            // The early `scale.is_empty()` guard above ensures
            // `nearest_z` always returns `Some`. The `?` keeps the
            // rule total even if a future refactor relaxes the outer
            // guard.
            let Some(nearest) = nearest_z(value, scale) else {
                continue;
            };

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
///
/// Returns `None` only when `scale` is empty. Encoding the empty-scale
/// case in the return type keeps the helper total: callers do not
/// have to repeat an `is_empty()` precondition to avoid a panic, and
/// the rule stays sound if a future caller forgets the outer guard.
fn nearest_z(value: i32, scale: &[i32]) -> Option<i32> {
    scale.iter().copied().fold(None, |best, candidate| {
        let candidate_delta = value.abs_diff(candidate);
        match best {
            None => Some(candidate),
            Some(current) => {
                let current_delta = value.abs_diff(current);
                if candidate_delta < current_delta
                    || (candidate_delta == current_delta
                        && candidate.unsigned_abs() < current.unsigned_abs())
                    || (candidate_delta == current_delta
                        && candidate.unsigned_abs() == current.unsigned_abs()
                        && candidate > current)
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
    use super::nearest_z;

    #[test]
    fn empty_scale_returns_none() {
        assert_eq!(nearest_z(5, &[]), None);
    }

    #[test]
    fn picks_closest_z_in_scale() {
        let scale = [0, 10, 100];
        assert_eq!(nearest_z(7, &scale), Some(10));
        assert_eq!(nearest_z(3, &scale), Some(0));
        assert_eq!(nearest_z(60, &scale), Some(100));
    }

    #[test]
    fn breaks_ties_toward_higher_signed_value_for_equal_abs() {
        let scale = [-10, 10];
        // 0 is equidistant from -10 and 10. With equal absolute value,
        // the tie-break picks the higher signed value (10), matching
        // the existing rule contract.
        assert_eq!(nearest_z(0, &scale), Some(10));
    }
}
