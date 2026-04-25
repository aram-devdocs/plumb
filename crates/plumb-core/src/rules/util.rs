//! Internal helpers shared by the built-in rules.
//!
//! Rules in `plumb-core` are pure functions of `(snapshot, config)`. The
//! shared helpers here encapsulate CSS-pixel parsing, CSS-color parsing,
//! and discrete-scale lookup so rule modules stay focused on their
//! domain logic.
//!
//! All helpers are `pub(crate)` — they are an implementation detail of
//! the rule modules, not a stable surface.

#![allow(clippy::redundant_pub_crate)]

use palette::Srgb;
use std::str::FromStr;

/// Parsed CSS color in the (gamma-encoded) sRGB color space.
///
/// Components are non-linear sRGB in `[0.0, 1.0]`, matching the
/// encoding of `palette::Srgb<f32>`. Alpha is in `[0.0, 1.0]`, with
/// `1.0` meaning fully opaque.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CssColor {
    /// sRGB red, gamma-encoded, `[0.0, 1.0]`.
    pub(crate) r: f32,
    /// sRGB green, gamma-encoded, `[0.0, 1.0]`.
    pub(crate) g: f32,
    /// sRGB blue, gamma-encoded, `[0.0, 1.0]`.
    pub(crate) b: f32,
    /// Alpha channel, `[0.0, 1.0]`.
    pub(crate) a: f32,
}

impl CssColor {
    /// Build from byte (0..=255) channels and an alpha in `[0.0, 1.0]`.
    fn from_rgb_u8_alpha(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self {
            r: f32::from(r) / 255.0,
            g: f32::from(g) / 255.0,
            b: f32::from(b) / 255.0,
            a,
        }
    }

    /// View as a `palette::Srgb<f32>` (alpha discarded).
    pub(crate) fn into_srgb(self) -> Srgb<f32> {
        Srgb::new(self.r, self.g, self.b)
    }
}

/// Parse the CSS color shapes that `getComputedStyle` ever returns
/// after Chromium's normalization, plus a few hand-friendly forms used
/// by Plumb config tokens.
///
/// Accepted shapes:
///
/// - `"transparent"` — returns alpha == 0 (caller MUST skip).
/// - `"#rgb"`, `"#rrggbb"`, `"#rgba"`, `"#rrggbbaa"` — hex with
///   optional alpha.
/// - `"rgb(r, g, b)"` — decimal channels, no alpha (`a = 1.0`).
/// - `"rgba(r, g, b, a)"` — decimal channels and alpha in `[0, 1]`.
///
/// Whitespace is tolerated. Anything else (named colors other than
/// `transparent`, `hsl()`, `hsla()`, `color()`, etc.) returns `None`
/// so the caller can skip silently. Chromium's resolved-style output
/// for any color other than `transparent` is `rgb(...)` or `rgba(...)`,
/// so this covers every snapshot value Plumb sees in practice; the
/// hex paths handle palette tokens defined in `plumb.toml`.
#[must_use]
pub(crate) fn parse_css_color(s: &str) -> Option<CssColor> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.eq_ignore_ascii_case("transparent") {
        return Some(CssColor {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        });
    }
    if trimmed.starts_with('#') {
        return parse_hex(trimmed);
    }
    if let Some(rest) = strip_ci_prefix(trimmed, "rgba") {
        return parse_rgb_functional(rest);
    }
    if let Some(rest) = strip_ci_prefix(trimmed, "rgb") {
        return parse_rgb_functional(rest);
    }
    None
}

fn strip_ci_prefix<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let plen = prefix.len();
    if s.len() < plen {
        return None;
    }
    let (head, tail) = s.split_at(plen);
    if head.eq_ignore_ascii_case(prefix) {
        Some(tail)
    } else {
        None
    }
}

fn parse_hex(input: &str) -> Option<CssColor> {
    // Route by hex length — `palette::Srgb::<u8>::from_str` covers the
    // 3 / 6 cases. The 4 / 8 (with alpha) shapes need a hand-rolled
    // split because palette's `Rgba` FromStr is generic over `Alpha`.
    let hex = input.strip_prefix('#').unwrap_or(input);
    match hex.len() {
        3 | 6 => {
            let rgb: Srgb<u8> = Srgb::from_str(input).ok()?;
            Some(CssColor::from_rgb_u8_alpha(
                rgb.red, rgb.green, rgb.blue, 1.0,
            ))
        }
        4 => {
            let red = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let green = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let blue = u8::from_str_radix(&hex[2..3], 16).ok()?;
            let alpha = u8::from_str_radix(&hex[3..4], 16).ok()?;
            Some(CssColor::from_rgb_u8_alpha(
                red * 17,
                green * 17,
                blue * 17,
                f32::from(alpha * 17) / 255.0,
            ))
        }
        8 => {
            let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let alpha = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(CssColor::from_rgb_u8_alpha(
                red,
                green,
                blue,
                f32::from(alpha) / 255.0,
            ))
        }
        _ => None,
    }
}

