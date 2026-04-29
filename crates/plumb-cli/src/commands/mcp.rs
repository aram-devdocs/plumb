//! `plumb mcp` — serve MCP on stdio.

use std::env;
use std::process::ExitCode;

use anyhow::Result;

pub async fn run() -> Result<ExitCode> {
    tracing::info!("starting mcp stdio server");
    let cwd = env::current_dir()?;
    plumb_mcp::run_stdio(cwd).await?;
    Ok(ExitCode::SUCCESS)
}
