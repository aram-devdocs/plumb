//! Plumb-side Chromium path discovery.
//!
//! On macOS, `chromiumoxide`'s default detection picks `chromium` from
//! `PATH` over a `/Applications` `.app` bundle. The Homebrew **formula**
//! `chromium` (not the cask) installs a developer build whose CDP
//! WebSocket hangs — every `plumb lint <real-url>` then times out at 30 s
//! with an unhelpful `driver failure: Request timed out`.
//!
//! The fix is to consult a Plumb-managed priority list **before**
//! delegating to chromiumoxide. The list prefers the `.app` bundles that
//! Google Chrome and Chromium ship on macOS so Mac users with both a
//! brew chromium and Google Chrome installed get the latter (the same
//! channel CI tests against). On Linux and Windows this module returns
//! `None` and the caller falls back to chromiumoxide's auto-detect,
//! which already does the right thing on those platforms.
//!
//! Logged at INFO when a path is selected so users can see which
//! channel was picked; silent otherwise.

use std::path::{Path, PathBuf};

/// Filesystem probe abstraction. Real callers use [`StdFsProbe`]; the
/// unit tests substitute an in-memory probe that asserts the priority
/// order without touching the actual filesystem.
pub(crate) trait FsProbe {
    /// Return `true` if `path` resolves to a regular file the current
    /// process can read.
    fn is_file(&self, path: &Path) -> bool;

    /// Return the value of the user's home directory, if any. The
    /// probe owns the lookup so tests can pin a stable home root.
    fn home_dir(&self) -> Option<PathBuf>;
}

/// Real probe — defers to [`Path::is_file`] and the `HOME` env var.
pub(crate) struct StdFsProbe;

