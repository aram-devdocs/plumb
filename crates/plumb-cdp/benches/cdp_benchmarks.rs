//! Criterion benchmarks for the Plumb CDP pipeline.
//!
//! ## Benchmark groups
//!
//! | Group              | What it measures                                     | Chromium? |
//! |--------------------|------------------------------------------------------|-----------|
//! | `per_rule_dom`     | Rule-engine cost on 100 / 1 000 / 10 000 node DOMs  | No        |
//! | `cold_start`       | Launch Chromium + first snapshot                     | Yes       |
//! | `warm_run`         | Subsequent snapshot on a reused browser               | Yes       |
//!
//! ## Running locally
//!
//! ```sh
//! # Rule-engine benchmarks only (no Chromium required):
//! cargo bench -p plumb-cdp
//!
//! # Full suite including CDP cold-start / warm-run (requires Chromium):
//! cargo bench -p plumb-cdp --features e2e-chromium
//! ```

// Benchmarks are standalone binaries; relax workspace lint strictness that
// is impractical outside library code.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::wildcard_imports,
    unreachable_pub,
    missing_docs
)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use indexmap::IndexMap;
use plumb_core::snapshot::SnapshotNode;
use plumb_core::{Config, PlumbSnapshot, Rect, ViewportKey};

// ---------------------------------------------------------------------------
// Synthetic snapshot builder
// ---------------------------------------------------------------------------

/// Build a `PlumbSnapshot` with `n` leaf `<div>` nodes under `<html><body>`.
///
/// Every node gets realistic computed styles so rules have work to do:
/// - `padding-top` / `margin-bottom` set to values deliberately off the
///   default 4 px spacing grid so `spacing/grid-conformance` fires.
/// - `font-size` set to `15px` (off the default type scale).
/// - A bounding `Rect` sized for a 1 280 × 800 viewport.
fn synthetic_snapshot(n: usize) -> PlumbSnapshot {
    let child_orders: Vec<u64> = (2..2 + n as u64).collect();

    let mut nodes = Vec::with_capacity(n + 2);

    // <html> root
    nodes.push(SnapshotNode {
        dom_order: 0,
        selector: "html".into(),
        tag: "html".into(),
        attrs: IndexMap::new(),
        computed_styles: IndexMap::new(),
        rect: Some(Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 800,
        }),
        parent: None,
        children: vec![1],
    });

    // <body>
    nodes.push(SnapshotNode {
        dom_order: 1,
        selector: "html > body".into(),
        tag: "body".into(),
        attrs: IndexMap::new(),
        computed_styles: IndexMap::new(),
        rect: Some(Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 800,
        }),
        parent: Some(0),
        children: child_orders,
    });

    // Leaf <div> nodes
    for i in 0..n {
        let dom_order = 2 + i as u64;
        let mut styles = IndexMap::new();
        // Off-grid spacing to exercise spacing rules.
        styles.insert("padding-top".into(), "13px".into());
        styles.insert("margin-bottom".into(), "7px".into());
        // Off-scale font size to exercise type rules.
        styles.insert("font-size".into(), "15px".into());
        styles.insert("line-height".into(), "20px".into());
        styles.insert("display".into(), "block".into());
        styles.insert("color".into(), "rgb(51, 51, 51)".into());
        styles.insert("background-color".into(), "rgb(255, 255, 255)".into());

        #[allow(clippy::cast_possible_truncation)]
        let row = (i / 3) as u32;
        #[allow(clippy::cast_possible_truncation)]
        let col = (i % 3) as u32;
        let card_w: u32 = 400;
        let card_h: u32 = 120;

        nodes.push(SnapshotNode {
            dom_order,
            selector: format!("html > body > div:nth-child({})", i + 1),
            tag: "div".into(),
            attrs: IndexMap::new(),
            computed_styles: styles,
            rect: Some(Rect {
                #[allow(clippy::cast_possible_wrap)]
                x: (col * card_w) as i32,
                #[allow(clippy::cast_possible_wrap)]
                y: (row * card_h) as i32,
                width: card_w,
                height: card_h,
            }),
            parent: Some(1),
            children: Vec::new(),
        });
    }

    PlumbSnapshot {
        url: format!("plumb-fake://bench-{n}"),
        viewport: ViewportKey::new("desktop"),
        viewport_width: 1280,
        viewport_height: 800,
        nodes,
    }
}

// ---------------------------------------------------------------------------
// per_rule_dom — rule-engine cost on varying DOM sizes
// ---------------------------------------------------------------------------

fn per_rule_dom(c: &mut Criterion) {
    let config = Config::default();
    let mut group = c.benchmark_group("per_rule_dom");

    for &size in &[100_usize, 1_000, 10_000] {
        let snapshot = synthetic_snapshot(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &snapshot, |b, snap| {
            b.iter(|| plumb_core::run(snap, &config));
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// CDP benchmarks — require a real Chromium binary (e2e-chromium feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "e2e-chromium")]
mod cdp {
    use super::*;
    use plumb_cdp::{BrowserDriver, ChromiumDriver, ChromiumOptions, PersistentBrowser, Target};
    use std::path::PathBuf;
    use tokio::runtime::Runtime;

    /// Build a [`Target`] pointing at a local bench fixture HTML file.
    fn fixture_target(name: &str) -> Target {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("benches/fixtures")
            .join(name);
        Target {
            url: format!("file://{}", path.display()),
            viewport: ViewportKey::new("desktop"),
            width: 1280,
            height: 800,
            device_pixel_ratio: 1.0,
        }
    }

    /// cold_start: construct a fresh `ChromiumDriver`, snapshot one page
    /// (which launches and tears down the browser each call).
    pub fn cold_start(c: &mut Criterion) {
        let rt = Runtime::new().unwrap();
        let target = fixture_target("fixed-dom-1k-nodes.html");

        c.bench_function("cold_start", |b| {
            b.iter(|| {
                rt.block_on(async {
                    let driver = ChromiumDriver::new(ChromiumOptions::default());
                    let _snap = driver.snapshot(target.clone()).await.expect("snapshot");
                });
            });
        });
    }

    /// warm_run: reuse a `PersistentBrowser` for subsequent snapshots.
    pub fn warm_run(c: &mut Criterion) {
        let rt = Runtime::new().unwrap();
        let target = fixture_target("fixed-dom-1k-nodes.html");

        let browser = rt
            .block_on(PersistentBrowser::launch(ChromiumOptions::default()))
            .expect("Chromium launch");

        c.bench_function("warm_run", |b| {
            b.iter(|| {
                rt.block_on(async {
                    let _snap = browser.snapshot(target.clone()).await.expect("snapshot");
                });
            });
        });

        rt.block_on(browser.shutdown()).expect("shutdown");
    }
}

// ---------------------------------------------------------------------------
// Group registration
// ---------------------------------------------------------------------------

// Always register per_rule_dom (pure, no Chromium needed).
#[cfg(not(feature = "e2e-chromium"))]
criterion_group!(benches, per_rule_dom);

// With Chromium available, register the full suite.
#[cfg(feature = "e2e-chromium")]
criterion_group!(benches, per_rule_dom, cdp::cold_start, cdp::warm_run);

criterion_main!(benches);
