//! `plumb schema` — write the JSON Schema for `plumb.toml` to stdout.

use std::process::ExitCode;

use anyhow::{Context, Result};

pub fn run() -> Result<ExitCode> {
    let schema = plumb_config::emit_schema().context("emit schema")?;
    #[allow(clippy::print_stdout)]
    {
        println!("{schema}");
    }
    Ok(ExitCode::SUCCESS)
}
