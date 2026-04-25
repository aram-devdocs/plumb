//! `color/palette-conformance` — flag computed colors that aren't on
//! the configured palette, measured by CIEDE2000 (ΔE00) in CIE Lab.
//!
//! Per acceptance criteria of the rule:
//!
//! - The palette is parsed once per `check` call, never per node.
//! - Colors with non-1.0 alpha are composited over the closest opaque
//!   ancestor `background-color` (defaulting to white) before the ΔE
//!   measurement, so a translucent overlay is judged against what the
//!   user actually sees.
//! - Properties iterated: `color`, `background-color`, the four
//!   `border-*-color` longhands, and `outline-color`. One violation
//!   per `(node, property)` pair.

use indexmap::IndexMap;
use palette::IntoColor;
use palette::color_difference::Ciede2000;
use palette::white_point::D65;
use palette::{Lab, LinSrgb, Srgb};

use crate::config::Config;
use crate::report::{Confidence, Fix, FixKind, Severity, Violation, ViolationSink};
use crate::rules::Rule;
use crate::rules::color::COLOR_PROPERTIES;
use crate::rules::util::{CssColor, parse_css_color};
use crate::snapshot::{SnapshotCtx, SnapshotNode};

/// Background assumed when no opaque ancestor declares a
/// `background-color`. Matches the User Agent default for HTML.
const DEFAULT_BACKGROUND: CssColor = CssColor {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

/// One palette token, pre-converted to CIE Lab (D65) for ΔE00.
struct PaletteEntry {
    name: String,
    hex: String,
    lab: Lab<D65, f32>,
}

/// Flags computed colors whose CIEDE2000 distance to every palette
/// token exceeds `color.delta_e_tolerance`.
#[derive(Debug, Clone, Copy)]
pub struct PaletteConformance;

impl Rule for PaletteConformance {
    fn id(&self) -> &'static str {
        "color/palette-conformance"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "Flags computed colors that aren't on `color.tokens` (CIEDE2000)."
    }

    fn check(&self, ctx: &SnapshotCtx<'_>, config: &Config, sink: &mut ViolationSink<'_>) {
        let palette = build_palette(config);
        if palette.is_empty() {
            // Empty palette is a no-op. Without an allow-list, flagging
            // every color would be noise.
            return;
        }
        let tolerance = config.color.delta_e_tolerance;
        if !tolerance.is_finite() || tolerance < 0.0 {
            // Defensive: the schema enforces a non-negative tolerance,
            // but a malformed runtime config shouldn't panic. Skip.
            return;
        }

        let snapshot = ctx.snapshot();
        let parents = parent_index(snapshot);

        for node in ctx.nodes() {
            for prop in COLOR_PROPERTIES {
                let Some(raw) = node.computed_styles.get(*prop) else {
                    continue;
                };
                let Some(parsed) = parse_css_color(raw) else {
                    continue;
                };
                if parsed.a <= 0.0 {
                    // `transparent` and zero-alpha values have no
                    // visible color — skip rather than match against
                    // the default background.
                    continue;
                }
                let effective = if (parsed.a - 1.0).abs() < f32::EPSILON {
                    parsed
                } else {
                    let backdrop = resolve_backdrop(snapshot, &parents, node);
                    composite_over(parsed, backdrop)
                };
                let candidate_lab: Lab<D65, f32> = srgb_to_lab(effective.into_srgb());

                let Some(nearest) = nearest_palette_entry(&palette, candidate_lab) else {
                    continue;
                };
                if f64::from(nearest.delta) <= f64::from(tolerance) {
                    continue;
                }

                let entry = &palette[nearest.index];
                let mut metadata = IndexMap::new();
                metadata.insert((*prop).to_owned(), serde_json::Value::String(raw.clone()));
                metadata.insert(
                    "nearest_token".to_owned(),
                    serde_json::Value::String(entry.name.clone()),
                );
                metadata.insert(
                    "nearest_token_hex".to_owned(),
                    serde_json::Value::String(entry.hex.clone()),
                );
                metadata.insert(
                    "delta_e".to_owned(),
                    delta_e_metadata(nearest.delta).unwrap_or(serde_json::Value::Null),
                );
                metadata.insert(
                    "delta_e_tolerance".to_owned(),
                    delta_e_metadata(tolerance).unwrap_or(serde_json::Value::Null),
                );

                sink.push(Violation {
                    rule_id: self.id().to_owned(),
                    severity: self.default_severity(),
                    message: format!(
                        "`{selector}` has off-palette {prop} {raw}; nearest token is `{token}` ({hex}).",
                        selector = node.selector,
                        token = entry.name,
                        hex = entry.hex,
                    ),
                    selector: node.selector.clone(),
                    viewport: ctx.snapshot().viewport.clone(),
                    rect: ctx.rect_for(node.dom_order),
                    dom_order: node.dom_order,
                    fix: Some(Fix {
                        kind: FixKind::CssPropertyReplace {
                            property: (*prop).to_owned(),
                            from: raw.clone(),
                            to: entry.hex.clone(),
                        },
                        description: format!(
                            "Snap `{prop}` to the nearest palette token `{token}` ({hex}).",
                            token = entry.name,
                            hex = entry.hex,
                        ),
                        confidence: Confidence::Medium,
                    }),
                    doc_url: "https://plumb.aramhammoudeh.com/rules/color-palette-conformance"
                        .to_owned(),
                    metadata,
                });
            }
        }
    }
}

