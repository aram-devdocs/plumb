//! Walking-skeleton placeholder rule.
//!
//! This rule exists only to prove the engine end-to-end: it emits one
//! deterministic violation whenever a snapshot contains a `<body>` with
//! `padding: 13px`. It is removed the moment a real rule lands.

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::snapshot::SnapshotCtx;

/// The walking-skeleton rule.
#[doc(hidden)]
#[deprecated(note = "Placeholder — removed when the first real rule lands.")]
#[derive(Debug, Clone, Copy)]
pub struct HelloWorld;

#[allow(deprecated)]
impl Rule for HelloWorld {
    fn id(&self) -> &'static str {
        "placeholder/hello-world"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "Walking-skeleton placeholder rule; flags `body { padding: 13px }`."
    }

    fn check(&self, ctx: &SnapshotCtx<'_>, _config: &Config, sink: &mut ViolationSink<'_>) {
        for node in ctx.nodes() {
            if node.tag != "body" {
                continue;
            }
            let Some(padding) = node.computed_styles.get("padding") else {
                continue;
            };
            if padding != "13px" {
                continue;
            }
            sink.push(Violation {
                rule_id: self.id().to_owned(),
                severity: self.default_severity(),
                message: format!(
                    "`body` has off-scale padding {padding}; expected a value from the spacing token set.",
                ),
                selector: node.selector.clone(),
                viewport: ctx.snapshot().viewport.clone(),
                rect: node.rect,
                dom_order: node.dom_order,
                fix: Some(Fix {
                    kind: FixKind::CssPropertyReplace {
                        property: "padding".into(),
                        from: padding.clone(),
                        to: "16px".into(),
                    },
                    description: "Snap `body` padding to the nearest spacing token (16px).".into(),
                    confidence: Confidence::Medium,
                }),
                doc_url: "https://plumb.aramhammoudeh.com/rules/placeholder-hello-world".into(),
                metadata: indexmap::IndexMap::new(),
            });
        }
    }
}
