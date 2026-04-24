//! Golden snapshot of the walking-skeleton engine run.
//!
//! Proves `engine::run` produces deterministic, sorted output given the
//! canned snapshot + default config.

use plumb_core::{Config, PlumbSnapshot, Rect, SnapshotCtx, ViewportKey, run};

#[test]
fn hello_world_golden() -> Result<(), serde_json::Error> {
    let snapshot = PlumbSnapshot::canned();
    let config = Config::default();
    let violations = run(&snapshot, &config);

    let json = serde_json::to_string_pretty(&violations)?;
    insta::assert_snapshot!("hello_world", json);
    Ok(())
}

#[test]
fn engine_run_is_deterministic() -> Result<(), serde_json::Error> {
    let snapshot = PlumbSnapshot::canned();
    let config = Config::default();
    let a = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let b = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let c = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    assert_eq!(a, b);
    assert_eq!(b, c);
    Ok(())
}

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
