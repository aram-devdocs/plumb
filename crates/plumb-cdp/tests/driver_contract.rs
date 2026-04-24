//! Contract tests for [`BrowserDriver`].

use plumb_cdp::{
    BrowserDriver, CdpError, ChromiumDriver, ChromiumOptions, FakeDriver, Target, is_fake_url,
};
use plumb_core::ViewportKey;

fn target(url: &str) -> Target {
    Target {
        url: url.into(),
        viewport: ViewportKey::new("desktop"),
        width: 1280,
        height: 800,
        device_pixel_ratio: 1.0,
    }
}

#[cfg(feature = "e2e-chromium")]
fn fixture_url(name: &str) -> std::io::Result<String> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    // file:// URLs need an absolute POSIX-style path; on Unix `Display`
    // already produces that. The e2e suite is exercised on Linux/macOS
    // CI runners only.
    let canonical = path.canonicalize()?;
    Ok(format!("file://{}", canonical.display()))
}

#[tokio::test]
async fn fake_driver_returns_canned_snapshot() -> Result<(), CdpError> {
    let driver = FakeDriver;
    let snap = driver.snapshot(target("plumb-fake://hello")).await?;
    assert_eq!(snap.url, "plumb-fake://hello");
    assert_eq!(snap.viewport.as_str(), "desktop");
    assert!(!snap.nodes.is_empty());
    Ok(())
}

#[tokio::test]
async fn fake_driver_rejects_unknown_urls() {
    let driver = FakeDriver;
    let result = driver.snapshot(target("plumb-fake://unknown")).await;
    assert!(matches!(result, Err(CdpError::UnknownFakeUrl(_))));
}

#[tokio::test]
async fn real_driver_reports_missing_explicit_executable() {
    let driver = ChromiumDriver::new(ChromiumOptions {
        executable_path: Some(std::path::PathBuf::from(
            "/definitely/not/a/chromium/binary",
        )),
    });

    let result = driver.snapshot(target("https://example.com")).await;

    assert!(matches!(result, Err(CdpError::ChromiumNotFound { .. })));
    if let Err(CdpError::ChromiumNotFound { install_hint }) = result {
        assert!(install_hint.contains("--executable-path"));
    }
}

#[test]
fn fake_url_detector() {
    assert!(is_fake_url("plumb-fake://hello"));
    assert!(!is_fake_url("https://plumb.aramhammoudeh.com"));
    assert!(!is_fake_url(""));
}

#[cfg(feature = "e2e-chromium")]
type E2eResult = Result<(), Box<dyn std::error::Error>>;

#[cfg(feature = "e2e-chromium")]
#[tokio::test]
async fn chromium_driver_captures_static_fixture() -> E2eResult {
    let url = fixture_url("static_page.html")?;
    let driver = ChromiumDriver::default();
    let snap = driver.snapshot(target(&url)).await?;

    assert_eq!(snap.url, url);
    assert_eq!(snap.viewport.as_str(), "desktop");
    assert_eq!(snap.viewport_width, 1280);
    assert_eq!(snap.viewport_height, 800);
    assert!(!snap.nodes.is_empty(), "expected non-empty node tree");

    let body = snap
        .nodes
        .iter()
        .find(|n| n.tag == "body")
        .ok_or("fixture has a <body>")?;
    assert_eq!(
        body.computed_styles.get("padding-top").map(String::as_str),
        Some("13px"),
        "Chromium normalizes the padding shorthand into per-side values"
    );

    let div = snap
        .nodes
        .iter()
        .find(|n| n.tag == "div")
        .ok_or("fixture has at least one <div>")?;
    let bg = div
        .computed_styles
        .get("background-color")
        .ok_or("div has a computed background-color")?;
    assert!(
        !bg.is_empty(),
        "background-color should be a non-empty CSS value, got `{bg}`"
    );

    let html = snap
        .nodes
        .iter()
        .find(|n| n.tag == "html")
        .ok_or("fixture has an <html> root")?;
    let rect = html
        .rect
        .ok_or("root html node should report a bounding rect")?;
    assert_eq!(
        rect.width, snap.viewport_width,
        "root rect width matches viewport width"
    );

    Ok(())
}

#[cfg(feature = "e2e-chromium")]
#[tokio::test]
async fn chromium_driver_snapshot_is_byte_identical() -> E2eResult {
    let url = fixture_url("static_page.html")?;
    let driver = ChromiumDriver::default();

    let mut serialized = Vec::with_capacity(3);
    for _ in 0..3 {
        let snap = driver.snapshot(target(&url)).await?;
        serialized.push(serde_json::to_string(&snap)?);
    }

    assert_eq!(
        serialized[0], serialized[1],
        "snapshot run 1 and 2 must be byte-identical"
    );
    assert_eq!(
        serialized[1], serialized[2],
        "snapshot run 2 and 3 must be byte-identical"
    );
    Ok(())
}
