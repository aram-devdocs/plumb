//! `color/contrast-aa` — enforce WCAG 2.1 AA text contrast from
//! existing computed styles only.
//!
//! The rule pairs a node's computed `color`, `font-size`,
//! optional `font-weight`, and the nearest composited
//! `background-color` in the DOM ancestor chain.
//!
//! Thresholds follow WCAG 2.1 AA:
//!
//! - normal text: 4.5:1 minimum,
//! - large text: 3.0:1 minimum,
//! - "large" means at least 24px regular or 18.5px bold.

use indexmap::IndexMap;
use palette::{LinSrgb, Srgb};

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::rules::util::{CssColor, parse_css_color, parse_px};
use crate::snapshot::{PlumbSnapshot, SnapshotCtx, SnapshotNode};

const DEFAULT_BACKGROUND: CssColor = CssColor {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

const FOREGROUND_COLOR: &str = "color";
const BACKGROUND_COLOR: &str = "background-color";
const FONT_SIZE: &str = "font-size";
const FONT_WEIGHT: &str = "font-weight";

const NORMAL_TEXT_MIN_RATIO: f64 = 4.5;
const LARGE_TEXT_MIN_RATIO: f64 = 3.0;
const LARGE_TEXT_MIN_PX: f64 = 24.0;
const LARGE_BOLD_TEXT_MIN_PX: f64 = 18.5;
const BOLD_WEIGHT_MIN: u16 = 700;

/// Flags text whose computed foreground/background contrast misses the
/// WCAG 2.1 AA threshold for its size class.
#[derive(Debug, Clone, Copy)]
pub struct ContrastAa;

impl Rule for ContrastAa {
    fn id(&self) -> &'static str {
        "color/contrast-aa"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "Flags text whose computed foreground/background contrast misses WCAG 2.1 AA."
    }

    fn check(&self, ctx: &SnapshotCtx<'_>, config: &Config, sink: &mut ViolationSink<'_>) {
        let snapshot = ctx.snapshot();
        let parents = parent_index(snapshot);

        for node in ctx.nodes() {
            if let Some(violation) = violation_for_node(ctx, config, snapshot, &parents, node) {
                sink.push(violation);
            }
        }
    }
}

fn violation_for_node(
    ctx: &SnapshotCtx<'_>,
    config: &Config,
    snapshot: &PlumbSnapshot,
    parents: &IndexMap<u64, u64>,
    node: &SnapshotNode,
) -> Option<Violation> {
    let raw_foreground = node.computed_styles.get(FOREGROUND_COLOR)?;
    let foreground = parse_css_color(raw_foreground)?;
    if foreground.a <= 0.0 {
        return None;
    }

    let raw_font_size = node.computed_styles.get(FONT_SIZE)?;
    let font_size_px = parse_px(raw_font_size)?;
    if !font_size_px.is_finite() || font_size_px <= 0.0 {
        return None;
    }

    let font_weight = node
        .computed_styles
        .get(FONT_WEIGHT)
        .and_then(|raw| parse_font_weight(raw));
    let is_large = classify_large_text(font_size_px, font_weight);
    let required_ratio = required_ratio(config, is_large);
    let background = resolve_background(snapshot, parents, node);
    let effective_foreground = if (foreground.a - 1.0).abs() < f32::EPSILON {
        foreground
    } else {
        composite_over(foreground, background)
    };
    let measured_ratio = contrast_ratio(effective_foreground, background);
    if measured_ratio >= required_ratio {
        return None;
    }

    Some(Violation {
        rule_id: "color/contrast-aa".to_owned(),
        severity: Severity::Warning,
        message: violation_message(node, measured_ratio, required_ratio, is_large),
        selector: node.selector.clone(),
        viewport: snapshot.viewport.clone(),
        rect: ctx.rect_for(node.dom_order),
        dom_order: node.dom_order,
        fix: Some(Fix {
            kind: FixKind::Description {
                text: fix_text(required_ratio, is_large),
            },
            description: format!(
                "Raise `{selector}` to the WCAG 2.1 AA contrast floor.",
                selector = node.selector,
            ),
            confidence: Confidence::Low,
        }),
        doc_url: "https://plumb.aramhammoudeh.com/rules/color-contrast-aa".to_owned(),
        metadata: build_metadata(
            node,
            raw_foreground,
            measured_ratio,
            required_ratio,
            font_size_px,
            is_large,
        ),
    })
}

