//! Default-implementation contract for [`BrowserDriver::snapshot_all`].
//!
//! Verifies that the trait's default `snapshot_all` calls `snapshot`
//! per target in input order and that [`FakeDriver`] is viewport-aware
//! (carries each target's viewport name and dimensions onto the
//! returned snapshot).

use plumb_cdp::{BrowserDriver, CdpError, FakeDriver, Target};
use plumb_core::ViewportKey;

fn target(viewport: &str, width: u32, height: u32) -> Target {
    Target {
        url: "plumb-fake://hello".into(),
        viewport: ViewportKey::new(viewport),
        width,
        height,
        device_pixel_ratio: 1.0,
    }
}

#[tokio::test]
async fn fake_driver_snapshot_all_preserves_target_order_and_viewport() -> Result<(), CdpError> {
    let driver = FakeDriver;
    let targets = vec![target("mobile", 375, 812), target("desktop", 1280, 800)];

    let snapshots = driver.snapshot_all(targets).await?;

    assert_eq!(snapshots.len(), 2);

    assert_eq!(snapshots[0].viewport, ViewportKey::new("mobile"));
    assert_eq!(snapshots[0].viewport_width, 375);
    assert_eq!(snapshots[0].viewport_height, 812);

    assert_eq!(snapshots[1].viewport, ViewportKey::new("desktop"));
    assert_eq!(snapshots[1].viewport_width, 1280);
    assert_eq!(snapshots[1].viewport_height, 800);

    Ok(())
}

/// Regression for #121: the canned snapshot's html/body rects are
/// viewport-sized, so when a non-default viewport is requested the
/// fake driver MUST rewrite those rects to match the target's
/// width/height — otherwise hand-testing multi-viewport behavior
/// against `plumb-fake://` reports desktop rects on mobile.
#[tokio::test]
async fn fake_driver_rewrites_viewport_sized_rects_to_target() -> Result<(), CdpError> {
    let driver = FakeDriver;
    let snap = driver.snapshot(target("mobile", 375, 667)).await?;

    assert_eq!(snap.viewport_width, 375);
    assert_eq!(snap.viewport_height, 667);

    let html = snap
        .nodes
        .iter()
        .find(|n| n.tag == "html")
        .expect("canned snapshot has an <html> root");
    let html_rect = html.rect.expect("html node has a rect");
    assert_eq!(html_rect.x, 0);
    assert_eq!(html_rect.y, 0);
    assert_eq!(
        html_rect.width, 375,
        "html rect width follows target viewport"
    );
    assert_eq!(
        html_rect.height, 667,
        "html rect height follows target viewport"
    );

    let body = snap
        .nodes
        .iter()
        .find(|n| n.tag == "body")
        .expect("canned snapshot has a <body>");
    let body_rect = body.rect.expect("body node has a rect");
    assert_eq!(
        body_rect.width, 375,
        "body rect width follows target viewport"
    );
    assert_eq!(
        body_rect.height, 667,
        "body rect height follows target viewport"
    );

    Ok(())
}
