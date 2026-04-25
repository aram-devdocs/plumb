//! Integration tests for [`plumb_config::merge_tailwind`].
//!
//! ## Skipping when Node is missing
//!
//! These tests spawn a real `node` subprocess. CI runners and most
//! workstations have Node; some Nix-style environments do not. When
//! `node` cannot be found we treat that as a skip — same shape as the
//! `e2e-chromium` tests in `plumb-cdp`.

// allow expect_used — integration-test helpers lack the #[test] proximity
// that clippy needs to apply `allow-expect-in-tests` from clippy.toml.
#![allow(clippy::expect_used)]

use std::path::PathBuf;
use std::time::Duration;

use plumb_config::{ConfigError, TailwindOptions, merge_tailwind};
use plumb_core::Config;

/// Returns `Some(path)` when a usable `node` is on PATH, `None`
/// otherwise. Tests that need a real subprocess early-return on `None`
/// so the suite stays green on Node-less hosts.
fn node_on_path() -> Option<PathBuf> {
    which::which("node").ok()
}

fn fixture_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p.push("tailwind");
    p
}

#[test]
fn merges_round_trip_tailwind_ts_config() {
    if node_on_path().is_none() {
        // Skip: host lacks Node.
        return;
    }
    let path = fixture_dir().join("tailwind.config.ts");
    if !path.exists() {
        // Skip: fixture not present (e.g. shallow checkout).
        return;
    }
    let cache_dir = tempfile::tempdir().expect("cache tempdir");
    let opts = TailwindOptions {
        cache_dir: Some(cache_dir.path().to_path_buf()),
        // The fixture lives under `crates/plumb-config/tests/...` —
        // explicitly anchor the guard at the workspace root so the
        // test passes regardless of the cwd cargo picks at runtime.
        cwd_root: Some(PathBuf::from(env!("CARGO_MANIFEST_DIR"))),
        ..Default::default()
    };
    let merged = match merge_tailwind(Config::default(), &path, &opts) {
        Ok(cfg) => cfg,
        Err(ConfigError::TailwindUnavailable { .. }) => {
            // Node disappeared between probe and spawn — treat as skip.
            return;
        }
        Err(ConfigError::TailwindEval { reason, .. }) if reason.contains("TS_LOADER_MISSING") => {
            // Host has no `tsx`/`ts-node`/`esbuild-register` installed.
            // The .ts round-trip is the real test; without a TS loader
            // we fall back to the .js fixture exercise via the other
            // tests in this file.
            return;
        }
        Err(other) => panic!("unexpected error: {other:?}"),
    };

    // Colours: flat token + nested group → slash-namespaced.
    assert_eq!(merged.color.tokens["white"], "#ffffff");
    assert_eq!(merged.color.tokens["red/500"], "#ef4444");
    assert_eq!(merged.color.tokens["red/600"], "#dc2626");

    // Spacing: rem → px at 16px = 1rem.
    assert_eq!(merged.spacing.tokens["1"], 4);
    assert_eq!(merged.spacing.tokens["4"], 16);
    // Scale rebuilt from tokens, sorted, deduped.
    assert!(merged.spacing.scale.contains(&4));
    assert!(merged.spacing.scale.contains(&16));

    // Type: fontSize tuple form keeps the size only.
    assert_eq!(merged.type_scale.tokens["base"], 16);
    assert_eq!(merged.type_scale.tokens["lg"], 18);
    assert_eq!(merged.type_scale.weights, vec![400, 500, 700]);
    assert!(merged.type_scale.families.contains(&"Inter".to_string()));

    // Radius: rem → px, deduped.
    assert!(merged.radius.scale.contains(&2));
    assert!(merged.radius.scale.contains(&4));
    assert!(merged.radius.scale.contains(&8));
}