fn parse_rgb_functional(input: &str) -> Option<CssColor> {
    let trimmed = input.trim();
    let inner = trimmed.strip_prefix('(')?.strip_suffix(')')?.trim();
    // Tolerate both comma and whitespace separation. Chromium emits
    // `rgb(255, 0, 0)` with commas; CSS Color 4 also allows
    // `rgb(255 0 0)`. Splitting on either keeps the parser usable
    // for hand-written palette config.
    let parts: Vec<&str> = if inner.contains(',') {
        inner.split(',').map(str::trim).collect()
    } else {
        inner.split_whitespace().collect()
    };

    let (red_s, green_s, blue_s, alpha_s) = match parts.as_slice() {
        [r, g, b] => (*r, *g, *b, None),
        [r, g, b, a] => (*r, *g, *b, Some(*a)),
        _ => return None,
    };
    let alpha = match alpha_s {
        Some(token) => parse_alpha(token)?,
        // The function name (`rgb` vs `rgba`) does not constrain the
        // channel count — `rgba(r, g, b)` is silently treated as
        // opaque. Chromium normalizes both to the same shape.
        None => 1.0,
    };
    let red = parse_channel(red_s)?;
    let green = parse_channel(green_s)?;
    let blue = parse_channel(blue_s)?;
    Some(CssColor::from_rgb_u8_alpha(red, green, blue, alpha))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn parse_channel(s: &str) -> Option<u8> {
    let trimmed = s.trim();
    if let Some(pct) = trimmed.strip_suffix('%') {
        let v = pct.trim().parse::<f32>().ok()?;
        let scaled = (v / 100.0).clamp(0.0, 1.0) * 255.0;
        // Clamp keeps the cast safe; `round` is half-away-from-zero.
        return Some(scaled.round().clamp(0.0, 255.0) as u8);
    }
    let v = trimmed.parse::<f32>().ok()?;
    // Accept fractional channel values (CSS Color 4) by rounding to
    // the nearest integer.
    Some(v.round().clamp(0.0, 255.0) as u8)
}

fn parse_alpha(s: &str) -> Option<f32> {
    let trimmed = s.trim();
    if let Some(pct) = trimmed.strip_suffix('%') {
        let v = pct.trim().parse::<f32>().ok()?;
        return Some((v / 100.0).clamp(0.0, 1.0));
    }
    let v = trimmed.parse::<f32>().ok()?;
    Some(v.clamp(0.0, 1.0))
}

/// Parse a CSS pixel value into an `f64`.
///
/// Accepts the small subset of inputs Plumb's rules ever look at:
///
/// - `"0"` and `"-0"` (no unit) — returned as `0.0`.
/// - `"<n>px"` and `"-<n>px"` where `<n>` parses as `f64`, optionally
///   with a fractional part (e.g. `"4.5px"`).
///
/// Anything else — `"auto"`, `"normal"`, the empty string,
/// `"calc(...)"`, `"<n>em"`, `"<n>rem"`, `"<n>%"`, etc. — returns
/// `None` so the caller can skip the property silently.
#[must_use]
pub(crate) fn parse_px(s: &str) -> Option<f64> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "0" || trimmed == "-0" {
        return Some(0.0);
    }
    let stripped = trimmed.strip_suffix("px")?;
    if stripped.is_empty() {
        return None;
    }
    stripped.parse::<f64>().ok()
}

/// Return the signed nearest multiple of `base` to `value`.
///
/// Returns `i64` so the caller can format negatives back into the
/// original `<n>px` shape (e.g. `-13px` snaps to `-12px` against
/// base 4). Sign is preserved by rounding the magnitude and re-applying
/// the sign of `value`.
///
/// Tie-break: `f64::round` rounds half away from zero, so `14.0`
/// against base `4` snaps to `16` (not `12`). Compare with
/// [`nearest_in_scale`], which breaks ties toward the lower scale
/// member — the two helpers intentionally differ because the grid
/// has no ordering hint while a discrete scale does.
///
/// If `base == 0`, the magnitude is undefined; callers MUST guard
/// against that case before invoking this helper. As a defensive
/// fallback this returns `value.round() as i64` — never panics.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn nearest_multiple(value: f64, base: u32) -> i64 {
    if base == 0 {
        // Defensive: callers must skip when base_unit is zero. We
        // return a rounded value rather than panicking.
        return value.round() as i64;
    }
    let base_f = f64::from(base);
    // `magnitude` is `n * base_f` for integer `n`, so the result is
    // already an exact f64-representable integer for any `base ≤ 2^24`
    // — no further rounding needed before casting.
    let magnitude = (value.abs() / base_f).round() * base_f;
    let signed = if value.is_sign_negative() {
        -magnitude
    } else {
        magnitude
    };
    signed as i64
}

