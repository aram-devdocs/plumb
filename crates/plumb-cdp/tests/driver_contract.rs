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
