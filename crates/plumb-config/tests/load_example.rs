//! Round-trip the repo-root `examples/plumb.toml` through `plumb-config`.

use std::path::PathBuf;

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
