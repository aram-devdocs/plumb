//! End-to-end tests for `plumb init --from <path>`.
//!
//! Builds a tempdir fixture (Tailwind config + a CSS token sheet),
//! runs the CLI binary, and snapshots the rendered `plumb.toml`.
//! `insta` redacts the absolute fixture path so the snapshot is stable
//! across machines.

use std::fs;

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn init_from_infers_starter_config_from_real_project_tree() -> Result<(), Box<dyn std::error::Error>>
{
    let project = TempDir::new()?;

    // A representative Tailwind project: tailwind.config.js at the
    // root, a styles directory with a `:root` token sheet, and a
    // `node_modules` decoy that the walker MUST skip.
    fs::write(
        project.path().join("tailwind.config.js"),
        "module.exports = {\n  content: [],\n  theme: {\n    spacing: {\n      '0.5': '0.125rem',\n      '1.5': '0.375rem'\n    }\n  }\n};\n",
    )?;
    fs::create_dir_all(project.path().join("src/styles"))?;
    fs::write(
        project.path().join("src/styles/tokens.css"),
        ":root {\n  --color-bg: #ffffff;\n  --color-fg: #0b0b0b;\n  --color-accent: #0b7285;\n  --space-xs: 4px;\n  --space-sm: 8px;\n  --space-md: 16px;\n  --radius-sm: 4px;\n  --radius-md: 8px;\n}\n",
    )?;
    fs::create_dir_all(project.path().join("node_modules/decoy"))?;
    fs::write(
        project.path().join("node_modules/decoy/poison.css"),
        ":root { --color-poison: #ff0000; }",
    )?;

    let outdir = TempDir::new()?;
    Command::cargo_bin("plumb")?
        .arg("init")
        .arg("--from")
        .arg(project.path())
        .current_dir(outdir.path())
        .assert()
        .success()
        .stdout(contains("Wrote"))
        .stdout(contains("Inferred from"));

    let written = fs::read_to_string(outdir.path().join("plumb.toml"))?;
    let node_available = node_on_path();

    // Sanity checks on the written body. We assert structural facts
    // and rely on the snapshot below for the canonical form; the
    // `toml::to_string_pretty` representation wraps numeric arrays
    // across lines, hence the substring matches rather than exact
    // bracketed slices.
    assert!(written.contains("[color.tokens]"));
    assert!(written.contains("color-bg = \"#ffffff\""));
    assert!(written.contains("color-fg = \"#0b0b0b\""));
    assert!(written.contains("color-accent = \"#0b7285\""));
    assert!(written.contains("[spacing]"));
    assert!(written.contains("base_unit = 4"));
    if node_available {
        assert!(
            written.contains("\"0.5\" = 2"),
            "Tailwind spacing token 0.5 should be merged into init --from output"
        );
        assert!(
            written.contains("\"1.5\" = 6"),
            "Tailwind spacing token 1.5 should be merged into init --from output"
        );
    }
    assert!(written.contains("[radius]"));
    assert!(
        written.contains("Tailwind config detected"),
        "expected Tailwind hint in header"
    );
    assert!(
        !written.contains("color-poison"),
        "node_modules CSS leaked through the walker"
    );
    assert!(
        !written.contains("[viewports]\n"),
        "init --from must not emit an empty [viewports] table that disables lint targets"
    );
    assert!(
        !written.contains("[rules]\n"),
        "init --from must not emit an empty [rules] table"
    );

    Command::cargo_bin("plumb")?
        .args(["lint", "plumb-fake://hello"])
        .current_dir(outdir.path())
        .assert()
        .code(1)
        .stdout(contains("spacing/grid-conformance"));

    // Determinism: a second invocation against the same tree must
    // produce byte-identical output.
    let second_outdir = TempDir::new()?;
    Command::cargo_bin("plumb")?
        .arg("init")
        .arg("--from")
        .arg(project.path())
        .current_dir(second_outdir.path())
        .assert()
        .success();
    let second = fs::read_to_string(second_outdir.path().join("plumb.toml"))?;
    assert_eq!(written, second, "init --from output is non-deterministic");

    // Snapshot — we redact the absolute project path so the snapshot is
    // portable across machines. We rewrite the path to `<TMP>` here
    // rather than rely on insta's `filters` feature, which would
    // require enabling an extra feature on the dev-dep. macOS reports
    // `/var/folders/...` but `canonicalize` resolves to
    // `/private/var/folders/...`; redact both.
    let project_path = project.path().to_string_lossy().into_owned();
    let canonical = std::fs::canonicalize(project.path())?
        .to_string_lossy()
        .into_owned();
    let redacted = written
        .replace(&canonical, "<TMP>")
        .replace(&project_path, "<TMP>");
    if node_available {
        insta::assert_snapshot!("init_from_real_project", redacted);
    }

    Ok(())
}