impl FsProbe for StdFsProbe {
    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn home_dir(&self) -> Option<PathBuf> {
        // `HOME` is the macOS-correct env var; we only call this from
        // the macOS branch. Skipping the broader `dirs` crate keeps the
        // dependency footprint small.
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

/// Resolve a Chromium binary path that Plumb prefers over
/// chromiumoxide's auto-detect.
///
/// Returns `Some(path)` only when the host is macOS **and** one of the
/// canonical `.app` bundles exists. Returns `None` otherwise so the
/// caller falls through to chromiumoxide's existing detection — which
/// is the correct behavior on Linux (PATH `chromium` / `google-chrome`)
/// and Windows (registry-based discovery).
#[must_use]
pub fn detect() -> Option<PathBuf> {
    detect_with(&StdFsProbe)
}

/// `detect()` parameterized over a [`FsProbe`] for testability.
pub(crate) fn detect_with<P: FsProbe>(probe: &P) -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let candidates = macos_candidates(probe);
    let selected = candidates.into_iter().find(|p| probe.is_file(p));
    if let Some(path) = selected.as_ref() {
        tracing::info!(chrome_path = %path.display(), "selected chromium binary");
    }
    selected
}

/// macOS Chromium-binary search order.
///
/// The order matches the audit finding behind PR 6:
/// 1. `/Applications/Google Chrome.app` — the channel CI tests against.
/// 2. `/Applications/Google Chrome Canary.app` — beta channel.
/// 3. `/Applications/Chromium.app` — open-source build (cask).
/// 4. The same three under `~/Applications` (per-user installs).
///
/// PATH `chromium` / `chrome` and chromiumoxide's bundled-fetch path
/// remain available as later fallbacks because this function only
/// returns the **preferred** path; an empty result keeps the existing
/// detection chain.
fn macos_candidates<P: FsProbe>(probe: &P) -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
        PathBuf::from("/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary"),
        PathBuf::from("/Applications/Chromium.app/Contents/MacOS/Chromium"),
    ];
    if let Some(home) = probe.home_dir() {
        paths.push(home.join("Applications/Google Chrome.app/Contents/MacOS/Google Chrome"));
        paths.push(
            home.join("Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary"),
        );
        paths.push(home.join("Applications/Chromium.app/Contents/MacOS/Chromium"));
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::{FsProbe, detect_with, macos_candidates};
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};

    /// In-memory probe used by the unit tests. Pretends a fixed set of
    /// paths exist on disk.
    struct FakeFs {
        present: HashSet<PathBuf>,
        home: Option<PathBuf>,
    }

    impl FakeFs {
        fn new(paths: &[&str], home: Option<&str>) -> Self {
            Self {
                present: paths.iter().map(PathBuf::from).collect(),
                home: home.map(PathBuf::from),
            }
        }
    }

    impl FsProbe for FakeFs {
        fn is_file(&self, path: &Path) -> bool {
            self.present.contains(path)
        }

        fn home_dir(&self) -> Option<PathBuf> {
            self.home.clone()
        }
    }

    /// Static-ordering check that doesn't probe the filesystem. Gated
    /// to `unix` because the assertions compare forward-slash literals
    /// against `Path::display()`, which uses `\` separators on Windows
    /// — and `macos_candidates` is only reached in production on macOS
    /// (see `detect_with`), so Windows coverage adds no signal.
    #[cfg(unix)]
    #[test]
    fn macos_candidate_order_lists_system_apps_then_user_apps() {
        let fs = FakeFs::new(&[], Some("/Users/example"));
        let candidates = macos_candidates(&fs);
        let display: Vec<String> = candidates.iter().map(|p| p.display().to_string()).collect();
        assert_eq!(display.len(), 6);
        assert!(display[0].contains("/Applications/Google Chrome.app"));
        assert!(display[1].contains("/Applications/Google Chrome Canary.app"));
        assert!(display[2].contains("/Applications/Chromium.app"));
        assert!(display[3].starts_with("/Users/example/Applications/Google Chrome.app"));
        assert!(display[4].starts_with("/Users/example/Applications/Google Chrome Canary.app"));
        assert!(display[5].starts_with("/Users/example/Applications/Chromium.app"));
    }

    /// Reproducer for the audit finding: when both Google Chrome and
    /// the brew formula chromium are present, Plumb selects Chrome.
    /// The brew binary lives on `PATH`, so it never appears in the
    /// `.app`-bundle list at all — the priority is enforced
    /// implicitly by `detect_with` only returning `.app` paths.
    #[cfg(target_os = "macos")]
    #[test]
    fn selects_google_chrome_when_present() {
        let fs = FakeFs::new(
            &["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"],
            None,
        );
        let selected = detect_with(&fs).expect("Google Chrome present");
        assert!(
            selected
                .display()
                .to_string()
                .contains("/Applications/Google Chrome.app")
        );
    }

    /// `Google Chrome Canary` wins when only Canary is installed.
    #[cfg(target_os = "macos")]
    #[test]
    fn falls_through_to_canary_when_chrome_missing() {
        let fs = FakeFs::new(
            &["/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary"],
            None,
        );
        let selected = detect_with(&fs).expect("Canary present");
        assert!(
            selected
                .display()
                .to_string()
                .contains("Google Chrome Canary")
        );
    }

    /// Chromium.app (the cask, not the brew formula) wins as the third
    /// system-Applications candidate.
    #[cfg(target_os = "macos")]
    #[test]
    fn falls_through_to_chromium_app_when_chrome_and_canary_missing() {
        let fs = FakeFs::new(
            &["/Applications/Chromium.app/Contents/MacOS/Chromium"],
            None,
        );
        let selected = detect_with(&fs).expect("Chromium.app present");
        assert!(
            selected
                .display()
                .to_string()
                .contains("/Applications/Chromium.app/Contents/MacOS/Chromium")
        );
    }

    /// User-install Chrome is preferred over user-install Chromium.
    #[cfg(target_os = "macos")]
    #[test]
    fn user_chrome_app_beats_user_chromium_app() {
        let fs = FakeFs::new(
            &[
                "/Users/example/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Users/example/Applications/Chromium.app/Contents/MacOS/Chromium",
            ],
            Some("/Users/example"),
        );
        let selected = detect_with(&fs).expect("user-install present");
        assert!(
            selected
                .display()
                .to_string()
                .contains("Google Chrome.app/Contents/MacOS/Google Chrome")
        );
    }

    /// System Chrome wins over user Chromium — regardless of which
    /// directory `chromiumoxide` would have inspected first.
    #[cfg(target_os = "macos")]
    #[test]
    fn system_chrome_beats_user_chromium() {
        let fs = FakeFs::new(
            &[
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Users/example/Applications/Chromium.app/Contents/MacOS/Chromium",
            ],
            Some("/Users/example"),
        );
        let selected = detect_with(&fs).expect("system Chrome present");
        assert!(
            selected
                .display()
                .to_string()
                .starts_with("/Applications/Google Chrome.app")
        );
    }

    /// No candidate present → `None`, and the caller falls back to
    /// chromiumoxide's existing detection.
    #[cfg(target_os = "macos")]
    #[test]
    fn returns_none_when_no_apps_installed() {
        let fs = FakeFs::new(&[], Some("/Users/example"));
        assert!(detect_with(&fs).is_none());
    }

    /// On non-macOS hosts the function unconditionally returns `None`
    /// even when a `.app` bundle path happens to exist.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_always_returns_none() {
        let fs = FakeFs::new(
            &["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"],
            None,
        );
        assert!(detect_with(&fs).is_none());
    }
}
