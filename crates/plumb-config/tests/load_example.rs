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
fn loads_prd_color_radius_alignment_a11y_sections() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let path = dir.path().join("plumb.toml");
    std::fs::write(
        &path,
        r##"
[color]
tokens = { "bg/canvas" = "#ffffff", "fg/primary" = "#0b0b0b", "accent/brand" = "#0b7285" }
delta_e_tolerance = 1.5

[radius]
scale = [0, 2, 4, 8, 12, 16, 9999]

[alignment]
grid_columns = 12
gutter_px = 24
tolerance_px = 3

[a11y]
min_contrast_ratio = 4.5

[a11y.touch_target]
min_width_px = 24
min_height_px = 24
"##,
    )
    .expect("write config");

    let cfg = plumb_config::load(&path).expect("load config");

    assert_eq!(cfg.color.tokens["bg/canvas"], "#ffffff");
    assert_eq!(cfg.color.tokens["fg/primary"], "#0b0b0b");
    assert_eq!(cfg.color.tokens["accent/brand"], "#0b7285");
    assert!((cfg.color.delta_e_tolerance - 1.5).abs() < f32::EPSILON);

    assert_eq!(cfg.radius.scale, vec![0, 2, 4, 8, 12, 16, 9999]);

    assert_eq!(cfg.alignment.grid_columns, Some(12));
    assert_eq!(cfg.alignment.gutter_px, Some(24));
    assert_eq!(cfg.alignment.tolerance_px, 3);

    assert_eq!(cfg.a11y.min_contrast_ratio, Some(4.5));
    assert_eq!(cfg.a11y.touch_target.min_width_px, 24);
    assert_eq!(cfg.a11y.touch_target.min_height_px, 24);
}

#[test]
fn defaults_for_color_radius_alignment_a11y_sections() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let path = dir.path().join("plumb.toml");
    std::fs::write(&path, "").expect("write config");

    let cfg = plumb_config::load(&path).expect("load config");

    assert!(cfg.color.tokens.is_empty());
    assert!((cfg.color.delta_e_tolerance - 2.0).abs() < f32::EPSILON);

    assert!(cfg.radius.scale.is_empty());

    assert_eq!(cfg.alignment.grid_columns, None);
    assert_eq!(cfg.alignment.gutter_px, None);
    assert_eq!(cfg.alignment.tolerance_px, 3);

    assert_eq!(cfg.a11y.min_contrast_ratio, None);
    assert_eq!(cfg.a11y.touch_target.min_width_px, 24);
    assert_eq!(cfg.a11y.touch_target.min_height_px, 24);
}

#[test]
fn rejects_old_config_aliases_and_unknown_fields() {
    for toml in [
        "[spacing]\nbase_px = 4\n",
        "[type_scale]\nsizes_px = [16]\n",
        "[type]\nsizes_px = [16]\n",
        "[type]\nline_heights = [1.5]\n",
        "[spacing]\nunknown = 4\n",
        "[radius]\nallowed_px = [4]\n",
        "[alignment]\nunknown = 1\n",
        "[a11y]\nunknown = 1\n",
        "[a11y.touch_target]\nunknown = 1\n",
        "[color]\nunknown = 1\n",
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

    let definitions = schema_json["$defs"]
        .as_object()
        .expect("schema $defs should be an object");
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

    let radius_props = definitions["RadiusSpec"]["properties"]
        .as_object()
        .expect("radius properties should be an object");
    assert!(radius_props.contains_key("scale"));
    assert!(!radius_props.contains_key("allowed_px"));

    let alignment_props = definitions["AlignmentSpec"]["properties"]
        .as_object()
        .expect("alignment properties should be an object");
    assert!(alignment_props.contains_key("grid_columns"));
    assert!(alignment_props.contains_key("gutter_px"));
    assert!(alignment_props.contains_key("tolerance_px"));

    let a11y_props = definitions["A11ySpec"]["properties"]
        .as_object()
        .expect("a11y properties should be an object");
    assert!(a11y_props.contains_key("min_contrast_ratio"));
    assert!(a11y_props.contains_key("touch_target"));

    let touch_target_props = definitions["TouchTargetSpec"]["properties"]
        .as_object()
        .expect("touch_target properties should be an object");
    assert!(touch_target_props.contains_key("min_width_px"));
    assert!(touch_target_props.contains_key("min_height_px"));

    assert!(!schema.contains("\"base_px\""));
    assert!(!schema.contains("\"sizes_px\""));
    assert!(!schema.contains("\"line_heights\""));
    assert!(!schema.contains("\"allowed_px\""));
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
