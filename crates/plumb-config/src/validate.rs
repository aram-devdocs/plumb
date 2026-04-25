//! Post-deserialization semantic validation.
//!
//! Returns a single [`ValidationIssue`] describing the first invalid
//! value encountered. The walk is deterministic (uses [`indexmap`]
//! iteration order), so repeated runs against the same config surface
//! the same diagnostic.

#![allow(clippy::redundant_pub_crate)]

use plumb_core::Config;

/// A single semantic validation problem.
#[derive(Debug, Clone)]
pub(crate) struct ValidationIssue {
    /// Dotted path of the offending value, broken into segments suitable
    /// for [`crate::span::locate_path`]. Map keys appear as their key
    /// strings (no quoting).
    pub(crate) path_segments: Vec<String>,
    /// Human-readable explanation of what's wrong.
    pub(crate) message: String,
}

/// Walk the deserialized [`Config`] and return the first semantic
/// validation problem, or `None` if everything is valid.
pub(crate) fn validate(cfg: &Config) -> Option<ValidationIssue> {
    for (token, value) in &cfg.color.tokens {
        if !is_valid_hex_color(value) {
            return Some(ValidationIssue {
                path_segments: vec!["color".to_owned(), "tokens".to_owned(), token.clone()],
                message: format!(
                    "`{value}` is not a valid hex color (expected `#rgb`, `#rgba`, `#rrggbb`, or `#rrggbbaa`)"
                ),
            });
        }
    }
    None
}

/// Returns `true` if `value` is a `#`-prefixed hex color of length
/// 3, 4, 6, or 8 nibbles.
pub(crate) fn is_valid_hex_color(value: &str) -> bool {
    let Some(body) = value.strip_prefix('#') else {
        return false;
    };
    let len = body.len();
    if !(len == 3 || len == 4 || len == 6 || len == 8) {
        return false;
    }
    body.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::is_valid_hex_color;

    #[test]
    fn accepts_canonical_six_digit_hex() {
        assert!(is_valid_hex_color("#0b7285"));
        assert!(is_valid_hex_color("#FFFFFF"));
    }

    #[test]
    fn accepts_short_and_alpha_forms() {
        assert!(is_valid_hex_color("#fff"));
        assert!(is_valid_hex_color("#fffe"));
        assert!(is_valid_hex_color("#0b728580"));
    }

    #[test]
    fn rejects_missing_hash_or_bad_chars() {
        assert!(!is_valid_hex_color("0b7285"));
        assert!(!is_valid_hex_color("#0b72g5"));
        assert!(!is_valid_hex_color("#12345"));
        assert!(!is_valid_hex_color("not-a-hex"));
        assert!(!is_valid_hex_color(""));
        assert!(!is_valid_hex_color("#"));
    }
}
