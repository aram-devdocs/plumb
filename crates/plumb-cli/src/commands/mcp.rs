//! `plumb mcp` — serve MCP on stdio.

use std::process::ExitCode;

use anyhow::Result;

pub async fn run() -> Result<ExitCode> {
    tracing::info!("starting mcp stdio server");
    plumb_mcp::run_stdio().await?;
    Ok(ExitCode::SUCCESS)
}
