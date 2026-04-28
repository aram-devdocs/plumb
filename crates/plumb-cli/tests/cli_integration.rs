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

#[test]
fn init_writes_generic_config_in_clean_dir() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    Command::cargo_bin("plumb")?
        .arg("init")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(contains("Wrote"))
        .stdout(contains("Tailwind").not());
    let written = fs::read_to_string(dir.path().join("plumb.toml"))?;
    assert!(written.contains("[viewports.desktop]"));
    assert!(!written.contains("Tailwind detected"));
    assert!(!written.contains("{{TAILWIND_CONFIG}}"));
    Ok(())
}

#[test]
fn init_refuses_to_overwrite_without_force() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    fs::write(dir.path().join("plumb.toml"), "# existing\n")?;
    Command::cargo_bin("plumb")?
        .arg("init")
        .current_dir(dir.path())
        .assert()
        .code(2)
        .stderr(contains("already exists"));
    let preserved = fs::read_to_string(dir.path().join("plumb.toml"))?;
    assert_eq!(preserved, "# existing\n");
    Ok(())
}

#[test]
fn init_overwrites_with_force() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    fs::write(dir.path().join("plumb.toml"), "# sentinel-do-not-keep\n")?;
    Command::cargo_bin("plumb")?
        .args(["init", "--force"])
        .current_dir(dir.path())
        .assert()
        .success();
    let written = fs::read_to_string(dir.path().join("plumb.toml"))?;
    assert!(!written.contains("sentinel-do-not-keep"));
    assert!(written.contains("[viewports.desktop]"));
    Ok(())
}

#[test]
fn init_detects_tailwind_and_emits_tailwind_template() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    fs::write(
        dir.path().join("tailwind.config.ts"),
        "export default { content: [] };\n",
    )?;
    fs::write(
        dir.path().join("package.json"),
        r#"{
            "name": "tailwind-fixture",
            "private": true,
            "devDependencies": {
                "next": "14.2.0",
                "tailwindcss": "3.4.0"
            }
        }
        "#,
    )?;
    Command::cargo_bin("plumb")?
        .arg("init")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(contains("Tailwind config detected"));
    let written = fs::read_to_string(dir.path().join("plumb.toml"))?;
    assert!(written.contains("./tailwind.config.ts"));
    assert!(written.contains("Tailwind config detected"));
    assert!(!written.contains("{{TAILWIND_CONFIG}}"));
    Ok(())
}

#[test]
fn init_then_lint_runs_against_fake_driver() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    fs::write(
        dir.path().join("tailwind.config.js"),
        "module.exports = { content: [] };\n",
    )?;
    fs::write(
        dir.path().join("package.json"),
        r#"{
            "name": "tailwind-lint-fixture",
            "private": true,
            "dependencies": { "tailwindcss": "3.4.0" }
        }
        "#,
    )?;
    Command::cargo_bin("plumb")?
        .arg("init")
        .current_dir(dir.path())
        .assert()
        .success();
    Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello"])
        .current_dir(dir.path())
        .assert()
        .code(3)
        .stdout(contains("spacing/"));
    Ok(())
}

// `--selector` (PRD §15.4) — restricts the lint to a CSS subtree
// before rule dispatch. The canned `plumb-fake://hello` snapshot has
// `padding-top: 13px` on `<body>`, off-grid against the default
// `spacing.base_unit = 4`, so `spacing/grid-conformance` fires when
// body is in the kept set and stays silent when it isn't.

#[test]
fn lint_with_selector_matching_body_keeps_violation() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello", "--selector", "body"])
        .assert()
        .code(3)
        .stdout(contains("spacing/grid-conformance"));
    Ok(())
}

#[test]
fn lint_with_selector_matching_only_head_drops_violation() -> Result<(), Box<dyn std::error::Error>>
{
    Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello", "--selector", "head"])
        .assert()
        .success()
        .stdout(contains("spacing/grid-conformance").not());
    Ok(())
}

#[test]
fn lint_with_invalid_selector_exits_input_error() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello", "--selector", ">>>"])
        .assert()
        .code(2)
        .stderr(contains("invalid --selector"))
        .stderr(contains(">>>"));
    Ok(())
}

#[test]
fn lint_with_selector_matching_nothing_exits_input_error() -> Result<(), Box<dyn std::error::Error>>
{
    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "plumb-fake://hello",
            "--selector",
            ".does-not-exist",
        ])
        .assert()
        .code(2)
        .stderr(contains("matched no elements"));
    Ok(())
}

/// End-to-end regression for #121: when `plumb lint plumb-fake://hello
/// --format json --viewport mobile` runs against a multi-viewport
/// config, the JSON `rect` field MUST carry the mobile dimensions, not
/// the canned desktop ones. Before #125 this would emit `1280x800`
/// even for the mobile target.
#[test]
fn lint_fake_url_json_rect_matches_requested_viewport() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = workspace_with_two_viewports()?;
    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "plumb-fake://hello",
            "--viewport",
            "mobile",
            "--format",
            "json",
        ])
        .current_dir(workspace.path())
        .assert()
        .code(3)
        .stdout(contains("\"viewport\": \"mobile\""))
        .stdout(contains("\"width\": 375"))
        .stdout(contains("\"height\": 812"))
        .stdout(contains("\"width\": 1280").not())
        .stdout(contains("\"height\": 800").not());
    Ok(())
}
