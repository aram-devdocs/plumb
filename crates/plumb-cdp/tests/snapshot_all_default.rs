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
