//! End-to-end CLI integration tests via `assert_cmd`.

use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn lint_fake_url_emits_one_violation() {
    Command::cargo_bin("plumb")
        .expect("plumb binary")
        .args(["lint", "plumb-fake://hello"])
        .assert()
        .code(3) // warning-only -> exit 3
        .stdout(contains("placeholder/hello-world"));
}

#[test]
fn lint_fake_url_json_format() {
    Command::cargo_bin("plumb")
        .expect("plumb binary")
        .args(["lint", "plumb-fake://hello", "--format", "json"])
        .assert()
        .code(3)
        .stdout(contains("\"rule_id\""));
}

#[test]
fn lint_real_url_is_not_implemented() {
    Command::cargo_bin("plumb")
        .expect("plumb binary")
        .args(["lint", "https://plumb.dev"])
        .assert()
        .code(2)
        .stderr(contains("walking skeleton"));
}

#[test]
fn schema_outputs_json_schema() {
    Command::cargo_bin("plumb")
        .expect("plumb binary")
        .arg("schema")
        .assert()
        .success()
        .stdout(contains("\"$schema\""))
        .stdout(contains("viewports"));
}

#[test]
fn explain_placeholder_rule() {
    // `plumb explain` resolves docs relative to CWD. Run it from the
    // workspace root so it can find docs/src/rules/...
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    Command::cargo_bin("plumb")
        .expect("plumb binary")
        .args(["explain", "placeholder/hello-world"])
        .current_dir(workspace_root)
        .assert()
        .success();
}

#[test]
fn help_runs() {
    Command::cargo_bin("plumb")
        .expect("plumb binary")
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("Deterministic"));
}
