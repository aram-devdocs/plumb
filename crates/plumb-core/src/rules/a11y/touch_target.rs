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
use crate::snapshot::{SnapshotCtx, SnapshotNode};

/// Tags that are always interactive without further inspection.
const ALWAYS_INTERACTIVE_TAGS: &[&str] = &["button", "select", "textarea"];

/// `<input type="…">` values that produce a button-shaped control.
const BUTTON_INPUT_TYPES: &[&str] = &["button", "submit", "reset", "image", "checkbox", "radio"];

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

/// Whether a node represents an interactive control for the purpose of
/// the touch-target check.
fn is_interactive(node: &SnapshotNode) -> bool {
    let tag = node.tag.as_str();

    if ALWAYS_INTERACTIVE_TAGS.contains(&tag) {
        return true;
    }

    if tag == "a" && node.attrs.contains_key("href") {
        return true;
    }

    if tag == "input" {
        // Default `<input>` (no `type`) is `text`, which is not a
        // button-shaped target.
        let kind = node.attrs.get("type").map_or("text", String::as_str);
        if BUTTON_INPUT_TYPES.contains(&kind) {
            return true;
        }
    }

    if let Some(role) = node.attrs.get("role") {
        // Role-based interactivity: `role="button"` is the canonical
        // case. Other roles (link, switch, etc.) are not enforced
        // here to keep the rule's contract narrow.
        if role == "button" {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::is_interactive;
    use crate::snapshot::SnapshotNode;
    use indexmap::IndexMap;

    fn make_node(tag: &str, attrs: &[(&str, &str)]) -> SnapshotNode {
        let mut attr_map = IndexMap::new();
        for (k, v) in attrs {
            attr_map.insert((*k).to_owned(), (*v).to_owned());
        }
        SnapshotNode {
            dom_order: 0,
            selector: tag.to_owned(),
            tag: tag.to_owned(),
            attrs: attr_map,
            computed_styles: IndexMap::new(),
            rect: None,
            parent: None,
            children: Vec::new(),
        }
    }

    #[test]
    fn always_interactive_tags_match() {
        for tag in ["button", "select", "textarea"] {
            assert!(is_interactive(&make_node(tag, &[])), "{tag}");
        }
    }

    #[test]
    fn anchor_requires_href() {
        assert!(!is_interactive(&make_node("a", &[])));
        assert!(is_interactive(&make_node("a", &[("href", "/x")])));
    }

    #[test]
    fn input_button_types_match() {
        for kind in ["button", "submit", "reset", "image", "checkbox", "radio"] {
            assert!(
                is_interactive(&make_node("input", &[("type", kind)])),
                "{kind}"
            );
        }
        // Bare <input> defaults to text — not interactive for this rule.
        assert!(!is_interactive(&make_node("input", &[])));
        assert!(!is_interactive(&make_node("input", &[("type", "text")])));
    }

    #[test]
    fn role_button_matches() {
        assert!(is_interactive(&make_node("div", &[("role", "button")])));
        // Other roles are out of scope for the rule.
        assert!(!is_interactive(&make_node("div", &[("role", "link")])));
    }
}
