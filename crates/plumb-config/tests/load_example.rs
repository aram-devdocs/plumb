//! Round-trip the repo-root `examples/plumb.toml` through `plumb-config`.

use std::path::PathBuf;

use miette::Diagnostic;
use serde_json::Value;

#[test]
fn loads_example_toml() {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "examples",
        "plumb.toml",
    ]
    .iter()
    .collect();
    let cfg = plumb_config::load(&path).expect("load example");
    assert!(
        !cfg.viewports.is_empty(),
        "example config should define viewports"
    );
}

#[test]
fn loads_prd_spacing_and_type_sections() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let path = dir.path().join("plumb.toml");
    std::fs::write(
        &path,
        r#"
[spacing]
base_unit = 8
scale = [0, 8, 16, 24]
tokens = { sm = 8, md = 16 }

[type]
families = ["Inter", "ui-sans-serif"]
weights = [400, 700]
scale = [12, 14, 16, 20]
tokens = { body = 16, heading = 20 }
"#,
    )
    .expect("write config");

    let cfg = plumb_config::load(&path).expect("load config");

    assert_eq!(cfg.spacing.base_unit, 8);
    assert_eq!(cfg.spacing.scale, vec![0, 8, 16, 24]);
    assert_eq!(cfg.spacing.tokens["sm"], 8);
    assert_eq!(cfg.type_scale.families, vec!["Inter", "ui-sans-serif"]);
    assert_eq!(cfg.type_scale.weights, vec![400, 700]);
    assert_eq!(cfg.type_scale.scale, vec![12, 14, 16, 20]);
    assert_eq!(cfg.type_scale.tokens["heading"], 20);
}

#[test]
fn default_spacing_base_unit_is_four() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let path = dir.path().join("plumb.toml");
    std::fs::write(&path, "").expect("write config");

    let cfg = plumb_config::load(&path).expect("load config");

    assert_eq!(cfg.spacing.base_unit, 4);
}

#[test]
fn rejects_old_config_aliases_and_unknown_fields() {
    for toml in [
        "[spacing]\nbase_px = 4\n",
        "[type_scale]\nsizes_px = [16]\n",
        "[type]\nsizes_px = [16]\n",
        "[type]\nline_heights = [1.5]\n",
        "[spacing]\nunknown = 4\n",
    ] {
        let dir = tempfile::tempdir().expect("create tempdir");
        let path = dir.path().join("plumb.toml");
        std::fs::write(&path, toml).expect("write config");

        let err = plumb_config::load(&path).expect_err("reject invalid config");
        assert!(
            matches!(err, plumb_config::ConfigError::Parse { .. }),
            "expected parse error for {toml:?}, got {err:?}"
        );
    }
}

#[test]
fn schema_uses_prd_names_and_drops_old_aliases() {
    let schema = plumb_config::emit_schema().expect("emit schema");
    let schema_json: Value = serde_json::from_str(&schema).expect("schema should be valid JSON");

    let properties = schema_json["properties"]
        .as_object()
        .expect("schema properties should be an object");
    assert!(properties.contains_key("spacing"));
    assert!(properties.contains_key("type"));
    assert!(!properties.contains_key("type_scale"));

    let definitions = schema_json["definitions"]
        .as_object()
        .expect("schema definitions should be an object");
    let spacing_props = definitions["SpacingSpec"]["properties"]
        .as_object()
        .expect("spacing properties should be an object");
    assert!(spacing_props.contains_key("base_unit"));
    assert!(spacing_props.contains_key("scale"));
    assert!(spacing_props.contains_key("tokens"));

    let type_props = definitions["TypeScaleSpec"]["properties"]
        .as_object()
        .expect("type properties should be an object");
    assert!(type_props.contains_key("families"));
    assert!(type_props.contains_key("weights"));
    assert!(type_props.contains_key("scale"));
    assert!(type_props.contains_key("tokens"));

    assert!(!schema.contains("\"base_px\""));
    assert!(!schema.contains("\"sizes_px\""));
    assert!(!schema.contains("\"line_heights\""));
}

#[test]
fn toml_schema_errors_expose_miette_source_and_labels() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let path = dir.path().join("plumb.toml");
    std::fs::write(&path, "[spacing]\nbase_px = 4\n").expect("write config");

    let err = plumb_config::load(&path).expect_err("reject old field");

    assert!(
        err.source_code().is_some(),
        "parse error should expose source code"
    );

    let mut labels = err.labels().expect("parse error should expose labels");
    assert!(labels.next().is_some(), "parse error should expose labels");
}

#[test]
fn emits_schema() {
    let schema = plumb_config::emit_schema().expect("emit schema");
    assert!(
        schema.contains("\"$schema\""),
        "schema should declare $schema"
    );
    assert!(
        schema.contains("viewports"),
        "schema should mention viewports"
    );
}

#[test]
fn rejects_unknown_extension() {
    let path = PathBuf::from("/definitely/does/not/exist.xml");
    let err = plumb_config::load(&path).unwrap_err();
    assert!(matches!(err, plumb_config::ConfigError::NotFound(_)));
}
