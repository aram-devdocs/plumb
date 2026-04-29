//! Contract tests for [`plumb_cdp::PersistentBrowser`].
//!
//! Unit-scope checks (target-helper shape, missing-executable rejection)
//! always run; the rest are gated on `feature = "e2e-chromium"` because
//! they need a real Chromium binary in the supported major-version
//! range. The e2e suite covers:
//!
//! - State isolation: localStorage written in call N does not leak into
//!   call N+1.
//! - Warm reuse: the second snapshot is meaningfully faster than the
//!   first because Chromium is already running.
//! - Graceful shutdown: `shutdown` is idempotent across repeated calls.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::missing_panics_doc)]

use plumb_cdp::{CdpError, ChromiumOptions, PersistentBrowser, Target};
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
    let canonical = path.canonicalize()?;
    Ok(format!("file://{}", canonical.display()))
}

/// Skip-on-missing parity with the rest of the e2e suite. See
/// `driver_contract.rs` for the rationale and opt-in flag.
#[cfg(feature = "e2e-chromium")]
const SKIP_ENV_VAR: &str = "PLUMB_E2E_CHROMIUM_SKIP";

#[cfg(feature = "e2e-chromium")]
fn host_missing_chromium(err: &CdpError) -> bool {
    let unavailable = matches!(
        err,
        CdpError::ChromiumNotFound { .. } | CdpError::UnsupportedChromium { .. }
    );
    unavailable && std::env::var_os(SKIP_ENV_VAR).is_some()
}

/// `Target` lookup for unit tests that don't need to drive a browser.
#[test]
fn target_helper_builds_desktop_viewport() {
    let t = target("https://example.com");
    assert_eq!(t.viewport.as_str(), "desktop");
    assert_eq!(t.width, 1280);
    assert_eq!(t.height, 800);
}

/// A bogus executable path produces `ChromiumNotFound` even on the
/// persistent path — the error never blocks construction of an `Arc`
/// holding a half-initialized browser. Runs without `e2e-chromium`
/// because `ensure_executable_path` rejects the missing file
/// synchronously, before any browser launch.
#[tokio::test]
async fn persistent_browser_rejects_missing_executable() {
    let result = PersistentBrowser::launch(ChromiumOptions {
        executable_path: Some(std::path::PathBuf::from(
            "/definitely/not/a/chromium/binary",
        )),
        ..ChromiumOptions::default()
    })
    .await;

    assert!(matches!(result, Err(CdpError::ChromiumNotFound { .. })));
}

#[cfg(feature = "e2e-chromium")]
fn isolated_options() -> std::io::Result<(ChromiumOptions, tempfile::TempDir)> {
    let dir = tempfile::tempdir()?;
    Ok((
        ChromiumOptions {
            executable_path: None,
            user_data_dir: Some(dir.path().to_path_buf()),
        },
        dir,
    ))
}

/// The second call is materially faster than the first because Chromium
/// stays warm across calls.
///
/// The PRD's 3x target assumes Chromium launch dominates the cold
/// elapsed; that holds for tiny local fixtures but not for real
/// internet pages where `wait_for_navigation` dwarfs launch. To stay
/// useful as a CI regression guard we assert `cold > warm * 1.3` —
/// strong enough to catch a regression where every call re-launches
/// Chromium (which would push the ratio to ~1.0x), and loose enough
/// to absorb CI noise. The 3x target is exercised manually on
/// real URLs in `docs/local/prd.md` §6.1.
///
/// Timing here is in the test harness only — never fed into a
/// `PlumbSnapshot`, which would violate the determinism invariant.
/// `Instant::now` is banned in library code (PRD §9), so the allow is
/// scoped to this single function rather than the whole file — that
/// keeps the ban active for any other timing source someone might
/// accidentally reach for elsewhere in this file.
#[cfg(feature = "e2e-chromium")]
#[tokio::test]
#[allow(clippy::disallowed_methods)]
async fn persistent_browser_warm_call_is_faster_than_cold() {
    let url = match fixture_url("static_page.html") {
        Ok(u) => u,
        Err(err) => panic!("fixture path: {err}"),
    };
    let (options, _profile) = isolated_options().expect("tempdir");

    let cold_start = std::time::Instant::now();
    let browser = match PersistentBrowser::launch(options).await {
        Ok(b) => b,
        Err(err) if host_missing_chromium(&err) => return,
        Err(err) => panic!("launch failed: {err}"),
    };
    let _first = match browser.snapshot(target(&url)).await {
        Ok(s) => s,
        Err(err) if host_missing_chromium(&err) => {
            let _ = browser.shutdown().await;
            return;
        }
        Err(err) => panic!("first snapshot: {err}"),
    };
    let cold = cold_start.elapsed();

    let warm_start = std::time::Instant::now();
    let _second = match browser.snapshot(target(&url)).await {
        Ok(s) => s,
        Err(err) => panic!("second snapshot: {err}"),
    };
    let warm = warm_start.elapsed();

    let _ = browser.shutdown().await;

    // `cold > warm * 1.3` — regression guard. Equivalent integer math:
    // `cold * 10 > warm * 13`. The factor catches re-launch-per-call
    // regressions (which converge to ~1.0x) without flaking on a busy
    // runner.
    assert!(
        cold * 10 > warm * 13,
        "warm call should be meaningfully faster than cold (cold={cold:?}, warm={warm:?}); regression guard expects cold > warm * 1.3"
    );
}