/// Copy the JS fixture into `dir` so this test can mutate the file's
/// mtime without disturbing parallel tests that read the canonical
/// fixture under `tests/fixtures/tailwind/`. Returns the destination
/// path on success, or `None` if the source fixture is missing.
fn copy_fixture_to(dir: &std::path::Path) -> Option<PathBuf> {
    let src = fixture_dir().join("tailwind.config.js");
    if !src.exists() {
        return None;
    }
    let dst = dir.join("tailwind.config.js");
    std::fs::copy(&src, &dst).expect("copy fixture");
    Some(dst)
}

#[test]
fn cache_hit_skips_node_spawn() {
    if node_on_path().is_none() {
        return;
    }
    let fixture_root = tempfile::tempdir().expect("fixture tempdir");
    let Some(path) = copy_fixture_to(fixture_root.path()) else {
        return;
    };
    let cache_dir = tempfile::tempdir().expect("cache tempdir");
    let opts = TailwindOptions {
        cache_dir: Some(cache_dir.path().to_path_buf()),
        cwd_root: Some(fixture_root.path().to_path_buf()),
        ..Default::default()
    };

    // First call populates the cache.
    let first = merge_tailwind(Config::default(), &path, &opts).expect("first call");
    // Second call must be a cache hit. We force the issue by pointing
    // `node_path` at a non-existent binary; if the cache is honoured,
    // we never reach the `find_node` code path.
    let opts_with_bogus_node = TailwindOptions {
        cache_dir: Some(cache_dir.path().to_path_buf()),
        cwd_root: Some(fixture_root.path().to_path_buf()),
        node_path: Some(PathBuf::from("/definitely/not/a/node/binary")),
        ..Default::default()
    };
    let second =
        merge_tailwind(Config::default(), &path, &opts_with_bogus_node).expect("cache hit");

    // Same input → same merged config.
    assert_eq!(first.color.tokens, second.color.tokens);
    assert_eq!(first.spacing.tokens, second.spacing.tokens);
    assert_eq!(first.type_scale.tokens, second.type_scale.tokens);
}

#[test]
fn cache_invalidates_on_mtime_bump() {
    if node_on_path().is_none() {
        return;
    }
    let fixture_root = tempfile::tempdir().expect("fixture tempdir");
    let Some(path) = copy_fixture_to(fixture_root.path()) else {
        return;
    };
    let cache_dir = tempfile::tempdir().expect("cache tempdir");
    let opts = TailwindOptions {
        cache_dir: Some(cache_dir.path().to_path_buf()),
        cwd_root: Some(fixture_root.path().to_path_buf()),
        ..Default::default()
    };
    let _ = merge_tailwind(Config::default(), &path, &opts).expect("first call");

    // Bump the fixture mtime to a guaranteed-distinct future timestamp.
    let later = std::time::UNIX_EPOCH + std::time::Duration::from_secs(2_000_000_000);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("open fixture")
        .set_modified(later)
        .expect("set mtime");

    // Cache miss → must re-spawn. Force failure if it tried the cache
    // path: same bogus node trick. With an mtime mismatch the cache
    // won't be consulted, so this must error with `TailwindUnavailable`.
    let opts_with_bogus_node = TailwindOptions {
        cache_dir: Some(cache_dir.path().to_path_buf()),
        cwd_root: Some(fixture_root.path().to_path_buf()),
        node_path: Some(PathBuf::from("/definitely/not/a/node/binary")),
        ..Default::default()
    };
    let err = merge_tailwind(Config::default(), &path, &opts_with_bogus_node)
        .expect_err("cache should miss on mtime change");
    assert!(matches!(err, ConfigError::TailwindUnavailable { .. }));
}

