//! Contract tests for the `SnapshotCtx` API.
//!
//! These tests landed alongside the walking-skeleton golden in an earlier
//! revision. They exercise the `SnapshotCtx` constructors and the
//! `rect_for` lookup directly — every rule author depends on this surface,
//! so it deserves its own home rather than riding along with whichever
//! golden test happens to use `PlumbSnapshot::canned()`.

use plumb_core::report::Rect;
use plumb_core::{PlumbSnapshot, SnapshotCtx, ViewportKey};

#[test]
fn snapshot_ctx_new_exposes_snapshot_viewport() {
    let snapshot = PlumbSnapshot::canned();
    let ctx = SnapshotCtx::new(&snapshot);

    assert_eq!(ctx.viewports(), &[ViewportKey::new("desktop")]);
}

#[test]
fn snapshot_ctx_with_viewports_preserves_supplied_order() {
    let snapshot = PlumbSnapshot::canned();
    let viewports = vec![
        ViewportKey::new("mobile"),
        ViewportKey::new("tablet"),
        ViewportKey::new("desktop"),
    ];
    let ctx = SnapshotCtx::with_viewports(&snapshot, viewports);

    assert_eq!(
        ctx.viewports(),
        &[
            ViewportKey::new("mobile"),
            ViewportKey::new("tablet"),
            ViewportKey::new("desktop"),
        ],
    );
}

#[test]
fn snapshot_ctx_rect_for_uses_precomputed_rect_index() {
    let snapshot = PlumbSnapshot::canned();
    let ctx = SnapshotCtx::new(&snapshot);

    let full_viewport_rect = Some(Rect {
        x: 0,
        y: 0,
        width: 1280,
        height: 800,
    });

    assert_eq!(ctx.rect_for(0), full_viewport_rect);
    assert_eq!(ctx.rect_for(1), None);
    assert_eq!(ctx.rect_for(2), full_viewport_rect);
    assert_eq!(ctx.rect_for(99), None);
}
