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
fn lint_fake_url_json_output_writes_exact_payload_to_file() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = TempDir::new()?;
    let output_path = dir.path().join("violations.json");

    let expected = Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello", "--format", "json"])
        .output()?;
    assert_eq!(expected.status.code(), Some(3));
    assert!(expected.stderr.is_empty());

    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "plumb-fake://hello",
            "--format",
            "json",
            "--output",
            output_path.to_str().ok_or("non-utf8 output path")?,
        ])
        .assert()
        .code(3)
        .stdout("")
        .stderr("");

    let written = fs::read(&output_path)?;
    assert_eq!(written, expected.stdout);
    Ok(())
}

#[test]
fn lint_fake_url_sarif_output_writes_exact_payload_to_file()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let output_path = dir.path().join("results.sarif");

    let expected = Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello", "--format", "sarif"])
        .output()?;
    assert_eq!(expected.status.code(), Some(3));
    assert!(expected.stderr.is_empty());

    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "plumb-fake://hello",
            "--format",
            "sarif",
            "--output",
            output_path.to_str().ok_or("non-utf8 output path")?,
        ])
        .assert()
        .code(3)
        .stdout("")
        .stderr("");

    let written = fs::read(&output_path)?;
    assert_eq!(written, expected.stdout);
    Ok(())
}

#[test]
fn lint_output_with_missing_parent_exits_infra_error_without_rendering_payload()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let output_path = dir.path().join("missing").join("violations.json");

    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "plumb-fake://hello",
            "--format",
            "json",
            "--output",
            output_path.to_str().ok_or("non-utf8 output path")?,
        ])
        .assert()
        .code(2)
        .stdout("")
        .stderr(contains("write lint output to"))
        .stderr(contains("\"rule_id\"").not())
        .stderr(contains("spacing/grid-conformance").not());
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

/// `--auto-fetch-chromium` is a CLI flag whose runtime effect (a
/// network download) we don't want to exercise inside the test suite.
/// The contract we _can_ check end-to-end is that the flag parses,
/// shows up in `--help`, and that fake-URL runs ignore it just like
/// `--executable-path`.
#[test]
fn auto_fetch_chromium_flag_is_documented_in_help() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args(["lint", "--help"])
        .assert()
        .success()
        .stdout(contains("--auto-fetch-chromium"));
    Ok(())
}

#[test]
fn lint_fake_url_ignores_auto_fetch_flag() -> Result<(), Box<dyn std::error::Error>> {
    // Auto-fetch must not fire on the FakeDriver path. If it did, this
    // test would either hang waiting for a download or fail with a
    // network error — the FakeDriver branch in `commands::lint::run`
    // skips driver options entirely, so passing the flag is a no-op
    // and the canned snapshot still produces its single warning.
    Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello", "--auto-fetch-chromium"])
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

// ============================================================
// Driver-ergonomics wave (#74 #75 #76 #77)
//
// FakeDriver short-circuits before any browser-side wiring runs, so
// these tests focus on:
//   - Argument parsing accepts every flag without error.
//   - Validation errors surface as exit code 2 with the expected message.
//   - Successful cases still produce the canned snapshot's violation.
// Real Chromium-driven coverage lives behind the `e2e-chromium`
// feature in `crates/plumb-cdp/tests/`.

#[test]
fn lint_accepts_wait_for_and_wait_ms_against_fake_driver() -> Result<(), Box<dyn std::error::Error>>
{
    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "plumb-fake://hello",
            "--wait-for",
            "body",
            "--wait-ms",
            "10",
        ])
        .assert()
        .code(3)
        .stdout(contains("spacing/grid-conformance"));
    Ok(())
}

#[test]
fn lint_accepts_repeated_cookies_against_fake_driver() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "plumb-fake://hello",
            "--cookie",
            "session=abc123",
            "--cookie",
            "lang=en",
        ])
        .assert()
        .code(3)
        .stdout(contains("spacing/grid-conformance"));
    Ok(())
}

#[test]
fn lint_rejects_malformed_cookie_with_input_error() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello", "--cookie", "no-equals"])
        .assert()
        .code(2)
        .stderr(contains("invalid cookie"));
    Ok(())
}

#[test]
fn lint_rejects_cookie_with_crlf() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "plumb-fake://hello",
            "--cookie",
            "name=value\r\nSet-Cookie: pwn=1",
        ])
        .assert()
        .code(2)
        .stderr(contains("control characters"));
    Ok(())
}

#[test]
fn lint_accepts_repeated_headers_against_fake_driver() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "plumb-fake://hello",
            "--header",
            "X-Trace-Id: 12345",
            "--header",
            "Authorization: Bearer xyz",
        ])
        .assert()
        .code(3)
        .stdout(contains("spacing/grid-conformance"));
    Ok(())
}

#[test]
fn lint_rejects_header_with_lf_injection() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "plumb-fake://hello",
            "--header",
            "X-Pwn: hello\nInjected: 1",
        ])
        .assert()
        .code(2)
        .stderr(contains("control characters"));
    Ok(())
}

