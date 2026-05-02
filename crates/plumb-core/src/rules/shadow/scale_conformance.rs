//! `shadow/scale-conformance` — flag `box-shadow` values that aren't
//! in `shadow.scale`.

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::snapshot::SnapshotCtx;

/// The single property this rule inspects.
const BOX_SHADOW: &str = "box-shadow";

/// Flags `box-shadow` values that aren't in `shadow.scale`.
#[derive(Debug, Clone, Copy)]
pub struct ScaleConformance;

impl Rule for ScaleConformance {
    fn id(&self) -> &'static str {
        "shadow/scale-conformance"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "Flags `box-shadow` values that aren't in `shadow.scale`."
    }

    fn check(&self, ctx: &SnapshotCtx<'_>, config: &Config, sink: &mut ViolationSink<'_>) {
        let scale = &config.shadow.scale;
        if scale.is_empty() {
            return;
        }

        for node in ctx.nodes() {
            let Some(raw) = node.computed_styles.get(BOX_SHADOW) else {
                continue;
            };
            let trimmed = raw.trim();
            if trimmed.eq_ignore_ascii_case("none") {
                continue;
            }

            let matches = scale.iter().any(|s| s.trim() == trimmed);
            if matches {
                continue;
            }

            sink.push(Violation {
                rule_id: self.id().to_owned(),
                severity: self.default_severity(),
                message: format!(
                    "`{selector}` has off-scale box-shadow `{trimmed}`; expected a value from shadow.scale.",
                    selector = node.selector,
                ),
                selector: node.selector.clone(),
                viewport: ctx.snapshot().viewport.clone(),
                rect: ctx.rect_for(node.dom_order),
                dom_order: node.dom_order,
                fix: Some(Fix {
                    kind: FixKind::Description {
                        text: format!(
                            "The box-shadow value `{trimmed}` is not in the allowed shadow scale.",
                        ),
                    },
                    description: "Replace `box-shadow` with one of the allowed shadow tokens."
                        .to_owned(),
                    confidence: Confidence::Medium,
                }),
                doc_url: "https://plumb.aramhammoudeh.com/rules/shadow-scale-conformance"
                    .to_owned(),
                metadata: indexmap::IndexMap::new(),
            });
        }
    }
}
