//! End-to-end CLI integration tests via `assert_cmd`.

use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn lint_fake_url_emits_one_violation() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello"])
        .assert()
        .code(3) // warning-only -> exit 3
        .stdout(contains("placeholder/hello-world"));
    Ok(())
}

#[test]
fn lint_fake_url_json_format() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello", "--format", "json"])
        .assert()
        .code(3)
        .stdout(contains("\"rule_id\""));
    Ok(())
}

#[test]
fn lint_real_url_with_missing_executable_path_reports_chromium_hint()
-> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "https://plumb.aramhammoudeh.com",
            "--executable-path",
            "/definitely/not/a/chromium/binary",
        ])
        .assert()
        .code(2)
        .stderr(contains("Chromium executable not found"))
        .stderr(contains("--executable-path"));
    Ok(())
}

#[test]
fn lint_fake_url_ignores_executable_path_override() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "plumb-fake://hello",
            "--executable-path",
            "/definitely/not/a/chromium/binary",
        ])
        .assert()
        .code(3)
        .stdout(contains("placeholder/hello-world"));
    Ok(())
}

#[test]
fn schema_outputs_json_schema() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .arg("schema")
        .assert()
        .success()
        .stdout(contains("\"$schema\""))
        .stdout(contains("viewports"));
    Ok(())
}

#[test]
fn explain_placeholder_rule() -> Result<(), Box<dyn std::error::Error>> {
    // `plumb explain` resolves docs relative to CWD. Run it from the
    // workspace root so it can find docs/src/rules/...
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    Command::cargo_bin("plumb")?
        .args(["explain", "placeholder/hello-world"])
        .current_dir(workspace_root)
        .assert()
        .success();
    Ok(())
}

#[test]
fn help_runs() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("Deterministic"));
    Ok(())
}
