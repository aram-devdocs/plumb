//! `type/scale-conformance` — flag `font-size` values that are not
//! members of `type.scale`.
//!
//! Mirrors [`super::super::spacing::scale_conformance`] but applies
//! only to the `font-size` computed style.

use indexmap::IndexMap;

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::rules::util::{nearest_in_scale, parse_px};
use crate::snapshot::SnapshotCtx;

/// Tolerance for the off-scale comparison; matches
/// `spacing/scale-conformance` so the two rules round identically.
const SCALE_TOLERANCE: f64 = 0.5;

/// The single property this rule inspects.
const FONT_SIZE: &str = "font-size";

/// Flags `font-size` values that aren't members of `type.scale`.
#[derive(Debug, Clone, Copy)]
pub struct ScaleConformance;

impl Rule for ScaleConformance {
    fn id(&self) -> &'static str {
        "type/scale-conformance"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "Flags `font-size` values that aren't members of `type.scale`."
    }

    fn check(&self, ctx: &SnapshotCtx<'_>, config: &Config, sink: &mut ViolationSink<'_>) {
        let scale = &config.type_scale.scale;
        if scale.is_empty() {
            return;
        }

        for node in ctx.nodes() {
            let Some(raw) = node.computed_styles.get(FONT_SIZE) else {
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
                    "`{selector}` has off-scale font-size {raw}; expected a value from type.scale.",
                    selector = node.selector,
                ),
                selector: node.selector.clone(),
                viewport: ctx.snapshot().viewport.clone(),
                rect: ctx.rect_for(node.dom_order),
                dom_order: node.dom_order,
                fix: Some(Fix {
                    kind: FixKind::CssPropertyReplace {
                        property: FONT_SIZE.to_owned(),
                        from: raw.clone(),
                        to: to.clone(),
                    },
                    description: format!(
                        "Snap `font-size` to the nearest type-scale value ({to})."
                    ),
                    confidence: Confidence::Medium,
                }),
                doc_url: "https://plumb.aramhammoudeh.com/rules/type-scale-conformance".to_owned(),
                metadata: IndexMap::new(),
            });
        }
    }
}
