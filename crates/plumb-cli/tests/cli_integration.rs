//! End-to-end CLI integration tests via `assert_cmd`.

use std::fs;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use tempfile::TempDir;

fn workspace_with_two_viewports() -> Result<TempDir, Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    fs::write(
        dir.path().join("plumb.toml"),
        // `body { padding-top: 13px }` in the canned snapshot is
        // off-grid against the default `spacing.base_unit = 4`, so the
        // orchestrator produces one `spacing/grid-conformance`
        // violation per requested viewport.
        "[viewports.mobile]\nwidth = 375\nheight = 812\n\n\
         [viewports.desktop]\nwidth = 1280\nheight = 800\n",
    )?;
    Ok(dir)
}

fn workspace_with_three_viewports() -> Result<TempDir, Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    fs::write(
        dir.path().join("plumb.toml"),
        "[viewports.mobile]\nwidth = 375\nheight = 812\n\n\
         [viewports.desktop]\nwidth = 1280\nheight = 800\n\n\
         [viewports.tablet]\nwidth = 768\nheight = 1024\n",
    )?;
    Ok(dir)
}

#[test]
fn lint_fake_url_emits_one_violation() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello"])
        .assert()
        .code(3) // warning-only -> exit 3
        .stdout(contains("spacing/grid-conformance"));
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
        .stdout(contains("spacing/grid-conformance"));
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
fn explain_spacing_grid_rule() -> Result<(), Box<dyn std::error::Error>> {
    // `plumb explain` resolves docs relative to CWD. Run it from the
    // workspace root so it can find docs/src/rules/...
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    Command::cargo_bin("plumb")?
        .args(["explain", "spacing/grid-conformance"])
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

#[test]
fn lint_with_unknown_viewport_exits_input_error() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = workspace_with_two_viewports()?;
    Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello", "--viewport", "bogus"])
        .current_dir(workspace.path())
        .assert()
        .code(2)
        .stderr(contains("bogus"))
        .stderr(contains("mobile"))
        .stderr(contains("desktop"));
    Ok(())
}

#[test]
fn lint_with_viewport_flag_and_no_config_exits_input_error()
-> Result<(), Box<dyn std::error::Error>> {
    // Fresh TempDir with no `plumb.toml`. Passing `--viewport mobile`
    // here used to silently fall back to the default 1280x800 desktop
    // viewport (issue #119); the flag is now refused so the user sees
    // the mismatch instead of running with the wrong viewport.
    let dir = TempDir::new()?;
    Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello", "--viewport", "mobile"])
        .current_dir(dir.path())
        .assert()
        .code(2)
        .stderr(contains("mobile"))
        .stderr(contains("no [viewports]"));
    Ok(())
}

#[test]
fn lint_runs_every_configured_viewport_when_flag_absent() -> Result<(), Box<dyn std::error::Error>>
{
    let workspace = workspace_with_two_viewports()?;
    Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello"])
        .current_dir(workspace.path())
        .assert()
        .code(3)
        .stdout(contains("spacing/grid-conformance"))
        .stdout(contains("mobile"))
        .stdout(contains("desktop"));
    Ok(())
}

#[test]
fn lint_filters_to_named_viewport() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = workspace_with_two_viewports()?;
    Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello", "--viewport", "mobile"])
        .current_dir(workspace.path())
        .assert()
        .code(3)
        .stdout(contains("mobile"))
        .stdout(contains("desktop").not());
    Ok(())
}

#[test]
fn lint_repeats_viewport_flag() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = workspace_with_three_viewports()?;
    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "plumb-fake://hello",
            "--viewport",
            "mobile",
            "--viewport",
            "desktop",
        ])
        .current_dir(workspace.path())
        .assert()
        .code(3)
        .stdout(contains("mobile"))
        .stdout(contains("desktop"))
        .stdout(contains("tablet").not());
    Ok(())
}