#[test]
fn lint_accepts_storage_state_path_against_fake_driver() -> Result<(), Box<dyn std::error::Error>> {
    let dir = TempDir::new()?;
    let path = dir.path().join("storage-state.json");
    fs::write(&path, r#"{"cookies":[],"origins":[]}"#)?;
    // Run from inside the tempdir so the safe-path canonicalize check
    // passes (the path resolves under CWD).
    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "plumb-fake://hello",
            "--storage-state",
            "storage-state.json",
        ])
        .current_dir(dir.path())
        .assert()
        .code(3)
        .stdout(contains("spacing/grid-conformance"));
    Ok(())
}

#[test]
fn lint_rejects_auth_script_outside_cwd() -> Result<(), Box<dyn std::error::Error>> {
    // Two tempdirs: one contains the benign script, the other is the
    // CWD we run `plumb` from. The script is absolute and outside the
    // CWD, so the safe-path check MUST refuse it with exit code 2 even
    // though the URL is `plumb-fake://hello` (the FakeDriver doesn't
    // need an auth script — the CLI validates the path up front).
    let script_dir = TempDir::new()?;
    let script_path = script_dir.path().join("auth.js");
    fs::write(&script_path, "// benign auth script\n")?;

    let cwd = TempDir::new()?;

    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "plumb-fake://hello",
            "--auth-script",
            script_path
                .to_str()
                .ok_or("auth.js path is not valid UTF-8")?,
        ])
        .current_dir(cwd.path())
        .assert()
        .code(2)
        .stderr(contains("outside the current working directory"));
    Ok(())
}

#[test]
fn lint_rejects_storage_state_outside_cwd() -> Result<(), Box<dyn std::error::Error>> {
    // Same shape as the auth-script test above: the storage-state file
    // is in a separate tempdir and therefore outside the CLI's CWD.
    let state_dir = TempDir::new()?;
    let state_path = state_dir.path().join("storage-state.json");
    fs::write(&state_path, r#"{"cookies":[],"origins":[]}"#)?;

    let cwd = TempDir::new()?;

    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "plumb-fake://hello",
            "--storage-state",
            state_path
                .to_str()
                .ok_or("storage-state.json path is not valid UTF-8")?,
        ])
        .current_dir(cwd.path())
        .assert()
        .code(2)
        .stderr(contains("outside the current working directory"));
    Ok(())
}

#[test]
fn lint_accepts_disable_animations_and_hide_scrollbars_and_dpr()
-> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "plumb-fake://hello",
            "--disable-animations",
            "true",
            "--hide-scrollbars",
            "false",
            "--dpr",
            "2.0",
        ])
        .assert()
        .code(3)
        .stdout(contains("spacing/grid-conformance"));
    Ok(())
}

// `--suggest-ignores` (#84) — after a normal lint run, append a
// suggested `.plumbignore` block listing one entry per
// (rule_id, selector) tuple that would suppress every active violation.
//
// The canned `plumb-fake://hello` snapshot fires
// `spacing/grid-conformance` on `body`, so both the pretty and JSON
// shapes have a single deterministic entry to assert against.

#[test]
fn lint_without_suggest_ignores_omits_section() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello"])
        .assert()
        .code(3)
        .stdout(contains("Suggested .plumbignore").not());
    Ok(())
}

// ============================================================
// `plumb watch` (#83) — `--once` flag runs a single lint cycle and
// exits without entering the filesystem watcher loop, which gives us
// a deterministic shape to assert against without racing the OS.

#[test]
fn watch_once_runs_a_single_cycle_and_emits_status_line() -> Result<(), Box<dyn std::error::Error>>
{
    Command::cargo_bin("plumb")?
        .args(["watch", "plumb-fake://hello", "--once"])
        .assert()
        .code(3)
        .stdout(contains("spacing/grid-conformance"))
        .stderr(contains("watching"))
        .stderr(contains("lint:"))
        .stderr(contains("violations"));
    Ok(())
}

#[test]
fn lint_pretty_with_suggest_ignores_appends_footer() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello", "--suggest-ignores"])
        .assert()
        .code(3)
        .stdout(contains("Suggested .plumbignore"))
        .stdout(contains("would suppress 1 violation"))
        .stdout(contains("# Format: <rule_id> <selector_path>"))
        .stdout(contains("spacing/grid-conformance html > body"));
    Ok(())
}

#[test]
fn watch_once_with_json_format_emits_lint_payload() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args(["watch", "plumb-fake://hello", "--once", "--format", "json"])
        .assert()
        .code(3)
        .stdout(contains("\"rule_id\""))
        .stderr(contains("watching"));
    Ok(())
}

#[test]
fn lint_json_with_suggest_ignores_adds_array() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args([
            "lint",
            "plumb-fake://hello",
            "--format",
            "json",
            "--suggest-ignores",
        ])
        .assert()
        .code(3)
        .stdout(contains("\"suggested_ignores\""))
        .stdout(contains("\"rule_id\": \"spacing/grid-conformance\""))
        .stdout(contains("\"selector\": \"html > body\""));
    Ok(())
}

#[test]
fn lint_json_without_suggest_ignores_omits_array() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello", "--format", "json"])
        .assert()
        .code(3)
        .stdout(contains("\"suggested_ignores\"").not());
    Ok(())
}

#[test]
fn watch_help_lists_the_subcommand() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("plumb")?
        .args(["watch", "--help"])
        .assert()
        .success()
        .stdout(contains("watch").or(contains("Watch")));
    Ok(())
}
