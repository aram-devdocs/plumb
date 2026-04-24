//! `plumb init` — write a starter `plumb.toml`.

use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};

const STARTER_CONTENT: &str = include_str!("../../../../examples/plumb.toml");

pub fn run(force: bool) -> Result<ExitCode> {
    let target = Path::new("plumb.toml");
    if target.exists() && !force {
        bail!(
            "{} already exists; pass --force to overwrite.",
            target.display()
        );
    }
    std::fs::write(target, STARTER_CONTENT)
        .with_context(|| format!("write {}", target.display()))?;
    #[allow(clippy::print_stdout)]
    {
        println!("Wrote {}.", target.display());
    }
    Ok(ExitCode::SUCCESS)
}
