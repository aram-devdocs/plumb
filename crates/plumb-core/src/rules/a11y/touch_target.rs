//! `a11y/touch-target` — flag interactive elements smaller than the
//! configured minimum target size.
//!
//! Implements the WCAG 2.5.8 *Target Size (Minimum)* criterion: any
//! interactive element with a rendered bounding rect smaller than
//! `a11y.touch_target.min_width_px` × `a11y.touch_target.min_height_px`
//! fires a violation. Defaults to 24×24 CSS pixels.
//!
//! Interactive nodes are detected by tag name (`button`, `select`,
//! `textarea`), by `<a href="…">` (anchors with an `href` attribute,
//! per the HTML spec — bare `<a>` is non-interactive), by
//! button-shaped `<input>` types, and by ARIA role
//! (`role="button"`).

use indexmap::IndexMap;

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::rules::util::is_interactive;
use crate::snapshot::SnapshotCtx;

/// Flags interactive elements smaller than `a11y.touch_target`.
#[derive(Debug, Clone, Copy)]
pub struct TouchTarget;

impl Rule for TouchTarget {
    fn id(&self) -> &'static str {
        "a11y/touch-target"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "Flags interactive elements smaller than the configured minimum target size."
    }

    fn check(&self, ctx: &SnapshotCtx<'_>, config: &Config, sink: &mut ViolationSink<'_>) {
        let min_w = config.a11y.touch_target.min_width_px;
        let min_h = config.a11y.touch_target.min_height_px;
        if min_w == 0 && min_h == 0 {
            // Both thresholds disabled — nothing to enforce.
            return;
        }

        for node in ctx.nodes() {
            if !is_interactive(node) {
                continue;
            }
            // WCAG 2.5.8 inline exception: targets whose size is
            // constrained by the line-height of surrounding text are
            // exempt. Inline prose links (`<a>` with computed
            // `display: inline`) are the canonical case.
            if node.tag == "a"
                && node.computed_styles.get("display").map(String::as_str) == Some("inline")
            {
                continue;
            }
            let Some(rect) = ctx.rect_for(node.dom_order) else {
                // Off-screen, hidden, or otherwise un-laid-out — skip.
                continue;
            };
            if rect.width >= min_w && rect.height >= min_h {
                continue;
            }
            let mut metadata: IndexMap<String, serde_json::Value> = IndexMap::new();
            metadata.insert("rendered_width_px".to_owned(), rect.width.into());
            metadata.insert("rendered_height_px".to_owned(), rect.height.into());
            metadata.insert("min_width_px".to_owned(), min_w.into());
            metadata.insert("min_height_px".to_owned(), min_h.into());

            sink.push(Violation {
                rule_id: self.id().to_owned(),
                severity: self.default_severity(),
                message: format!(
                    "`{selector}` is {w}×{h}px; WCAG 2.5.8 wants at least {min_w}×{min_h}px for interactive targets.",
                    selector = node.selector,
                    w = rect.width,
                    h = rect.height,
                ),
                selector: node.selector.clone(),
                viewport: ctx.snapshot().viewport.clone(),
                rect: Some(rect),
                dom_order: node.dom_order,
                fix: Some(Fix {
                    kind: FixKind::Description {
                        text: format!(
                            "Enlarge the hit area to at least {min_w}×{min_h}px (CSS pixels). Padding or `min-width` / `min-height` typically does the trick without changing the visual size.",
                        ),
                    },
                    description: format!(
                        "Bring `{selector}` up to the minimum touch-target size ({min_w}×{min_h}px).",
                        selector = node.selector,
                    ),
                    confidence: Confidence::Low,
                }),
                doc_url: "https://plumb.aramhammoudeh.com/rules/a11y-touch-target".to_owned(),
                metadata,
            });
        }
    }
}
