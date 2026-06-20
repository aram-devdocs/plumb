//! `spacing/grid-conformance` — flag spacing values that aren't on the
//! configured grid.
//!
//! Iterates the physical-longhand spacing properties (margin / padding
//! per side, plus `gap` / `row-gap` / `column-gap`) and emits one
//! violation per offending property when the parsed pixel value isn't
//! a multiple of `config.spacing.base_unit`.
//!
//! The rule defers to the configured spacing scale. When
//! `config.spacing.scale` is non-empty and the parsed value sits within
//! `GRID_TOLERANCE_PX` of one of its members, the value is treated as
//! on the design system and skipped — even when it's off the base-unit
//! grid. This matters for Tailwind, whose scale includes 2px half-steps
//! (6/10/14px, …) that a pure "multiple of `base_unit`" test would flag.
//! An empty scale (the default config) restores the plain base-unit
//! grid behavior.

use indexmap::IndexMap;

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::rules::spacing::{SPACING_PROPERTIES, is_framework_hidden_spacing_node};
use crate::rules::util::{nearest_in_scale, nearest_multiple, parse_px};
use crate::snapshot::SnapshotCtx;

/// Tolerance for the off-grid test, in CSS pixels. A value within this
/// absolute band of the nearest grid multiple is treated as on-grid.
/// `0.5px` absorbs subpixel rounding and UA-stylesheet `em`-derived
/// residue (e.g. a `16.08px` default `<h1>` margin snaps to `16`) while
/// still catching honest off-grid values like `13px` or `10px`.
const GRID_TOLERANCE_PX: f64 = 0.5;

/// Flags spacing values that aren't multiples of `spacing.base_unit`.
///
/// Values explicitly listed in `config.spacing.scale` are exempt: when
/// the parsed value is within `GRID_TOLERANCE_PX` of a scale member it
/// is treated as on the design system and never flagged, even if it
/// falls off the base-unit grid. When the scale is empty the rule checks
/// against the base unit alone.
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

        for node in ctx.nodes() {
            if is_framework_hidden_spacing_node(node) {
                continue;
            }
            for prop in SPACING_PROPERTIES {
                if is_root_body_margin(&node.tag, &node.selector, prop) {
                    continue;
                }
                let Some(raw) = node.computed_styles.get(*prop) else {
                    continue;
                };
                let Some(value) = parse_px(raw) else { continue };
                // Defer to the configured spacing scale. When the design
                // system explicitly lists a value — Tailwind populates
                // `spacing.scale` with its tokens, including 2px
                // half-steps like 6/10/14px — that value belongs even
                // though it's off the `base_unit` grid. Skip the off-grid
                // test when `value` is within `GRID_TOLERANCE_PX` of its
                // nearest scale member. An empty scale (default config)
                // yields `None` here, preserving the pure base-unit grid.
                if let Some(on_scale) = nearest_in_scale(value, &config.spacing.scale)
                    && (value.abs() - f64::from(on_scale)).abs() <= GRID_TOLERANCE_PX
                {
                    continue;
                }
                let suggested = nearest_multiple(value, base_unit);
                #[allow(clippy::cast_precision_loss)]
                let nearest = suggested as f64;
                if (value - nearest).abs() <= GRID_TOLERANCE_PX {
                    continue;
                }
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

fn is_root_body_margin(tag: &str, selector: &str, prop: &str) -> bool {
    tag == "body" && selector == "html > body" && prop.starts_with("margin-")
}