fn classify_large_text(font_size_px: f64, font_weight: Option<u16>) -> bool {
    if font_size_px >= LARGE_TEXT_MIN_PX {
        return true;
    }
    font_size_px >= LARGE_BOLD_TEXT_MIN_PX && font_weight.is_some_and(is_bold_weight)
}

fn is_bold_weight(weight: u16) -> bool {
    weight >= BOLD_WEIGHT_MIN
}

fn parse_font_weight(raw: &str) -> Option<u16> {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("normal") {
        return Some(400);
    }
    if trimmed.eq_ignore_ascii_case("bold") {
        return Some(700);
    }
    if trimmed.eq_ignore_ascii_case("bolder") {
        return Some(700);
    }
    if trimmed.eq_ignore_ascii_case("lighter") {
        return Some(300);
    }
    trimmed.parse::<u16>().ok()
}

fn contrast_ratio(foreground: CssColor, background: CssColor) -> f64 {
    let fg_luminance = relative_luminance(foreground);
    let bg_luminance = relative_luminance(background);
    let lighter = fg_luminance.max(bg_luminance);
    let darker = fg_luminance.min(bg_luminance);
    (lighter + 0.05) / (darker + 0.05)
}

fn relative_luminance(color: CssColor) -> f64 {
    let linear: LinSrgb<f32> = Srgb::new(color.r, color.g, color.b).into_linear();
    0.0722f64.mul_add(
        f64::from(linear.blue),
        0.2126f64.mul_add(f64::from(linear.red), 0.7152 * f64::from(linear.green)),
    )
}

fn parent_index(snapshot: &PlumbSnapshot) -> IndexMap<u64, u64> {
    snapshot
        .nodes
        .iter()
        .filter_map(|node| node.parent.map(|parent| (node.dom_order, parent)))
        .collect()
}

fn node_by_dom_order(snapshot: &PlumbSnapshot, dom_order: u64) -> Option<&SnapshotNode> {
    snapshot
        .nodes
        .iter()
        .find(|node| node.dom_order == dom_order)
}

fn resolve_background(
    snapshot: &PlumbSnapshot,
    parents: &IndexMap<u64, u64>,
    start: &SnapshotNode,
) -> CssColor {
    let mut layers = Vec::new();
    let mut current = Some(start.dom_order);

    while let Some(dom_order) = current {
        let Some(node) = node_by_dom_order(snapshot, dom_order) else {
            break;
        };
        if let Some(background) = node
            .computed_styles
            .get(BACKGROUND_COLOR)
            .and_then(|raw| parse_css_color(raw))
            && background.a > 0.0
        {
            let opaque = (background.a - 1.0).abs() < f32::EPSILON;
            layers.push(background);
            if opaque {
                break;
            }
        }
        current = parents.get(&dom_order).copied();
    }

    let mut backdrop = DEFAULT_BACKGROUND;
    for layer in layers.iter().rev() {
        backdrop = composite_over(*layer, backdrop);
    }
    backdrop
}

fn composite_over(src: CssColor, dst: CssColor) -> CssColor {
    let src_linear: LinSrgb<f32> = Srgb::new(src.r, src.g, src.b).into_linear();
    let dst_linear: LinSrgb<f32> = Srgb::new(dst.r, dst.g, dst.b).into_linear();
    let alpha = src.a;
    let inverse_alpha = 1.0 - alpha;
    let blended = LinSrgb::new(
        src_linear
            .red
            .mul_add(alpha, dst_linear.red * inverse_alpha),
        src_linear
            .green
            .mul_add(alpha, dst_linear.green * inverse_alpha),
        src_linear
            .blue
            .mul_add(alpha, dst_linear.blue * inverse_alpha),
    );
    let out: Srgb<f32> = Srgb::from_linear(blended);
    CssColor {
        r: out.red,
        g: out.green,
        b: out.blue,
        a: 1.0,
    }
}

fn rounded_json_number(value: f64) -> Option<serde_json::Value> {
    let rounded = (value * 1000.0).round() / 1000.0;
    serde_json::Number::from_f64(rounded).map(serde_json::Value::Number)
}

fn rounded_decimal_string(value: f64) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    format!("{rounded:.3}")
}