fn build_palette(config: &Config) -> Vec<PaletteEntry> {
    let mut out = Vec::with_capacity(config.color.tokens.len());
    for (name, hex) in &config.color.tokens {
        let Some(parsed) = parse_css_color(hex) else {
            // Tokens that aren't parseable hex are skipped silently
            // rather than panicking. The config-loader is the right
            // place to validate; the rule MUST stay pure.
            continue;
        };
        if parsed.a <= 0.0 {
            continue;
        }
        let lab = srgb_to_lab(parsed.into_srgb());
        out.push(PaletteEntry {
            name: name.clone(),
            hex: hex.clone(),
            lab,
        });
    }
    out
}

fn srgb_to_lab(rgb: Srgb<f32>) -> Lab<D65, f32> {
    // `palette` chains the conversion through `LinSrgb` and `Xyz`.
    // Going through `LinSrgb` explicitly keeps the gamma-decode step
    // visible at the call site — composite math runs in linear space,
    // ΔE math in Lab.
    let linear: LinSrgb<f32> = rgb.into_linear();
    linear.into_color()
}

struct Nearest {
    index: usize,
    delta: f32,
}

fn nearest_palette_entry(palette: &[PaletteEntry], candidate: Lab<D65, f32>) -> Option<Nearest> {
    let mut best: Option<Nearest> = None;
    for (idx, entry) in palette.iter().enumerate() {
        let delta = candidate.difference(entry.lab);
        match best.as_mut() {
            None => best = Some(Nearest { index: idx, delta }),
            Some(current) => {
                // Strictly less keeps the first-seen tie-winner
                // (deterministic given `IndexMap` insertion order).
                if delta < current.delta {
                    current.index = idx;
                    current.delta = delta;
                }
            }
        }
    }
    best
}

fn delta_e_metadata(value: f32) -> Option<serde_json::Value> {
    let rounded = (f64::from(value) * 1000.0).round() / 1000.0;
    serde_json::Number::from_f64(rounded).map(serde_json::Value::Number)
}

fn parent_index(snapshot: &crate::snapshot::PlumbSnapshot) -> IndexMap<u64, u64> {
    snapshot
        .nodes
        .iter()
        .filter_map(|n| n.parent.map(|p| (n.dom_order, p)))
        .collect()
}

fn node_by_dom_order(
    snapshot: &crate::snapshot::PlumbSnapshot,
    dom_order: u64,
) -> Option<&SnapshotNode> {
    snapshot.nodes.iter().find(|n| n.dom_order == dom_order)
}

fn resolve_backdrop(
    snapshot: &crate::snapshot::PlumbSnapshot,
    parents: &IndexMap<u64, u64>,
    start: &SnapshotNode,
) -> CssColor {
    // Walk up the DOM ancestor chain looking for the closest
    // `background-color` with full alpha. If we never find one, fall
    // back to the UA default (white). The walk MUST start at the
    // parent — the start node's own colour is what we're judging.
    let mut current = parents.get(&start.dom_order).copied();
    while let Some(dom_order) = current {
        let Some(node) = node_by_dom_order(snapshot, dom_order) else {
            break;
        };
        if let Some(bg) = node
            .computed_styles
            .get("background-color")
            .and_then(|raw| parse_css_color(raw))
            && (bg.a - 1.0).abs() < f32::EPSILON
        {
            return bg;
        }
        current = parents.get(&dom_order).copied();
    }
    DEFAULT_BACKGROUND
}