/// Return the closest member of `scale` to `value` by absolute delta.
///
/// `None` iff `scale.is_empty()`. Tie-break: the lower scale value
/// wins, which is deterministic and matches "snap toward zero" in
/// the symmetric case (e.g. `value = 14.0` against `[12, 16]` picks
/// `12` only on a strict-less comparison). Compare with
/// [`nearest_multiple`], which breaks ties by rounding half away
/// from zero — the discrete scale has an ordering hint to lean on,
/// the continuous grid does not.
///
/// `value.abs()` is the comparison axis — negative inputs are treated
/// like their positive counterparts, since spacing scales are
/// non-negative by convention.
#[allow(clippy::float_cmp)]
#[must_use]
pub(crate) fn nearest_in_scale(value: f64, scale: &[u32]) -> Option<u32> {
    if scale.is_empty() {
        return None;
    }
    let target = value.abs();
    let mut best: Option<(u32, f64)> = None;
    for &candidate in scale {
        let delta = (f64::from(candidate) - target).abs();
        match best {
            None => best = Some((candidate, delta)),
            Some((current, current_delta)) => {
                // Exact equality is intentional: `delta` is computed the
                // same way for every candidate, so a true tie is exactly
                // representable in `f64`.
                if delta < current_delta || (delta == current_delta && candidate < current) {
                    best = Some((candidate, delta));
                }
            }
        }
    }
    best.map(|(value, _)| value)
}

#[cfg(test)]
mod tests {
    use super::{nearest_in_scale, nearest_multiple, parse_px};

    #[test]
    fn parse_px_accepts_supported_shapes() {
        assert_eq!(parse_px("0"), Some(0.0));
        assert_eq!(parse_px("-0"), Some(0.0));
        assert_eq!(parse_px("0px"), Some(0.0));
        assert_eq!(parse_px("16px"), Some(16.0));
        assert_eq!(parse_px("4.5px"), Some(4.5));
        assert_eq!(parse_px("-4px"), Some(-4.0));
        assert_eq!(parse_px("  12px  "), Some(12.0));
    }

    #[test]
    fn parse_px_rejects_unsupported_units_and_keywords() {
        assert_eq!(parse_px(""), None);
        assert_eq!(parse_px("auto"), None);
        assert_eq!(parse_px("normal"), None);
        assert_eq!(parse_px("calc(4px + 4px)"), None);
        assert_eq!(parse_px("1em"), None);
        assert_eq!(parse_px("2rem"), None);
        assert_eq!(parse_px("50%"), None);
        assert_eq!(parse_px("px"), None);
    }

    #[test]
    fn nearest_multiple_handles_positive_values() {
        assert_eq!(nearest_multiple(13.0, 4), 12);
        assert_eq!(nearest_multiple(14.0, 4), 16);
        assert_eq!(nearest_multiple(0.0, 4), 0);
        assert_eq!(nearest_multiple(7.9, 4), 8);
    }

    #[test]
    fn nearest_multiple_preserves_sign_for_negatives() {
        assert_eq!(nearest_multiple(-13.0, 4), -12);
        assert_eq!(nearest_multiple(-2.0, 4), -4);
        assert_eq!(nearest_multiple(-4.0, 4), -4);
    }

    #[test]
    fn nearest_multiple_zero_base_falls_back_to_round() {
        // Defensive — callers should skip when base_unit is 0, but the
        // helper must never panic.
        assert_eq!(nearest_multiple(13.4, 0), 13);
        assert_eq!(nearest_multiple(-2.6, 0), -3);
    }

    #[test]
    fn nearest_in_scale_picks_closest_by_absolute_delta() {
        let scale = [0, 4, 8, 12, 16, 24, 32, 48];
        assert_eq!(nearest_in_scale(13.0, &scale), Some(12));
        assert_eq!(nearest_in_scale(13.99, &scale), Some(12));
        assert_eq!(nearest_in_scale(15.0, &scale), Some(16));
        assert_eq!(nearest_in_scale(0.0, &scale), Some(0));
    }

    #[test]
    fn nearest_in_scale_breaks_ties_toward_lower_value() {
        let scale = [12, 16];
        // Equidistant: 14 - 12 == 16 - 14. Lower wins.
        assert_eq!(nearest_in_scale(14.0, &scale), Some(12));
    }

    #[test]
    fn nearest_in_scale_treats_negatives_like_their_magnitude() {
        let scale = [0, 4, 8, 12];
        assert_eq!(nearest_in_scale(-9.0, &scale), Some(8));
    }

    #[test]
    fn nearest_in_scale_returns_none_for_empty_scale() {
        assert_eq!(nearest_in_scale(13.0, &[]), None);
    }
}