/// State written by call N must not leak into call N+1 because each
/// snapshot opens a fresh incognito `BrowserContext`.
///
/// The `stateful_page.html` fixture writes a marker into
/// `window.localStorage` on every visit and renders one of two values
/// into the `data-marker` attribute of `<main>`: `state-fresh` when
/// the read-before-write returned `null`, `state-leaked` when a prior
/// call's write was still observable. With incognito isolation working
/// the second snapshot reads `null` again and renders `state-fresh`;
/// without isolation it would render `state-leaked` and the assertion
/// below would fail.
#[cfg(feature = "e2e-chromium")]
#[tokio::test]
async fn persistent_browser_isolates_state_between_calls() {
    let url = match fixture_url("stateful_page.html") {
        Ok(u) => u,
        Err(err) => panic!("fixture path: {err}"),
    };
    let (options, _profile) = isolated_options().expect("tempdir");
    let browser = match PersistentBrowser::launch(options).await {
        Ok(b) => b,
        Err(err) if host_missing_chromium(&err) => return,
        Err(err) => panic!("launch failed: {err}"),
    };

    let first = match browser.snapshot(target(&url)).await {
        Ok(s) => s,
        Err(err) if host_missing_chromium(&err) => {
            let _ = browser.shutdown().await;
            return;
        }
        Err(err) => panic!("first snapshot: {err}"),
    };
    let second = match browser.snapshot(target(&url)).await {
        Ok(s) => s,
        Err(err) => panic!("second snapshot: {err}"),
    };

    let _ = browser.shutdown().await;

    let first_marker = state_marker(&first);
    let second_marker = state_marker(&second);

    assert_eq!(
        first_marker, "state-fresh",
        "first snapshot must render `state-fresh` — the fixture writes \
         localStorage on every visit, so a value other than `state-fresh` \
         here means the read-before-write surfaced state from a prior call"
    );
    assert_eq!(
        second_marker, "state-fresh",
        "second snapshot must render `state-fresh` — observing \
         `state-leaked` would mean call 1's localStorage write was still \
         visible to call 2, proving the incognito BrowserContext was reused"
    );

    let first_json = serde_json::to_string(&first).expect("serialize first");
    let second_json = serde_json::to_string(&second).expect("serialize second");
    assert_eq!(
        first_json, second_json,
        "back-to-back snapshots over fresh incognito contexts must be byte-identical"
    );
}

/// Pull the `data-marker` attribute off the fixture's `<main>` element.
/// Returns the literal string `"state-missing"` if the element or its
/// attribute are absent — surfaced in the assertion message rather than
/// panicking so the test failure points at what was actually wrong.
#[cfg(feature = "e2e-chromium")]
fn state_marker(snap: &plumb_core::PlumbSnapshot) -> String {
    snap.nodes
        .iter()
        .find(|n| n.tag == "main")
        .and_then(|n| n.attrs.get("data-marker").cloned())
        .unwrap_or_else(|| "state-missing".to_string())
}

/// `shutdown` survives being called twice — the second call observes
/// the absent handler task and short-circuits without hitting CDP.
#[cfg(feature = "e2e-chromium")]
#[tokio::test]
async fn persistent_browser_shutdown_is_idempotent() {
    let (options, _profile) = isolated_options().expect("tempdir");
    let browser = match PersistentBrowser::launch(options).await {
        Ok(b) => b,
        Err(err) if host_missing_chromium(&err) => return,
        Err(err) => panic!("launch failed: {err}"),
    };

    browser.shutdown().await.expect("first shutdown ok");
    browser
        .shutdown()
        .await
        .expect("second shutdown must remain idempotent");
}
