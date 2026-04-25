//! `plumb explain <rule>` — print the long-form documentation for a rule.
//!
//! Reads from `docs/src/rules/<slug>.md` relative to the binary or CWD.
//! The rule id `spacing/grid-conformance` maps to
//! `docs/src/rules/spacing-grid-conformance.md`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};

pub fn run(rule: &str) -> Result<ExitCode> {
    let slug = rule.replace('/', "-");
    let relative = PathBuf::from(format!("docs/src/rules/{slug}.md"));

    // Try CWD first — normal dev workflow. Then fall back to the
    // sibling-of-binary layout that cargo-dist installs produce.
    let candidates = [relative.clone(), binary_relative(&relative)?];
    let Some(candidate) = candidates.iter().find(|p| p.exists()) else {
        bail!("no documentation found for rule `{rule}` at any of: {candidates:?}");
    };
    let content = std::fs::read_to_string(candidate)
        .with_context(|| format!("read {}", candidate.display()))?;
    #[allow(clippy::print_stdout)]
    {
        print!("{content}");
    }
    Ok(ExitCode::SUCCESS)
}

fn binary_relative(relative: &Path) -> Result<PathBuf> {
    let exe = std::env::current_exe().context("current_exe")?;
    let install_dir = exe.parent().context("exe has no parent")?;
    Ok(install_dir.join(relative))
}
