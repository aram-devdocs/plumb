//! Span-annotated validation diagnostics.
//!
//! Each test exercises one of the four documented failure classes and
//! asserts that the returned [`plumb_config::ConfigError`] surfaces a
//! miette `Diagnostic` with a non-empty source span pointing at the
//! offending line.

// Helpers deliberately use `expect`/`expect_err` to surface failures
// loudly. The crate's `clippy.toml` allows this in tests, but
// integration-test helpers don't carry the `#[test]` proximity that
// clippy looks for, so we opt them in explicitly.
#![allow(clippy::expect_used)]

use std::path::Path;

use miette::Diagnostic;
use plumb_config::ConfigError;

/// Read the byte slice covered by a diagnostic label and assert it
/// non-empty. The label may live on either the [`ConfigError::Parse`]
/// or [`ConfigError::Validation`] variant.
fn assert_span_points_at(err: &ConfigError, expected_substring: &str) {
    let source = err
        .source_code()
        .expect("config diagnostic should expose source code");

    let mut labels = err
        .labels()
        .expect("config diagnostic should expose labels")
        .collect::<Vec<_>>();
    let label = labels
        .pop()
        .expect("config diagnostic should expose at least one label");

    let span = label.inner();
    let contents = source
        .read_span(span, 0, 0)
        .expect("source should be readable for span");
    let bytes = contents.data();
    let text = std::str::from_utf8(bytes).expect("source bytes are utf-8");

    assert!(
        !text.trim().is_empty(),
        "span should cover non-empty source text, got {text:?}"
    );
    assert!(
        text.contains(expected_substring),
        "span text {text:?} should contain {expected_substring:?}",
    );
}

fn write_config(name: &str, body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("create tempdir");
    let path = dir.path().join(name);
    std::fs::write(&path, body).expect("write config");
    (dir, path)
}

fn load(path: &Path) -> ConfigError {
    plumb_config::load(path).expect_err("config should fail to load")
}

#[test]
fn unknown_top_level_key_surfaces_span() {
    // The unknown key must precede any `[table]` header so TOML grammar
    // parses it as a top-level key rather than a member of the active
    // table.
    let (_dir, path) = write_config(
        "plumb.toml",
        "\
bogus_top_level = true

[viewports.mobile]
width = 320
height = 568
",
    );

    let err = load(&path);
    assert!(
        matches!(err, ConfigError::Parse { .. }),
        "expected Parse, got {err:?}"
    );
    assert_span_points_at(&err, "bogus_top_level");
}

#[test]
fn unknown_nested_key_surfaces_span() {
    let (_dir, path) = write_config(
        "plumb.toml",
        "\
[spacing]
base_unit = 8
made_up_field = 12
",
    );

    let err = load(&path);
    assert!(
        matches!(err, ConfigError::Parse { .. }),
        "expected Parse, got {err:?}"
    );
    assert_span_points_at(&err, "made_up_field");
}

#[test]
fn wrong_type_surfaces_span() {
    let (_dir, path) = write_config(
        "plumb.toml",
        "\
[spacing]
base_unit = \"eight\"
",
    );

    let err = load(&path);
    assert!(
        matches!(err, ConfigError::Parse { .. }),
        "expected Parse, got {err:?}"
    );
    assert_span_points_at(&err, "eight");
}

#[test]
fn bad_palette_value_surfaces_span() {
    let (_dir, path) = write_config(
        "plumb.toml",
        "\
[color.tokens]
brand = \"#0b7285\"
bg = \"not-a-hex\"
",
    );

    let err = load(&path);
    assert!(
        matches!(err, ConfigError::Validation { .. }),
        "expected Validation, got {err:?}"
    );
    assert_span_points_at(&err, "not-a-hex");
}

#[test]
fn bad_palette_value_in_yaml_surfaces_diagnostic() {
    let (_dir, path) = write_config(
        "plumb.yaml",
        "\
color:
  tokens:
    brand: \"#0b7285\"
    bg: not-a-hex
",
    );

    let err = load(&path);
    assert!(
        matches!(err, ConfigError::Validation { .. }),
        "expected Validation, got {err:?}"
    );
    // YAML span recovery is best-effort: assert the diagnostic exposes
    // a source code section so editors can highlight the file.
    assert!(
        err.source_code().is_some(),
        "yaml validation error should expose source code"
    );
}

#[test]
fn unknown_top_level_key_in_yaml_surfaces_diagnostic() {
    let (_dir, path) = write_config(
        "plumb.yaml",
        "\
viewports:
  mobile:
    width: 320
    height: 568
bogus_top_level: true
",
    );

    let err = load(&path);
    assert!(
        matches!(err, ConfigError::Parse { .. }),
        "expected Parse, got {err:?}"
    );
    assert!(
        err.source_code().is_some(),
        "yaml parse error should expose source code"
    );
}
