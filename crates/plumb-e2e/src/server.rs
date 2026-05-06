//! Loopback HTTP server that serves a fixture's `dist/` directory.
//!
//! The harness binds `127.0.0.1:0`, lets the OS allocate a free port,
//! and serves until the [`StaticServer`] is dropped. `Drop` joins the
//! background thread by signaling shutdown via an atomic flag and
//! poking the listener with a one-shot connection.
//!
//! Only the static-asset shape Plumb needs is supported: GET requests
//! resolve to `dist/<path>` (no query string handling, no range
//! requests). Directory paths return `dist/<path>/index.html` when the
//! file exists, otherwise 404. Symlinks are not followed to keep the
//! server safe to run against arbitrary `dist/` layouts.

use std::io::{self, Write as _};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tiny_http::{Header, Response, Server};

/// Per-extension content-type lookup. Anything not listed falls back to
/// `application/octet-stream`. The list is intentionally small —
/// fixtures only ship the asset shapes the bundlers below produce.
const CONTENT_TYPES: &[(&str, &str)] = &[
    (".html", "text/html; charset=utf-8"),
    (".htm", "text/html; charset=utf-8"),
    (".css", "text/css; charset=utf-8"),
    (".js", "application/javascript; charset=utf-8"),
    (".mjs", "application/javascript; charset=utf-8"),
    (".json", "application/json; charset=utf-8"),
    (".svg", "image/svg+xml"),
    (".png", "image/png"),
    (".jpg", "image/jpeg"),
    (".jpeg", "image/jpeg"),
    (".gif", "image/gif"),
    (".ico", "image/x-icon"),
    (".woff", "font/woff"),
    (".woff2", "font/woff2"),
    (".ttf", "font/ttf"),
    (".map", "application/json; charset=utf-8"),
    (".txt", "text/plain; charset=utf-8"),
    (".webp", "image/webp"),
    (".wasm", "application/wasm"),
];

/// A loopback HTTP server. Drop the value to stop the server.
pub struct StaticServer {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl StaticServer {
    /// Bind `127.0.0.1:0`, spawn the server thread, and return the
    /// handle.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] if `root` is not a
    /// directory or binding fails (port exhaustion, sandbox denying
    /// loopback, etc.).
    pub fn bind(root: PathBuf) -> Result<Self, io::Error> {
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("dist directory `{}` not found", root.display()),
            ));
        }
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let addr = listener.local_addr()?;
        let server = Server::from_listener(listener, None)
            .map_err(|e| io::Error::other(format!("tiny_http: {e}")))?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = Arc::clone(&shutdown);
        let join = thread::Builder::new()
            .name(format!("plumb-e2e-server-{}", addr.port()))
            .spawn(move || serve(&server, &root, &shutdown_clone))?;
        Ok(Self {
            addr,
            shutdown,
            join: Some(join),
        })
    }

    /// The bound address (always `127.0.0.1:<allocated port>`).
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The base URL clients should hit. Always `http://127.0.0.1:<port>/`.
    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://{}/", self.addr)
    }
}

impl Drop for StaticServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // Open a shutdown connection so the listener wakes up and the
        // worker thread observes the flag.
        if let Ok(mut stream) = TcpStream::connect_timeout(&self.addr, Duration::from_millis(200)) {
            let _ = stream.write_all(b"GET / HTTP/1.0\r\n\r\n");
        }
        if let Some(join) = self.join.take() {
            // Best-effort join — a panic in the worker shouldn't take
            // down the harness.
            let _ = join.join();
        }
    }
}

fn serve(server: &Server, root: &Path, shutdown: &AtomicBool) {
    // Browsers request many chunks in parallel — Next.js for example
    // pulls a dozen `_next/static/chunks/*.js` files concurrently.
    // tiny_http's single-threaded recv loop must hand each request to
    // a worker thread immediately or Chromium will time out waiting
    // for a response. The receiver itself is cheap (just dequeues a
    // pending TCP accept), so a 25ms timeout polling cadence keeps the
    // shutdown latency low while still letting the dispatcher fan out
    // requests as fast as Chromium fires them.
    let root = Arc::new(root.to_path_buf());
    while !shutdown.load(Ordering::SeqCst) {
        match server.recv_timeout(Duration::from_millis(25)) {
            Ok(Some(request)) => {
                let worker_root = Arc::clone(&root);
                let _ = thread::Builder::new()
                    .name(String::from("plumb-e2e-server-worker"))
                    .spawn(move || handle(request, &worker_root));
            }
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(error = %err, "tiny_http recv error");
                break;
            }
        }
    }
}

