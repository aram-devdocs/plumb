//! `radius/scale-conformance` — flag `border-*-radius` values that are
//! not members of the configured discrete scale.
//!
//! Iterates the four physical-longhand corner-radius properties and
//! compares each parsed pixel value against `config.radius.scale`.
//! Subpixel values are tolerated to within `0.5px`, mirroring the
//! `spacing/scale-conformance` heuristic.

use indexmap::IndexMap;

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::rules::radius::RADIUS_PROPERTIES;
use crate::rules::util::{nearest_in_scale, parse_px};
use crate::snapshot::SnapshotCtx;

/// Tolerance for the off-scale comparison. Matches
/// `spacing/scale-conformance` so the two rules round identically.
const SCALE_TOLERANCE: f64 = 0.5;

/// Flags border-radius values that aren't members of `radius.scale`.
#[derive(Debug, Clone, Copy)]
pub struct ScaleConformance;

impl Rule for ScaleConformance {
    fn id(&self) -> &'static str {
        "radius/scale-conformance"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "Flags border-radius values that aren't members of `radius.scale`."
    }

    fn check(&self, ctx: &SnapshotCtx<'_>, config: &Config, sink: &mut ViolationSink<'_>) {
        let scale = &config.radius.scale;
        if scale.is_empty() {
            // No scale configured — the rule is a no-op rather than
            // flagging every pixel value as out of bounds.
            return;
        }

        for node in ctx.nodes() {
            for prop in RADIUS_PROPERTIES {
                let Some(raw) = node.computed_styles.get(*prop) else {
                    continue;
                };
                let Some(value) = parse_px(raw) else { continue };
                let abs = value.abs();
                if scale
                    .iter()
                    .any(|&elem| (abs - f64::from(elem)).abs() < SCALE_TOLERANCE)
                {
                    continue;
                }
                let Some(suggested) = nearest_in_scale(value, scale) else {
                    // Unreachable in practice — `scale.is_empty()` is
                    // checked above. Skip rather than emit a misleading
                    // violation if the invariant ever changes.
                    continue;
                };
                let to = if suggested == 0 {
                    "0".to_owned()
                } else {
                    format!("{suggested}px")
                };
                sink.push(Violation {
                    rule_id: self.id().to_owned(),
                    severity: self.default_severity(),
                    message: format!(
                        "`{selector}` has off-scale {prop} {raw}; expected a value from radius.scale.",
                        selector = node.selector,
                    ),
                    selector: node.selector.clone(),
                    viewport: ctx.snapshot().viewport.clone(),
                    rect: ctx.rect_for(node.dom_order),
                    dom_order: node.dom_order,
                    fix: Some(Fix {
                        kind: FixKind::CssPropertyReplace {
                            property: (*prop).to_owned(),
                            from: raw.clone(),
                            to: to.clone(),
                        },
                        description: format!(
                            "Snap `{prop}` to the nearest radius-scale value ({to}).",
                        ),
                        confidence: Confidence::Medium,
                    }),
                    doc_url: "https://plumb.aramhammoudeh.com/rules/radius-scale-conformance"
                        .to_owned(),
                    metadata: IndexMap::new(),
                });
            }
        }
    }
}
