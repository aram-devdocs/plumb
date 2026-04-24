//! Internal helpers shared by the built-in rules.
//!
//! Rules in `plumb-core` are pure functions of `(snapshot, config)`. The
//! shared helpers here encapsulate CSS-pixel parsing and discrete-scale
//! lookup so rule modules stay focused on their domain logic.
//!
//! All helpers are `pub(crate)` — they are an implementation detail of
//! the rule modules, not a stable surface.

#![allow(clippy::redundant_pub_crate)]

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
    let magnitude = (value.abs() / base_f).round() * base_f;
    let signed = if value.is_sign_negative() {
        -magnitude
    } else {
        magnitude
    };
    signed.round() as i64
}

/// Return the closest member of `scale` to `value` by absolute delta.
///
/// `None` iff `scale.is_empty()`. Tie-break: the lower scale value
/// wins, which is deterministic and matches "snap toward zero" in
/// the symmetric case (e.g. `value = 14.0` against `[12, 16]` picks
/// `12` only on a strict-less comparison).
///
/// `value.abs()` is the comparison axis — negative inputs are treated
/// like their positive counterparts, since spacing scales are
/// non-negative by convention.
// Used by sibling rules in subsequent commits.
#[allow(dead_code, clippy::float_cmp)]
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
