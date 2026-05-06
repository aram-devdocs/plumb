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

    // A representative Tailwind project: tailwind.config.ts at the
    // root, a styles directory with a `:root` token sheet, and a
    // `node_modules` decoy that the walker MUST skip.
    fs::write(
        project.path().join("tailwind.config.ts"),
        "export default { content: [] };\n",
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
    assert!(written.contains("[radius]"));
    assert!(
        written.contains("Tailwind config detected"),
        "expected Tailwind hint in header"
    );
    assert!(
        !written.contains("color-poison"),
        "node_modules CSS leaked through the walker"
    );

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
    insta::assert_snapshot!("init_from_real_project", redacted);

    Ok(())
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