fn composite_over(src: CssColor, dst: CssColor) -> CssColor {
    // Standard "source over" Porter–Duff in linear-light space, which
    // is the only physically correct compositing space for sRGB
    // alpha blending. Convert sRGB → linear, blend, convert back.
    let s_lin: LinSrgb<f32> = Srgb::new(src.r, src.g, src.b).into_linear();
    let d_lin: LinSrgb<f32> = Srgb::new(dst.r, dst.g, dst.b).into_linear();
    let alpha = src.a;
    let inv = 1.0 - alpha;
    // Pre-multiplied "over": `out = s*alpha + d*(1-alpha)` (assuming
    // the destination is fully opaque, which `resolve_backdrop`
    // guarantees by walking until alpha == 1.0 or hitting white).
    let blended = LinSrgb::new(
        s_lin.red.mul_add(alpha, d_lin.red * inv),
        s_lin.green.mul_add(alpha, d_lin.green * inv),
        s_lin.blue.mul_add(alpha, d_lin.blue * inv),
    );
    let out: Srgb<f32> = Srgb::from_linear(blended);
    CssColor {
        r: out.red,
        g: out.green,
        b: out.blue,
        a: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_BACKGROUND, build_palette, composite_over, nearest_palette_entry, srgb_to_lab,
    };
    use crate::config::{ColorSpec, Config};
    use crate::rules::util::{CssColor, parse_css_color};
    use indexmap::IndexMap;

    #[test]
    fn build_palette_skips_unparseable_tokens() {
        let mut tokens = IndexMap::new();
        tokens.insert("primary".into(), "#0b7285".into());
        tokens.insert("garbage".into(), "not-a-color".into());
        let config = Config {
            color: ColorSpec {
                tokens,
                delta_e_tolerance: 2.0,
            },
            ..Config::default()
        };
        let palette = build_palette(&config);
        assert_eq!(palette.len(), 1);
        assert_eq!(palette[0].name, "primary");
    }

    #[test]
    fn nearest_palette_entry_picks_minimum_delta() {
        let mut tokens = IndexMap::new();
        tokens.insert("white".into(), "#ffffff".into());
        tokens.insert("black".into(), "#000000".into());
        tokens.insert("primary".into(), "#0b7285".into());
        let config = Config {
            color: ColorSpec {
                tokens,
                delta_e_tolerance: 2.0,
            },
            ..Config::default()
        };
        let palette = build_palette(&config);

        // A near-black candidate.
        let cand = parse_css_color("#020202").expect("parse near-black");
        let lab = srgb_to_lab(cand.into_srgb());
        let nearest = nearest_palette_entry(&palette, lab).expect("non-empty palette");
        assert_eq!(palette[nearest.index].name, "black");
    }

    #[test]
    fn composite_over_respects_alpha_zero_and_one() {
        let red = parse_css_color("rgba(255, 0, 0, 1.0)").expect("opaque red");
        let composited = composite_over(red, DEFAULT_BACKGROUND);
        // Fully opaque source must come back unchanged.
        assert!((composited.r - 1.0).abs() < 1e-4);
        assert!((composited.g - 0.0).abs() < 1e-4);
        assert!((composited.b - 0.0).abs() < 1e-4);
        assert!((composited.a - 1.0).abs() < 1e-4);

        // Translucent black over white must land near 50% gray in
        // linear space — visibly mid-gray after gamma encode.
        let half_black = parse_css_color("rgba(0, 0, 0, 0.5)").expect("translucent black");
        let mid = composite_over(half_black, DEFAULT_BACKGROUND);
        assert!(mid.r > 0.5 && mid.r < 0.85);
    }

    #[test]
    fn composite_over_zero_alpha_returns_destination() {
        let zero = CssColor {
            r: 0.2,
            g: 0.2,
            b: 0.2,
            a: 0.0,
        };
        let result = composite_over(zero, DEFAULT_BACKGROUND);
        assert!((result.r - 1.0).abs() < 1e-4);
        assert!((result.g - 1.0).abs() < 1e-4);
        assert!((result.b - 1.0).abs() < 1e-4);
    }
}
