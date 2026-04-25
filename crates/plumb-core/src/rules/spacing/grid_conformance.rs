//! `spacing/grid-conformance` — flag spacing values that aren't on the
//! configured grid.
//!
//! Iterates the physical-longhand spacing properties (margin / padding
//! per side, plus `gap` / `row-gap` / `column-gap`) and emits one
//! violation per offending property when the parsed pixel value isn't
//! a multiple of `config.spacing.base_unit`.

use indexmap::IndexMap;

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::rules::spacing::SPACING_PROPERTIES;
use crate::rules::util::{nearest_multiple, parse_px};
use crate::snapshot::SnapshotCtx;

/// Tolerance for the off-grid test. Subpixel rounding from
/// `getComputedStyle` can leave a residue of ~1e-12; `1e-6` keeps the
/// rule resilient without admitting honest off-grid values.
const FRACT_TOLERANCE: f64 = 1e-6;

/// Flags spacing values that aren't multiples of `spacing.base_unit`.
#[derive(Debug, Clone, Copy)]
pub struct GridConformance;

impl Rule for GridConformance {
    fn id(&self) -> &'static str {
        "spacing/grid-conformance"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "Flags spacing values that aren't multiples of `spacing.base_unit`."
    }

    fn check(&self, ctx: &SnapshotCtx<'_>, config: &Config, sink: &mut ViolationSink<'_>) {
        let base_unit = config.spacing.base_unit;
        if base_unit == 0 {
            // Defensive: a zero base_unit makes the check meaningless and
            // would force a div-by-zero. Skip the rule entirely.
            return;
        }
        let base_unit_f = f64::from(base_unit);

        for node in ctx.nodes() {
            for prop in SPACING_PROPERTIES {
                let Some(raw) = node.computed_styles.get(*prop) else {
                    continue;
                };
                let Some(value) = parse_px(raw) else { continue };
                if (value / base_unit_f).fract().abs() <= FRACT_TOLERANCE {
                    continue;
                }
                let suggested = nearest_multiple(value, base_unit);
                let to = if suggested == 0 {
                    "0".to_owned()
                } else {
                    format!("{suggested}px")
                };
                sink.push(Violation {
                    rule_id: self.id().to_owned(),
                    severity: self.default_severity(),
                    message: format!(
                        "`{selector}` has off-grid {prop} {raw}; expected a multiple of {base_unit}px.",
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
                            "Snap `{prop}` to the nearest spacing-grid value ({to}).",
                        ),
                        confidence: Confidence::Medium,
                    }),
                    doc_url: "https://plumb.aramhammoudeh.com/rules/spacing-grid-conformance"
                        .to_owned(),
                    metadata: IndexMap::new(),
                });
            }
        }
    }
}
