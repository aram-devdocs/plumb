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
        ..ChromiumOptions::default()
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

/// Environment variable that opts a host into silently skipping the
/// e2e-chromium tests when Chromium is missing or out-of-range.
///
/// Without this set, a missing or unsupported Chromium hard-fails the
/// test — the previous "silently return Ok(())" behavior masked broken
/// e2e coverage on hosts where Chromium had drifted out of the
/// supported range. Set `PLUMB_E2E_CHROMIUM_SKIP=1` (or any value) to
/// restore the skip-on-missing behavior for hosts that genuinely don't
/// have Chromium installed.
#[cfg(feature = "e2e-chromium")]
const SKIP_ENV_VAR: &str = "PLUMB_E2E_CHROMIUM_SKIP";

/// Initialize a tracing subscriber for the test process at most once.
///
/// `tracing::warn!` from a skip path silently drops without a
/// subscriber. Using `try_init` here means concurrent tests don't race
/// each other to install one.
#[cfg(feature = "e2e-chromium")]
fn init_tracing() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
            )
            .with_test_writer()
            .try_init();
    });
}

/// Returns `true` if the e2e test should skip on `err`. Skipping is
/// allowed only when the user has explicitly opted in via
/// [`SKIP_ENV_VAR`] — otherwise the underlying error propagates and the
/// test fails loudly.
#[cfg(feature = "e2e-chromium")]
fn host_missing_chromium(err: &CdpError) -> bool {
    let is_chromium_unavailable = matches!(
        err,
        CdpError::ChromiumNotFound { .. } | CdpError::UnsupportedChromium { .. }
    );
    if !is_chromium_unavailable {
        return false;
    }
    if std::env::var_os(SKIP_ENV_VAR).is_some() {
        init_tracing();
        tracing::warn!(
            error = %err,
            env = SKIP_ENV_VAR,
            "skipping e2e-chromium test: host Chromium unavailable and skip opt-in is set"
        );
        true
    } else {
        false
    }
}

#[cfg(feature = "e2e-chromium")]
fn isolated_driver() -> std::io::Result<(ChromiumDriver, tempfile::TempDir)> {
    let dir = tempfile::tempdir()?;
    let driver = ChromiumDriver::new(ChromiumOptions {
        executable_path: None,
        user_data_dir: Some(dir.path().to_path_buf()),
    });
    Ok((driver, dir))
}

#[cfg(feature = "e2e-chromium")]
#[tokio::test]
async fn chromium_driver_captures_static_fixture() -> E2eResult {
    let url = fixture_url("static_page.html")?;
    let (driver, _profile) = isolated_driver()?;
    let snap = match driver.snapshot(target(&url)).await {
        Ok(snap) => snap,
        // Skip ONLY when the user opted in via PLUMB_E2E_CHROMIUM_SKIP.
        // `host_missing_chromium` logs the underlying error via
        // `tracing::warn!` so the skip is auditable.
        Err(err) if host_missing_chromium(&err) => return Ok(()),
        Err(err) => return Err(Box::<dyn std::error::Error>::from(err)),
    };

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
    let (driver, _profile) = isolated_driver()?;

    let mut serialized = Vec::with_capacity(3);
    for _ in 0..3 {
        let snap = match driver.snapshot(target(&url)).await {
            Ok(snap) => snap,
            // Skip ONLY when the user opted in via
            // PLUMB_E2E_CHROMIUM_SKIP — otherwise propagate so a
            // misconfigured host fails loudly.
            Err(err) if host_missing_chromium(&err) => return Ok(()),
            Err(err) => return Err(Box::<dyn std::error::Error>::from(err)),
        };
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
