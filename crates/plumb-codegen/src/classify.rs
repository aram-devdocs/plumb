//! Classify scraped CSS custom properties into [`plumb_core::Config`]
//! token slots.
//!
//! The classifier is a pure function of `(scrapes, config)`: it folds
//! a scraped property name + value pair into either the color tokens
//! map, the spacing scale, the radius scale, or the type scale. Names
//! that don't match any pattern are skipped — the V0 contract is to
//! produce a *plausible* starter config, not to be exhaustive.
//!
//! ## Name patterns
//!
//! - **Color tokens.** Any `--color-*`, `--bg-*`, `--fg-*`, `--accent-*`,
//!   `--surface-*`, `--text-*`, `--border-*`, `--ring-*` declaration
//!   whose value is a [`ScrapedValue::Color`].
//! - **Spacing scale + tokens.** Any `--space-*`, `--spacing-*`,
//!   `--gap-*`, `--padding-*`, `--margin-*`, `--size-*` declaration
//!   whose value is a [`ScrapedValue::Px`].
//! - **Radius scale + tokens.** Any `--radius-*`, `--rounded-*`,
//!   `--corner-*` declaration whose value is a [`ScrapedValue::Px`].
//! - **Type scale + tokens.** Any `--font-size-*`, `--text-*` (size
//!   variants like `--text-base`, `--text-lg`), `--type-*`,
//!   `--leading-*` declaration whose value is a [`ScrapedValue::Px`].
//!   `--text-*` is shared with color names; we differentiate by value
//!   type — `Color` lands in colors, `Px` in type sizes.

// Items here are crate-private but live inside a private module; the
// `redundant_pub_crate` lint flips between deny on `pub(crate)` and the
// rust-level `unreachable_pub` lint on bare `pub`. Allow the former
// scoped to this module so the items keep the explicit visibility.
#![allow(clippy::redundant_pub_crate)]

use plumb_config::{CssPropertyScrape, ScrapedValue};
use plumb_core::Config;

/// Aggregate stats for the per-file summary.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct PerFileStats {
    /// Number of color tokens contributed.
    pub colors: usize,
    /// Number of dimensional tokens contributed (px values into spacing,
    /// radius, or type-size buckets).
    pub dimensions: usize,
    /// Other scraped properties (rem, em, font stacks, gradients, …).
    pub other: usize,
}

impl PerFileStats {
    pub(crate) fn increment(&mut self, value: &ScrapedValue) {
        match value {
            ScrapedValue::Color(_) => self.colors += 1,
            ScrapedValue::Px(_) => self.dimensions += 1,
            _ => self.other += 1,
        }
    }
}

/// Fold every `scrape` into the matching `config` slot.
pub(crate) fn classify_css_scrapes(scrapes: &[CssPropertyScrape], config: &mut Config) {
    for scrape in scrapes {
        // Property names always start with `--` per the scraper contract.
        let stem = scrape
            .name
            .strip_prefix("--")
            .unwrap_or(scrape.name.as_str());
        let lower = stem.to_ascii_lowercase();

        match &scrape.value {
            ScrapedValue::Color(hex) => {
                if is_color_name(&lower) {
                    config
                        .color
                        .tokens
                        .entry(stem.to_owned())
                        .or_insert_with(|| hex.clone());
                }
            }
            ScrapedValue::Px(px) => {
                if is_radius_name(&lower) {
                    config.radius.scale.push(*px);
                } else if is_spacing_name(&lower) {
                    config.spacing.scale.push(*px);
                    config.spacing.tokens.entry(stem.to_owned()).or_insert(*px);
                } else if is_type_size_name(&lower) {
                    config.type_scale.scale.push(*px);
                    config
                        .type_scale
                        .tokens
                        .entry(stem.to_owned())
                        .or_insert(*px);
                }
            }
            // V0 leaves rem / em / Other untouched. Translating rem to
            // px requires an explicit `--root-font-size: 16px` baseline
            // we don't yet plumb through; keeping these out of the
            // starter config beats guessing.
            ScrapedValue::Rem(_) | ScrapedValue::Em(_) | ScrapedValue::Other(_) => {}
        }
    }
}

