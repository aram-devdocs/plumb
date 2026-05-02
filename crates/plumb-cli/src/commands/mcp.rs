//! `plumb mcp` — serve MCP on stdio or HTTP.

use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::process::ExitCode;

use anyhow::{Result, anyhow};

pub async fn run(transport: crate::McpTransport, port: u16) -> Result<ExitCode> {
    let cwd = env::current_dir()?;
    match transport {
        crate::McpTransport::Stdio => {
            tracing::info!("starting mcp stdio server");
            plumb_mcp::run_stdio(cwd).await?;
        }
        crate::McpTransport::Http => {
            let token = read_http_token()?;
            let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
            tracing::info!(%addr, "starting mcp http server");
            plumb_mcp::run_http(cwd, addr, token).await?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn read_http_token() -> Result<String> {
    match env::var("PLUMB_MCP_TOKEN") {
        Ok(token) if token.trim().is_empty() => Err(anyhow!(
            "PLUMB_MCP_TOKEN must be set to a non-empty bearer token when --transport http is used"
        )),
        Ok(token) => Ok(token),
        Err(env::VarError::NotPresent) => Err(anyhow!(
            "PLUMB_MCP_TOKEN must be set to a non-empty bearer token when --transport http is used"
        )),
        Err(env::VarError::NotUnicode(_)) => Err(anyhow!(
            "PLUMB_MCP_TOKEN must be valid Unicode when --transport http is used"
        )),
    }
}
