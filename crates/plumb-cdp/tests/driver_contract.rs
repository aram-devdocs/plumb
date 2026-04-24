//! Contract tests for [`BrowserDriver`]. Runs against `FakeDriver`;
//! `ChromiumDriver` will be covered by integration tests once PR #2 lands.

use plumb_cdp::{BrowserDriver, CdpError, ChromiumDriver, FakeDriver, Target, is_fake_url};
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
async fn fake_driver_returns_canned_snapshot() {
    let driver = FakeDriver;
    let snap = driver
        .snapshot(target("plumb-fake://hello"))
        .await
        .expect("canned");
    assert_eq!(snap.url, "plumb-fake://hello");
    assert_eq!(snap.viewport.as_str(), "desktop");
    assert!(!snap.nodes.is_empty());
}

#[tokio::test]
async fn fake_driver_rejects_unknown_urls() {
    let driver = FakeDriver;
    let err = driver
        .snapshot(target("plumb-fake://unknown"))
        .await
        .unwrap_err();
    assert!(matches!(err, CdpError::UnknownFakeUrl(_)));
}

#[tokio::test]
async fn real_driver_is_not_implemented_yet() {
    let driver = ChromiumDriver;
    let err = driver
        .snapshot(target("https://example.com"))
        .await
        .unwrap_err();
    assert!(matches!(err, CdpError::NotImplemented));
}

#[test]
fn fake_url_detector() {
    assert!(is_fake_url("plumb-fake://hello"));
    assert!(!is_fake_url("https://plumb.aramhammoudeh.com"));
    assert!(!is_fake_url(""));
}