fn is_color_name(name: &str) -> bool {
    static PREFIXES: &[&str] = &[
        "color-", "bg-", "fg-", "accent-", "surface-", "text-", "border-", "ring-",
    ];
    PREFIXES.iter().any(|p| name.starts_with(p))
}

fn is_spacing_name(name: &str) -> bool {
    static PREFIXES: &[&str] = &["space-", "spacing-", "gap-", "padding-", "margin-", "size-"];
    PREFIXES.iter().any(|p| name.starts_with(p))
}

fn is_radius_name(name: &str) -> bool {
    static PREFIXES: &[&str] = &["radius-", "rounded-", "corner-"];
    PREFIXES.iter().any(|p| name.starts_with(p))
}

fn is_type_size_name(name: &str) -> bool {
    static PREFIXES: &[&str] = &["font-size-", "type-", "leading-"];
    if PREFIXES.iter().any(|p| name.starts_with(p)) {
        return true;
    }
    // `--text-*` collides with the color namespace. We reach this branch
    // only when the value was Px, so a type-size interpretation is the
    // only one available.
    name.starts_with("text-")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scrape(name: &str, value: ScrapedValue) -> CssPropertyScrape {
        CssPropertyScrape {
            source: PathBuf::from("test.css"),
            at_rule: None,
            name: name.to_owned(),
            raw_value: String::new(),
            value,
        }
    }

    #[test]
    fn classifies_color_tokens() {
        let mut config = Config::default();
        let scrapes = vec![
            scrape("--color-bg", ScrapedValue::Color("#ffffff".into())),
            scrape("--accent-brand", ScrapedValue::Color("#0b7285".into())),
            scrape("--unknown", ScrapedValue::Color("#deadbe".into())),
        ];
        classify_css_scrapes(&scrapes, &mut config);
        assert_eq!(config.color.tokens.len(), 2);
        assert_eq!(
            config.color.tokens.get("color-bg"),
            Some(&"#ffffff".to_owned())
        );
    }

    #[test]
    fn classifies_spacing_tokens() {
        let mut config = Config::default();
        let scrapes = vec![
            scrape("--space-xs", ScrapedValue::Px(4)),
            scrape("--space-sm", ScrapedValue::Px(8)),
            scrape("--gap-md", ScrapedValue::Px(16)),
        ];
        classify_css_scrapes(&scrapes, &mut config);
        assert_eq!(config.spacing.scale, vec![4, 8, 16]);
        assert_eq!(config.spacing.tokens.len(), 3);
    }

    #[test]
    fn classifies_radius_tokens() {
        let mut config = Config::default();
        let scrapes = vec![
            scrape("--radius-sm", ScrapedValue::Px(2)),
            scrape("--radius-md", ScrapedValue::Px(8)),
            scrape("--rounded-full", ScrapedValue::Px(9999)),
        ];
        classify_css_scrapes(&scrapes, &mut config);
        assert_eq!(config.radius.scale, vec![2, 8, 9999]);
    }

    #[test]
    fn classifies_text_color_vs_text_size_by_value_kind() {
        let mut config = Config::default();
        let scrapes = vec![
            scrape("--text-base", ScrapedValue::Px(16)),
            scrape("--text-primary", ScrapedValue::Color("#0b0b0b".into())),
        ];
        classify_css_scrapes(&scrapes, &mut config);
        assert_eq!(config.type_scale.scale, vec![16]);
        assert_eq!(config.color.tokens.len(), 1);
        assert!(config.color.tokens.contains_key("text-primary"));
    }

    #[test]
    fn rem_and_em_skip_classification() {
        let mut config = Config::default();
        let scrapes = vec![
            scrape("--space-md", ScrapedValue::Rem(1.0)),
            scrape("--space-lg", ScrapedValue::Em(1.5)),
        ];
        classify_css_scrapes(&scrapes, &mut config);
        assert!(config.spacing.scale.is_empty());
    }
}
