//! Integration tests for the CSS custom-properties scraper.
//!
//! Covers:
//! - Plain `:root { ... }` blocks.
//! - `:root` inside `@media (prefers-color-scheme: dark)`.
//! - Multiple `:root` blocks in one file.
//! - Comments and quoted strings inside declarations.
//! - `rem` → px normalization at 16px = 1rem.
//! - `em` flagged as warning (surfaces as `ScrapedValue::Em`).
//! - Malformed CSS surfaces as `ConfigError::CssParse` with a span.

use std::path::PathBuf;

use plumb_config::{ConfigError, ScrapedValue, scrape_css_properties};

fn fixture(name: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", name]
        .iter()
        .collect()
}

#[test]
fn scrapes_top_level_root_block() {
    let path = fixture("tokens.css");
    let scrapes = scrape_css_properties(std::slice::from_ref(&path)).expect("scrape");

    let bg = scrapes
        .iter()
        .find(|s| s.name == "--bg-canvas" && s.at_rule.is_none())
        .expect("--bg-canvas in top-level :root");
    assert!(matches!(&bg.value, ScrapedValue::Color(c) if c == "#ffffff"));
    assert_eq!(bg.source, path);
}

#[test]
fn scrapes_root_inside_media_query() {
    let path = fixture("tokens.css");
    let scrapes = scrape_css_properties(&[path]).expect("scrape");

    let dark_bg = scrapes
        .iter()
        .find(|s| {
            s.name == "--bg-canvas"
                && s.at_rule.as_deref() == Some("@media (prefers-color-scheme: dark)")
        })
        .expect("dark-mode --bg-canvas");
    assert!(matches!(&dark_bg.value, ScrapedValue::Color(c) if c == "#0b0b0b"));
}

#[test]
fn scrapes_multiple_root_blocks_in_one_file() {
    let path = fixture("tokens.css");
    let scrapes = scrape_css_properties(&[path]).expect("scrape");

    let top_level: Vec<&str> = scrapes
        .iter()
        .filter(|s| s.at_rule.is_none())
        .map(|s| s.name.as_str())
        .collect();
    assert!(
        top_level.contains(&"--bg-canvas") && top_level.contains(&"--space-3"),
        "expected both top-level :root blocks to be merged: {top_level:?}"
    );
}

#[test]
fn scrapes_root_inside_supports_block() {
    let path = fixture("tokens.css");
    let scrapes = scrape_css_properties(&[path]).expect("scrape");

    let grid_gap = scrapes
        .iter()
        .find(|s| s.name == "--grid-gap")
        .expect("--grid-gap in @supports");
    assert_eq!(
        grid_gap.at_rule.as_deref(),
        Some("@supports (display: grid)")
    );
    assert!(matches!(grid_gap.value, ScrapedValue::Px(16)));
}

#[test]
fn handles_comments_and_quoted_strings_in_values() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("strings.css");
    std::fs::write(
        &path,
        "/* leading comment */\n\
         :root {\n\
             --font: \"Inter\", sans-serif; /* trailing comment */\n\
             --shout: 'a; b'; /* quoted semicolon */\n\
             --space: 8px;\n\
         }\n",
    )
    .expect("write css");

    let scrapes = scrape_css_properties(&[path]).expect("scrape");

    let font = scrapes.iter().find(|s| s.name == "--font").expect("--font");
    assert_eq!(font.raw_value, "\"Inter\", sans-serif");
    assert!(matches!(&font.value, ScrapedValue::Other(_)));

    let shout = scrapes
        .iter()
        .find(|s| s.name == "--shout")
        .expect("--shout");
    assert_eq!(shout.raw_value, "'a; b'");

    let space = scrapes
        .iter()
        .find(|s| s.name == "--space")
        .expect("--space");
    assert!(matches!(space.value, ScrapedValue::Px(8)));
}