fn required_ratio(config: &Config, is_large: bool) -> f64 {
    let mut ratio = if is_large {
        LARGE_TEXT_MIN_RATIO
    } else {
        NORMAL_TEXT_MIN_RATIO
    };
    if let Some(min_ratio) = config.a11y.min_contrast_ratio
        && min_ratio.is_finite()
        && min_ratio > 0.0
    {
        ratio = ratio.max(f64::from(min_ratio));
    }
    ratio
}

fn violation_message(
    node: &SnapshotNode,
    measured_ratio: f64,
    required_ratio: f64,
    is_large: bool,
) -> String {
    format!(
        "`{selector}` has contrast ratio {ratio}:1; WCAG 2.1 AA requires at least {required}:1 for {kind} text.",
        selector = node.selector,
        ratio = rounded_decimal_string(measured_ratio),
        required = rounded_decimal_string(required_ratio),
        kind = if is_large { "large" } else { "normal" },
    )
}

fn fix_text(required_ratio: f64, is_large: bool) -> String {
    format!(
        "Increase the foreground/background contrast to at least {}:1 for this {} text.",
        rounded_decimal_string(required_ratio),
        if is_large { "large" } else { "normal" },
    )
}

fn build_metadata(
    node: &SnapshotNode,
    raw_foreground: &str,
    measured_ratio: f64,
    required_ratio: f64,
    font_size_px: f64,
    is_large: bool,
) -> IndexMap<String, serde_json::Value> {
    let mut metadata = IndexMap::new();
    metadata.insert(
        "contrast_ratio".to_owned(),
        rounded_json_number(measured_ratio).unwrap_or(serde_json::Value::Null),
    );
    metadata.insert(
        "required_ratio".to_owned(),
        rounded_json_number(required_ratio).unwrap_or(serde_json::Value::Null),
    );
    metadata.insert(
        "font_size_px".to_owned(),
        rounded_json_number(font_size_px).unwrap_or(serde_json::Value::Null),
    );
    metadata.insert("large_text".to_owned(), serde_json::Value::Bool(is_large));
    metadata.insert(
        "foreground_color".to_owned(),
        serde_json::Value::String(raw_foreground.to_owned()),
    );
    if let Some(raw_weight) = node.computed_styles.get(FONT_WEIGHT) {
        metadata.insert(
            "font_weight".to_owned(),
            serde_json::Value::String(raw_weight.clone()),
        );
    }
    metadata
}

#[cfg(test)]
mod tests {
    use super::{
        CssColor, LARGE_BOLD_TEXT_MIN_PX, LARGE_TEXT_MIN_PX, classify_large_text, composite_over,
        contrast_ratio, is_bold_weight, parse_font_weight,
    };

    #[test]
    fn classifies_large_text_per_wcag_thresholds() {
        assert!(classify_large_text(LARGE_TEXT_MIN_PX, Some(400)));
        assert!(!classify_large_text(LARGE_TEXT_MIN_PX - 0.1, Some(400)));
        assert!(classify_large_text(LARGE_BOLD_TEXT_MIN_PX, Some(700)));
        assert!(!classify_large_text(
            LARGE_BOLD_TEXT_MIN_PX - 0.1,
            Some(700)
        ));
    }

    #[test]
    fn bold_weight_threshold_is_700() {
        assert!(!is_bold_weight(600));
        assert!(is_bold_weight(700));
        assert!(is_bold_weight(900));
    }

    #[test]
    fn parse_font_weight_handles_keywords_and_numbers() {
        assert_eq!(parse_font_weight("normal"), Some(400));
        assert_eq!(parse_font_weight("bold"), Some(700));
        assert_eq!(parse_font_weight("700"), Some(700));
        assert_eq!(parse_font_weight("garbage"), None);
    }

    #[test]
    fn contrast_ratio_matches_wcag_reference_pairs() {
        let black = CssColor {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let white = CssColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let ratio = contrast_ratio(black, white);
        assert!((ratio - 21.0).abs() < 1e-6);
    }

    #[test]
    fn composite_over_returns_opaque_color() {
        let translucent_black = CssColor {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.5,
        };
        let white = CssColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let composited = composite_over(translucent_black, white);
        assert!((composited.a - 1.0).abs() < 1e-6);
        assert!(composited.r > 0.5 && composited.r < 0.85);
    }
}