#[test]
fn errors_when_node_explicit_path_missing() {
    let path = fixture_dir().join("tailwind.config.js");
    if !path.exists() {
        return;
    }
    let cache_dir = tempfile::tempdir().expect("cache tempdir");
    let opts = TailwindOptions {
        cache_dir: Some(cache_dir.path().to_path_buf()),
        node_path: Some(PathBuf::from("/definitely/not/a/node/binary")),
        no_cache: true,
        cwd_root: Some(PathBuf::from(env!("CARGO_MANIFEST_DIR"))),
        ..Default::default()
    };
    let err = merge_tailwind(Config::default(), &path, &opts).expect_err("must error");
    let ConfigError::TailwindUnavailable { reason } = err else {
        panic!("expected TailwindUnavailable, got {err:?}");
    };
    assert!(reason.contains("does not exist"), "reason = {reason}");
}

#[test]
fn errors_on_unsupported_extension() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("tailwind.config.json5");
    std::fs::write(&path, "{}").expect("write");
    let opts = TailwindOptions::default();
    let err = merge_tailwind(Config::default(), &path, &opts).expect_err("must error");
    assert!(matches!(err, ConfigError::TailwindBadPath { .. }));
}

// Note: a "path resolves outside cwd" test would need to mutate the
// process-global CWD, which races with parallel tests in this file.
// The unit-test `is_under_or_ancestor_rejects_unrelated` covers the
// rejection logic deterministically.

/// Determinism invariant: calling `merge_tailwind` twice on the same
/// fixture MUST produce byte-identical output, including across the
/// cache-miss → cache-hit boundary. The first call populates the
/// cache; the second call reads from it. Both are functions of
/// `(snapshot, config)` only, so they must agree exactly.
#[test]
fn merge_is_byte_identical_across_runs() {
    if node_on_path().is_none() {
        return;
    }
    let fixture_root = tempfile::tempdir().expect("fixture tempdir");
    let Some(path) = copy_fixture_to(fixture_root.path()) else {
        return;
    };
    let cache_dir = tempfile::tempdir().expect("cache tempdir");
    let opts = TailwindOptions {
        cache_dir: Some(cache_dir.path().to_path_buf()),
        cwd_root: Some(fixture_root.path().to_path_buf()),
        ..Default::default()
    };

    // First call: cache miss — spawns Node, populates the cache.
    let first = merge_tailwind(Config::default(), &path, &opts).expect("first call");
    // Second call: cache hit — reads the cache file we just wrote.
    let second = merge_tailwind(Config::default(), &path, &opts).expect("second call");

    // `Config` is `PartialEq` (see `plumb-core::config`), so this
    // covers every nested spec — color, spacing, type_scale, radius,
    // alignment, a11y — without piecewise comparison.
    assert_eq!(
        first, second,
        "merge_tailwind output must be byte-identical across runs"
    );
}

#[test]
fn errors_when_node_subprocess_times_out() {
    if node_on_path().is_none() {
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    // A `.cjs` script that intentionally never exits. `setInterval`
    // keeps the event loop alive without busy-waiting; the polling
    // loop in `spawn_loader` should kill it once the budget elapses.
    let cfg_path = dir.path().join("tailwind.config.cjs");
    std::fs::write(&cfg_path, "setInterval(() => {}, 1000);\n").expect("write hang script");

    let cache_dir = tempfile::tempdir().expect("cache tempdir");
    let opts = TailwindOptions {
        cache_dir: Some(cache_dir.path().to_path_buf()),
        cwd_root: Some(dir.path().to_path_buf()),
        // Bypass the cache so the spawn happens.
        no_cache: true,
        // 200 ms is long enough to spin up Node on slow CI but short
        // enough that the test wraps up well under the suite's
        // per-test budget.
        timeout: Some(Duration::from_millis(200)),
        ..Default::default()
    };

    let err = merge_tailwind(Config::default(), &cfg_path, &opts)
        .expect_err("hanging subprocess must surface a TailwindEval error");
    let ConfigError::TailwindEval { reason, .. } = err else {
        panic!("expected TailwindEval, got {err:?}");
    };
    assert!(
        reason.contains("timed out"),
        "expected reason to mention `timed out`, got `{reason}`"
    );
}