fn handle(request: tiny_http::Request, root: &Path) {
    let url = request.url().to_string();
    let Some(path) = resolve_path(root, &url) else {
        let _ = request.respond(Response::from_string("not found").with_status_code(404));
        return;
    };
    match std::fs::read(&path) {
        Ok(bytes) => {
            let mime = content_type_for(&path);
            let Ok(header) = Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()) else {
                let _ = request.respond(Response::from_string("bad header").with_status_code(500));
                return;
            };
            let response = Response::from_data(bytes).with_header(header);
            let _ = request.respond(response);
        }
        Err(err) => {
            tracing::debug!(error = %err, path = %path.display(), "static read failed");
            let _ = request.respond(Response::from_string("not found").with_status_code(404));
        }
    }
}

/// Resolve `url` (path-and-query as received by tiny_http) to a file on
/// disk under `root`. Returns `None` if the path escapes `root`,
/// references a non-existent file, or names a symlink.
fn resolve_path(root: &Path, url: &str) -> Option<PathBuf> {
    // Strip any query string.
    let path_only = url.split('?').next().unwrap_or(url);
    let trimmed = path_only.trim_start_matches('/');
    // Reject any `..` component up front so we never read above `root`.
    if trimmed
        .split(['/', '\\'])
        .any(|seg| seg == ".." || seg.starts_with("..\\") || seg.starts_with("../"))
    {
        return None;
    }
    let mut candidate = root.join(trimmed);
    if candidate.is_dir() {
        candidate = candidate.join("index.html");
    }
    if !candidate.is_file() {
        return None;
    }
    let meta = std::fs::symlink_metadata(&candidate).ok()?;
    if meta.file_type().is_symlink() {
        return None;
    }
    Some(candidate)
}

fn content_type_for(path: &Path) -> &'static str {
    let lower = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| format!(".{}", s.to_ascii_lowercase()))
        .unwrap_or_default();
    for (suffix, mime) in CONTENT_TYPES {
        if lower == *suffix {
            return mime;
        }
    }
    "application/octet-stream"
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Read as _, Write as _};
    use std::net::TcpStream;
    use std::time::Duration;

    use tempfile::TempDir;

    use super::StaticServer;

    fn http_get(addr: &str) -> std::io::Result<Vec<u8>> {
        let mut stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        write!(stream, "GET / HTTP/1.0\r\nHost: {addr}\r\n\r\n")?;
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf)?;
        Ok(buf)
    }

    #[test]
    fn serves_index_html_on_root() {
        let dir = TempDir::new().expect("tempdir");
        fs::write(dir.path().join("index.html"), b"<!doctype html><p>ok</p>")
            .expect("write index.html");
        let server = StaticServer::bind(dir.path().to_path_buf()).expect("bind");
        let body = http_get(&server.addr().to_string()).expect("get");
        let text = String::from_utf8_lossy(&body);
        assert!(
            text.contains("200 OK") || text.contains("HTTP/1.1 200"),
            "expected 200, got: {text}"
        );
        assert!(text.contains("<p>ok</p>"));
    }

    #[test]
    fn rejects_path_traversal() {
        let dir = TempDir::new().expect("tempdir");
        fs::write(dir.path().join("index.html"), b"ok").expect("write");
        let server = StaticServer::bind(dir.path().to_path_buf()).expect("bind");
        let mut stream = TcpStream::connect(server.addr()).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("read timeout");
        write!(stream, "GET /../secret.txt HTTP/1.0\r\n\r\n").expect("write");
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).expect("read");
        let text = String::from_utf8_lossy(&buf);
        assert!(text.contains("404"), "expected 404, got: {text}");
    }
}