#[test]
fn normalizes_rem_to_px_via_default_root() {
    let path = fixture("tokens.css");
    let scrapes = scrape_css_properties(&[path]).expect("scrape");

    let space2 = scrapes
        .iter()
        .find(|s| s.name == "--space-2")
        .expect("--space-2");
    // 0.5rem at 16px/rem = 8px.
    match &space2.value {
        ScrapedValue::Rem(r) => assert!((*r - 0.5).abs() < f32::EPSILON),
        other => panic!("expected Rem, got {other:?}"),
    }
    assert_eq!(space2.raw_value, "0.5rem");
}

#[test]
fn em_surfaces_as_em_variant() {
    let path = fixture("tokens.css");
    let scrapes = scrape_css_properties(&[path]).expect("scrape");

    let gap = scrapes
        .iter()
        .find(|s| s.name == "--gap-em")
        .expect("--gap-em");
    match &gap.value {
        ScrapedValue::Em(v) => assert!((*v - 1.5).abs() < f32::EPSILON),
        other => panic!("expected Em, got {other:?}"),
    }
}

#[test]
fn unitless_or_string_value_surfaces_as_other() {
    let path = fixture("tokens.css");
    let scrapes = scrape_css_properties(&[path]).expect("scrape");

    let leading = scrapes
        .iter()
        .find(|s| s.name == "--leading-snug")
        .expect("--leading-snug");
    assert!(matches!(&leading.value, ScrapedValue::Other(s) if s == "1.25"));

    let font = scrapes
        .iter()
        .find(|s| s.name == "--font-body")
        .expect("--font-body");
    assert!(matches!(&font.value, ScrapedValue::Other(_)));
    assert_eq!(font.raw_value, "\"Inter\", sans-serif");
}

#[test]
fn rgb_color_values_normalize_to_hex() {
    let path = fixture("tokens.css");
    let scrapes = scrape_css_properties(&[path]).expect("scrape");

    let accent = scrapes
        .iter()
        .find(|s| s.name == "--accent-brand")
        .expect("--accent-brand");
    match &accent.value {
        ScrapedValue::Color(c) => assert_eq!(c, "#0b7285"),
        other => panic!("expected Color, got {other:?}"),
    }
}

#[test]
fn malformed_css_returns_css_parse_error_with_span() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("broken.css");
    // Unclosed :root block — no terminating brace.
    std::fs::write(&path, ":root {\n  --bg: #fff;\n").expect("write css");

    let err = scrape_css_properties(std::slice::from_ref(&path)).expect_err("malformed");
    match err {
        ConfigError::CssParse {
            path: errpath,
            span,
            ..
        } => {
            assert_eq!(errpath, path.display().to_string());
            assert!(span.is_some(), "expected a span for the offending region");
        }
        other => panic!("expected CssParse, got {other:?}"),
    }
}

#[test]
fn missing_file_returns_read_error() {
    let path = PathBuf::from("/definitely/not/a/path.css");
    let err = scrape_css_properties(&[path]).expect_err("missing");
    assert!(matches!(err, ConfigError::Read { .. }));
}

#[test]
fn ignores_non_root_selectors() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("non-root.css");
    std::fs::write(
        &path,
        ".card {\n  --hidden: 4px;\n}\n:root {\n  --visible: 8px;\n}\n",
    )
    .expect("write css");

    let scrapes = scrape_css_properties(&[path]).expect("scrape");

    assert!(
        scrapes.iter().any(|s| s.name == "--visible"),
        "expected --visible from :root"
    );
    assert!(
        !scrapes.iter().any(|s| s.name == "--hidden"),
        ":root-only scrape leaked --hidden from .card"
    );
}

#[test]
fn ignores_non_custom_properties() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("regular.css");
    std::fs::write(
        &path,
        ":root {\n  color: red;\n  --token: #fff;\n  font-size: 16px;\n}\n",
    )
    .expect("write css");

    let scrapes = scrape_css_properties(&[path]).expect("scrape");

    assert_eq!(scrapes.len(), 1);
    assert_eq!(scrapes[0].name, "--token");
}