fn node_on_path() -> bool {
    std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
}

#[test]
fn init_from_missing_directory_errors() -> Result<(), Box<dyn std::error::Error>> {
    let outdir = TempDir::new()?;
    Command::cargo_bin("plumb")?
        .arg("init")
        .arg("--from")
        .arg(outdir.path().join("definitely-does-not-exist"))
        .current_dir(outdir.path())
        .assert()
        .code(2)
        .stderr(contains("source directory not found"));
    Ok(())
}

#[test]
fn init_from_empty_dir_writes_blank_starter() -> Result<(), Box<dyn std::error::Error>> {
    let project = TempDir::new()?;
    let outdir = TempDir::new()?;
    Command::cargo_bin("plumb")?
        .arg("init")
        .arg("--from")
        .arg(project.path())
        .current_dir(outdir.path())
        .assert()
        .success()
        .stdout(contains("No design-token sources discovered"));
    let written = fs::read_to_string(outdir.path().join("plumb.toml"))?;
    assert!(written.contains("No design-token sources were discovered"));
    Ok(())
}

#[test]
fn init_from_app_subdir_infers_workspace_package_token_modules()
-> Result<(), Box<dyn std::error::Error>> {
    let workspace = TempDir::new()?;
    fs::write(
        workspace.path().join("package.json"),
        r#"{ "private": true, "workspaces": ["apps/*", "packages/*"] }"#,
    )?;
    fs::create_dir_all(workspace.path().join("apps/web"))?;
    let tokens = workspace.path().join("packages/types/src/tokens");
    fs::create_dir_all(&tokens)?;
    fs::write(
        tokens.join("spacing.ts"),
        r"
            export const SPACING = {
              0.5: '2px',
              1: '4px',
              1.5: '6px',
            } as const;

            export const RADIUS = {
              sm: '4px',
              md: '6px',
              '2xl': '16px',
            } as const;
        ",
    )?;
    fs::write(
        tokens.join("colors.ts"),
        r"
            export const COLOR_TOKENS = {
              navy: '#0A3D5C',
            } as const;

            export const STATUS_COLORS = {
              success: '#22c55e',
            } as const;

            export const DESIGN_TOKENS = {
              colors: COLOR_TOKENS,
            } as const;
        ",
    )?;
    fs::write(
        tokens.join("typography.ts"),
        r#"
            export const FONT_FAMILY = {
              heading: '"Poppins", sans-serif',
              body: '"apertura", "Inter", system-ui, sans-serif',
            } as const;

            export const FONT_SIZE = {
              '2xs': '9px',
              xs: '10px',
            } as const;

            export const FONT_WEIGHT = {
              normal: 400,
              semibold: 600,
              bold: 700,
              extrabold: 800,
            } as const;
        "#,
    )?;

    let outdir = TempDir::new()?;
    Command::cargo_bin("plumb")?
        .arg("init")
        .arg("--from")
        .arg(workspace.path().join("apps/web"))
        .current_dir(outdir.path())
        .assert()
        .success()
        .stdout(contains("Inferred from"));

    let written = fs::read_to_string(outdir.path().join("plumb.toml"))?;
    assert!(written.contains("\"0.5\" = 2"));
    assert!(written.contains("\"1.5\" = 6"));
    assert!(written.contains("navy = \"#0A3D5C\""));
    assert!(written.contains("success = \"#22c55e\""));
    assert!(written.contains("weights = [\n    400,\n    600,\n    700,\n    800,\n]"));
    assert!(written.contains("2,\n    4,\n    6,"));
    assert!(written.contains("../../packages/types/src/tokens/spacing.ts"));
    let workspace_path = workspace.path().to_string_lossy();
    assert!(!written.contains(workspace_path.as_ref()));

    Ok(())
}
