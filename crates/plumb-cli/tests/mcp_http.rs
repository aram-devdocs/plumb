//! End-to-end HTTP transport tests for `plumb mcp`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::missing_panics_doc)]

use std::io;
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::prelude::OutputAssertExt;
use predicates::str::contains;

const INIT_BODY: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"plumb-test","version":"0.0.0"}}}"#;

fn bin() -> std::path::PathBuf {
    cargo_bin("plumb")
}

fn reserve_port() -> io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

struct HttpServerChild {
    child: Child,
}

impl Drop for HttpServerChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

async fn spawn_http_server(
    token: &str,
) -> Result<(HttpServerChild, u16), Box<dyn std::error::Error>> {
    let port = reserve_port()?;
    let mut child = Command::new(bin())
        .args(["mcp", "--transport", "http", "--port", &port.to_string()])
        .env("PLUMB_MCP_TOKEN", token)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    for _ in 0..50 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Ok((HttpServerChild { child }, port));
        }

        if let Some(status) = child.try_wait()? {
            return Err(
                format!("http server exited before accepting connections: {status}").into(),
            );
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    Err("timed out waiting for http server to accept connections".into())
}

async fn initialize_request(
    port: u16,
    authorization: Option<&str>,
) -> Result<reqwest::Response, reqwest::Error> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()?;

    let mut request = client
        .post(format!("http://127.0.0.1:{port}/"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2025-03-26")
        .body(INIT_BODY);

    if let Some(authorization) = authorization {
        request = request.header("Authorization", authorization);
    }

    request.send().await
}

#[test]
fn http_transport_refuses_to_boot_without_token() {
    Command::new(bin())
        .args(["mcp", "--transport", "http", "--port", "4242"])
        .env_remove("PLUMB_MCP_TOKEN")
        .assert()
        .code(2)
        .stderr(contains("PLUMB_MCP_TOKEN"))
        .stderr(contains("--transport http"));
}

#[test]
fn http_transport_refuses_to_boot_with_empty_token() {
    Command::new(bin())
        .args(["mcp", "--transport", "http", "--port", "4242"])
        .env("PLUMB_MCP_TOKEN", "")
        .assert()
        .code(2)
        .stderr(contains("PLUMB_MCP_TOKEN"))
        .stderr(contains("non-empty bearer token"));
}

#[tokio::test]
async fn http_transport_rejects_requests_without_bearer_token()
-> Result<(), Box<dyn std::error::Error>> {
    let (_server, port) = spawn_http_server("secret-token").await?;

    let response = initialize_request(port, None).await?;

    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn http_transport_rejects_invalid_bearer_token() -> Result<(), Box<dyn std::error::Error>> {
    let (_server, port) = spawn_http_server("secret-token").await?;

    let response = initialize_request(port, Some("Bearer wrong-token")).await?;

    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn http_transport_accepts_valid_bearer_token() -> Result<(), Box<dyn std::error::Error>> {
    let (_server, port) = spawn_http_server("secret-token").await?;

    let response = initialize_request(port, Some("Bearer secret-token")).await?;

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert!(response.headers().contains_key("mcp-session-id"));
    Ok(())
}
