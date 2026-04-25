//! `spacing/scale-conformance` — flag spacing values that are not
//! members of the configured discrete scale.
//!
//! Iterates the same physical-longhand spacing properties as
//! [`super::grid_conformance`] but compares each parsed pixel value
//! against `config.spacing.scale`. Subpixel values are tolerated to
//! within `0.5px` so a computed `12.4px` matches scale element `12`.

use indexmap::IndexMap;

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::rules::spacing::SPACING_PROPERTIES;
use crate::rules::util::{nearest_in_scale, parse_px};
use crate::snapshot::SnapshotCtx;

/// Tolerance for the off-scale comparison. `0.5` keeps the rule
/// resilient against subpixel rounding from `getComputedStyle` while
/// still catching genuinely off-scale values like `15px` against an
/// `[16, 24]` scale.
const SCALE_TOLERANCE: f64 = 0.5;

/// Flags spacing values that aren't members of `spacing.scale`.
#[derive(Debug, Clone, Copy)]
pub struct ScaleConformance;

impl Rule for ScaleConformance {
    fn id(&self) -> &'static str {
        "spacing/scale-conformance"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "Flags spacing values that aren't members of `spacing.scale`."
    }

    fn check(&self, ctx: &SnapshotCtx<'_>, config: &Config, sink: &mut ViolationSink<'_>) {
        let scale = &config.spacing.scale;
        if scale.is_empty() {
            // No scale configured — the rule is a no-op rather than
            // flagging every pixel value as out of bounds.
            return;
        }

        for node in ctx.nodes() {
            for prop in SPACING_PROPERTIES {
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
                        "`{selector}` has off-scale {prop} {raw}; expected a value from spacing.scale.",
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
                            "Snap `{prop}` to the nearest spacing-scale value ({to}).",
                        ),
                        confidence: Confidence::Medium,
                    }),
                    doc_url: "https://plumb.aramhammoudeh.com/rules/spacing-scale-conformance"
                        .to_owned(),
                    metadata: IndexMap::new(),
                });
            }
        }
    }
}
