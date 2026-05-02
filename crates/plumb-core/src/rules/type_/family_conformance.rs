//! `type/family-conformance` — flag elements whose `font-family` is
//! not in `type.families`.

use indexmap::IndexMap;

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::snapshot::SnapshotCtx;

/// The single property this rule inspects.
const FONT_FAMILY: &str = "font-family";

/// Flags elements whose `font-family` is not in `type.families`.
#[derive(Debug, Clone, Copy)]
pub struct FamilyConformance;

impl Rule for FamilyConformance {
    fn id(&self) -> &'static str {
        "type/family-conformance"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "Flags elements whose `font-family` is not in `type.families`."
    }

    fn check(&self, ctx: &SnapshotCtx<'_>, config: &Config, sink: &mut ViolationSink<'_>) {
        let allowed = &config.type_scale.families;
        if allowed.is_empty() {
            return;
        }

        for node in ctx.nodes() {
            let Some(raw) = node.computed_styles.get(FONT_FAMILY) else {
                continue;
            };
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Parse comma-separated families, strip outer quotes.
            let families: Vec<&str> = trimmed
                .split(',')
                .map(|f| {
                    let s = f.trim();
                    // Strip outer quotes (both " and ')
                    if (s.starts_with('"') && s.ends_with('"'))
                        || (s.starts_with('\'') && s.ends_with('\''))
                    {
                        &s[1..s.len() - 1]
                    } else {
                        s
                    }
                })
                .collect();

            // Check if ANY family in the list matches ANY allowed entry (case-insensitive).
            let has_match = families.iter().any(|family| {
                allowed
                    .iter()
                    .any(|a| a.eq_ignore_ascii_case(family))
            });

            if has_match {
                continue;
            }

            let allowed_json =
                serde_json::Value::Array(allowed.iter().map(|s| serde_json::Value::String(s.clone())).collect());

            let mut metadata: IndexMap<String, serde_json::Value> = IndexMap::new();
            metadata.insert("font_family".to_owned(), serde_json::Value::String(raw.clone()));
            metadata.insert("allowed_families".to_owned(), allowed_json);

            sink.push(Violation {
                rule_id: self.id().to_owned(),
                severity: self.default_severity(),
                message: format!(
                    "`{selector}` uses font-family `{raw}` which is not in type.families.",
                    selector = node.selector,
                ),
                selector: node.selector.clone(),
                viewport: ctx.snapshot().viewport.clone(),
                rect: ctx.rect_for(node.dom_order),
                dom_order: node.dom_order,
                fix: Some(Fix {
                    kind: FixKind::Description {
                        text: format!(
                            "Use one of the allowed font families: {}.",
                            allowed.join(", "),
                        ),
                    },
                    description: format!(
                        "Replace `font-family` with one of the allowed families ({}).",
                        allowed.join(", "),
                    ),
                    confidence: Confidence::Medium,
                }),
                doc_url: "https://plumb.aramhammoudeh.com/rules/type-family-conformance".to_owned(),
                metadata,
            });
        }
    }
}
