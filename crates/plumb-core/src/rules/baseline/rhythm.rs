//! `baseline/rhythm` — flag text elements whose baselines miss the
//! configured vertical-rhythm grid.
//!
//! For each text-bearing element with a `font-size` and a bounding
//! rect, the rule computes an approximate baseline position and checks
//! whether it falls on a multiple of `rhythm.base_line_px` (within
//! `rhythm.tolerance_px`).

use indexmap::IndexMap;
use serde_json::Value as JsonValue;

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::rules::util::parse_px;
use crate::snapshot::SnapshotCtx;

/// Tags considered text-bearing for the purpose of this rule.
const TEXT_TAGS: &[&str] = &[
    "p",
    "span",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "a",
    "li",
    "td",
    "th",
    "label",
    "button",
    "input",
    "textarea",
    "select",
    "summary",
    "dt",
    "dd",
    "figcaption",
    "blockquote",
    "cite",
    "code",
    "pre",
    "em",
    "strong",
    "small",
    "b",
    "i",
    "u",
    "mark",
    "time",
    "abbr",
];

/// Typical Latin cap-height ratio (cap-height / font-size).
const CAP_HEIGHT_RATIO: f64 = 0.7;

/// Default line-height multiplier when the value is `normal` or missing.
const DEFAULT_LINE_HEIGHT_RATIO: f64 = 1.2;

/// Flags text elements whose baselines don't align to the rhythm grid.
#[derive(Debug, Clone, Copy)]
pub struct Rhythm;

impl Rule for Rhythm {
    fn id(&self) -> &'static str {
        "baseline/rhythm"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "Flags text elements whose baselines miss the vertical-rhythm grid."
    }

    fn check(&self, ctx: &SnapshotCtx<'_>, config: &Config, sink: &mut ViolationSink<'_>) {
        let base_line = config.rhythm.base_line_px;
        if base_line == 0 {
            return;
        }
        let base_line_f = f64::from(base_line);
        let tolerance_f = f64::from(config.rhythm.tolerance_px);
        let cap_fallback = config.rhythm.cap_height_fallback_px;

        for node in ctx.nodes() {
            if !TEXT_TAGS.contains(&node.tag.as_str()) {
                continue;
            }

            let Some(rect) = ctx.rect_for(node.dom_order) else {
                continue;
            };

            let Some(font_size_raw) = node.computed_styles.get("font-size") else {
                continue;
            };
            let Some(font_size) = parse_px(font_size_raw) else {
                continue;
            };
            if font_size <= 0.0 {
                continue;
            }

            // Cap-height approximation.
            let cap_height = if cap_fallback > 0 {
                f64::from(cap_fallback)
            } else {
                font_size * CAP_HEIGHT_RATIO
            };

            // Line-height: parse from computed styles, fall back to 1.2 * font_size.
            let line_height = node
                .computed_styles
                .get("line-height")
                .and_then(|v| parse_px(v))
                .unwrap_or(font_size * DEFAULT_LINE_HEIGHT_RATIO);

            // half_leading = (line_height - font_size) / 2
            let half_leading = (line_height - font_size) / 2.0;

            // Approximate baseline Y position.
            let baseline_y = f64::from(rect.y) + half_leading + cap_height;

            // Distance to nearest grid line.
            let nearest_grid_y = (baseline_y / base_line_f).round() * base_line_f;
            let distance = (baseline_y - nearest_grid_y).abs();

            if distance <= tolerance_f {
                continue;
            }

            let mut metadata = IndexMap::new();
            metadata.insert("baseline_y".to_owned(), JsonValue::from(baseline_y));
            metadata.insert("nearest_grid_y".to_owned(), JsonValue::from(nearest_grid_y));
            metadata.insert("distance_px".to_owned(), JsonValue::from(distance));

            sink.push(Violation {
                rule_id: self.id().to_owned(),
                severity: self.default_severity(),
                message: format!(
                    "`{selector}` baseline at {baseline_y:.1}px is {distance:.1}px off the {base_line}px rhythm grid.",
                    selector = node.selector,
                ),
                selector: node.selector.clone(),
                viewport: ctx.snapshot().viewport.clone(),
                rect: Some(rect),
                dom_order: node.dom_order,
                fix: Some(Fix {
                    kind: FixKind::Description {
                        text: format!(
                            "Adjust line-height or margin-top so the baseline aligns to the nearest {base_line}px grid line ({nearest_grid_y:.0}px).",
                        ),
                    },
                    description: format!(
                        "Shift baseline from {baseline_y:.1}px to {nearest_grid_y:.0}px to restore vertical rhythm.",
                    ),
                    confidence: Confidence::Low,
                }),
                doc_url: "https://plumb.aramhammoudeh.com/rules/baseline-rhythm".to_owned(),
                metadata,
            });
        }
    }
}
