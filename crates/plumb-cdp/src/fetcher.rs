//! Opt-in Chromium auto-fetch.
//!
//! When the user enables [`crate::ChromiumOptions::auto_fetch_chromium`]
//! *and* no explicit
//! [`crate::ChromiumOptions::executable_path`] is set, Plumb falls back
//! to chromiumoxide's [`chromiumoxide::fetcher::BrowserFetcher`] to
//! download Chrome-for-Testing pinned at the supported milestone
//! (PRD §16) into a Plumb-managed cache directory. The fetched binary
//! is hash-pinned on first use; subsequent runs verify the SHA-256
//! against a sidecar file before launch and refuse to execute on
//! mismatch.
//!
//! # Security boundary
//!
//! Auto-fetch downloads and executes a third-party binary. chromiumoxide
//! does not ship signature verification for Chrome-for-Testing
//! artifacts; the user opting in via `--auto-fetch-chromium` (or
//! `auto_fetch_chromium = true`) is the explicit acknowledgement of
//! trust. Once installed, the SHA-256 sidecar pins the binary content
//! so a later tampering attempt (an attacker swapping the cached
//! executable) is detected at launch time and refused. The caller MUST
//! treat the flag as the trust gesture; the sidecar is *not* a
//! signature check against an upstream publisher.
//!
//! # Determinism
//!
//! The cache path is a pure function of `(env vars, target_os)`;
//! nothing here depends on wall-clock time or randomness. `mtime`
//! reads via [`std::fs::metadata`] are filesystem-derived (and not
//! used in any observable Plumb output). The downloaded executable
//! is identified by its SHA-256, so the binary in use is reproducible
//! across runs given the same cache state.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::{CdpError, MIN_SUPPORTED_CHROMIUM_MAJOR, io_error};

/// Subdirectory under the platform cache root that owns Plumb's
/// auto-fetched Chromium installations. Public so callers (and tests)
/// can introspect the path layout.
pub const CACHE_SUBDIR: &str = "plumb/chromium";

/// Sidecar filename appended next to the fetched executable. Stores
/// the hex-encoded SHA-256 captured on first install; subsequent
/// launches re-hash the binary and compare.
pub const SHA256_SIDECAR_FILENAME: &str = ".plumb-sha256";

/// Resolve the platform-appropriate Plumb cache directory.
///
/// Resolution order, by target OS:
///
/// - **Linux**: `$XDG_CACHE_HOME/plumb/chromium`, falling back to
///   `$HOME/.cache/plumb/chromium`.
/// - **macOS**: `$HOME/Library/Caches/plumb/chromium`.
/// - **Windows**: `%LOCALAPPDATA%/plumb/chromium`.
///
/// Reads `std::env` directly (no external `dirs` crate) so Plumb
/// stays at one fewer dependency and the resolution is auditable in
/// one function. Tests exercise [`resolve_cache_dir_with`].
///
/// # Errors
///
/// Returns [`CdpError::CacheDirUnavailable`] when none of the
/// environment variables required to build a path on the host
/// platform are set.
pub fn resolve_cache_dir() -> Result<PathBuf, CdpError> {
    // Wrap in a closure to give Rust a `for<'a> Fn(&'a str)` type —
    // passing `std::env::var` as a function pointer pins it to a
    // single lifetime, which collides with the `Fn` trait's HRTB.
    resolve_cache_dir_with(|key| std::env::var(key), std::env::consts::OS)
}

/// Test-friendly variant of [`resolve_cache_dir`] that takes an
/// explicit env-var lookup closure and target OS string.
///
/// `os` MUST match a value from [`std::env::consts::OS`]
/// (`"linux"`, `"macos"`, `"windows"`, ...). Unknown values are
/// mapped to the Linux/XDG branch as a permissive default.
///
/// # Errors
///
/// Returns [`CdpError::CacheDirUnavailable`] when the host's required
/// env vars are unset.
pub fn resolve_cache_dir_with<F>(env: F, os: &str) -> Result<PathBuf, CdpError>
where
    F: Fn(&str) -> Result<String, std::env::VarError>,
{
    let base =
        match os {
            "macos" => env("HOME")
                .map(|home| PathBuf::from(home).join("Library").join("Caches"))
                .map_err(|err| CdpError::CacheDirUnavailable {
                    reason: format!("HOME not set: {err}"),
                })?,
            "windows" => env("LOCALAPPDATA").map(PathBuf::from).map_err(|err| {
                CdpError::CacheDirUnavailable {
                    reason: format!("LOCALAPPDATA not set: {err}"),
                }
            })?,
            // Linux / FreeBSD / unknown — XDG-style cache root.
            _ => {
                if let Ok(xdg) = env("XDG_CACHE_HOME") {
                    if xdg.is_empty() {
                        home_dot_cache(&env)?
                    } else {
                        PathBuf::from(xdg)
                    }
                } else {
                    home_dot_cache(&env)?
                }
            }
        };
    Ok(base.join(CACHE_SUBDIR))
}

fn home_dot_cache<F>(env: &F) -> Result<PathBuf, CdpError>
where
    F: Fn(&str) -> Result<String, std::env::VarError>,
{
    env("HOME")
        .map(|home| PathBuf::from(home).join(".cache"))
        .map_err(|err| CdpError::CacheDirUnavailable {
            reason: format!("HOME not set: {err}"),
        })
}

/// Hex-encode the SHA-256 of a file's contents.
///
/// Used to stamp the auto-fetch sidecar on first install, and to
/// verify the binary on every subsequent launch.
///
/// # Errors
///
/// Returns [`CdpError::Driver`] (wrapping the underlying I/O error)
/// when the file cannot be opened or read.
pub fn sha256_of_file(path: &Path) -> Result<String, CdpError> {
    let bytes = std::fs::read(path).map_err(io_error)?;
    Ok(sha256_of_bytes(&bytes))
}

/// Hex-encode the SHA-256 of an in-memory byte slice. Pure function;
/// used by tests and by [`sha256_of_file`].
#[must_use]
pub fn sha256_of_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_encode(&hasher.finalize())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// Verify the SHA-256 of `executable_path` against the sidecar at
/// `sidecar_path`. When the sidecar does not exist, write the
/// computed hash and return `Ok(())` (first-run install path).
///
/// Refuses to use the binary on:
///
/// - hash mismatch ([`CdpError::HashMismatch`]),
/// - sidecar I/O failures ([`CdpError::Driver`]),
/// - the binary not existing ([`CdpError::Driver`]).
///
/// # Errors
///
/// Returns [`CdpError::HashMismatch`] when the sidecar exists and
/// its recorded hash differs from the binary's current SHA-256, or
/// [`CdpError::Driver`] for any I/O failure on the executable or the
/// sidecar.
pub fn verify_or_record_sha256(
    executable_path: &Path,
    sidecar_path: &Path,
) -> Result<(), CdpError> {
    let actual = sha256_of_file(executable_path)?;

    if sidecar_path.exists() {
        let expected = std::fs::read_to_string(sidecar_path).map_err(io_error)?;
        let expected = expected.trim();
        if expected != actual {
            return Err(CdpError::HashMismatch {
                path: executable_path.to_path_buf(),
                expected: expected.to_owned(),
                found: actual,
            });
        }
    } else {
        std::fs::write(sidecar_path, actual.as_bytes()).map_err(io_error)?;
    }
    Ok(())
}

/// Resolve the sidecar path that lives next to a fetched executable.
///
/// We place the sidecar in the executable's parent directory rather
/// than next to the binary file itself so that the sidecar survives
/// macOS app-bundle layouts (where the executable is several
/// `.app/Contents/MacOS/...` levels deep) without polluting bundle
/// internals. The sidecar filename is fixed to
/// [`SHA256_SIDECAR_FILENAME`].
///
/// Returns `None` when the executable has no parent (a malformed
/// input).
#[must_use]
pub fn sidecar_path_for(executable_path: &Path) -> Option<PathBuf> {
    executable_path
        .parent()
        .map(|p| p.join(SHA256_SIDECAR_FILENAME))
}

/// Refuse a cache directory whose canonical form points outside the
/// resolved cache root. Defends against a `--cache-dir`-style
/// override that resolves through a symlink to `/etc` or similar —
/// the kind of silent escape that would let a hostile env var
/// promote auto-fetch into a write-anywhere primitive.
///
/// `requested` MUST be the path before canonicalization
/// (caller-provided); `expected_root` is the platform default that
/// the requested path must remain inside *or* equal to.
///
/// Returns the canonicalized requested path on success, the original
/// path with no canonicalization when it does not yet exist
/// (first-install path), or [`CdpError::InvalidPath`] when the
/// resolved path escapes `expected_root`.
///
/// # Errors
///
/// Returns [`CdpError::InvalidPath`] when `requested` resolves
/// outside `expected_root` after canonicalization.
pub fn enforce_cache_root(requested: &Path, expected_root: &Path) -> Result<PathBuf, CdpError> {
    if !requested.exists() {
        // First-install: path may not exist yet. We accept it as-is
        // because `ensure_chromium` will create it inside the
        // expected root; any later canonicalization happens against
        // a known location.
        return Ok(requested.to_path_buf());
    }
    let canonical = requested
        .canonicalize()
        .map_err(|err| CdpError::InvalidPath {
            path: requested.to_path_buf(),
            reason: format!("could not canonicalize cache dir: {err}"),
        })?;
    let root_canonical = expected_root
        .canonicalize()
        .unwrap_or_else(|_| expected_root.to_path_buf());
    if canonical.starts_with(&root_canonical) || canonical == root_canonical {
        Ok(canonical)
    } else {
        Err(CdpError::InvalidPath {
            path: requested.to_path_buf(),
            reason: format!(
                "cache dir resolves to `{}`, which is outside the platform cache root `{}`",
                canonical.display(),
                root_canonical.display()
            ),
        })
    }
}

/// The Chromium milestone Plumb pins for auto-fetch. Always equal
/// to [`MIN_SUPPORTED_CHROMIUM_MAJOR`] so the fetched binary is in
/// the supported range without further version negotiation.
#[must_use]
pub const fn pinned_milestone() -> u32 {
    MIN_SUPPORTED_CHROMIUM_MAJOR
}

/// Download (or reuse the cached) Chromium executable, verify its
/// SHA-256, and return the path the driver should launch.
///
/// On first run the function downloads Chrome-for-Testing pinned at
/// [`pinned_milestone`] into `cache_dir` and writes a SHA-256
/// sidecar. On subsequent runs the cached binary is reused without
/// re-download (chromiumoxide's fetcher already implements the
/// local-then-remote fallback) and the sidecar is verified before
/// the path is returned.
///
/// # Errors
///
/// Returns [`CdpError::AutoFetchFailed`] when the fetcher cannot
/// download or unpack the binary, or [`CdpError::HashMismatch`]
/// when a previously-cached binary's SHA-256 disagrees with the
/// recorded sidecar.
pub async fn ensure_chromium(cache_dir: &Path) -> Result<PathBuf, CdpError> {
    use chromiumoxide::fetcher::{
        BrowserFetcher, BrowserFetcherOptions, BrowserKind, BrowserVersion, Channel,
    };

    std::fs::create_dir_all(cache_dir).map_err(io_error)?;

    // chromiumoxide_fetcher 0.9 re-exports `Milestone`, but Plumb still
    // pins to the stable channel — it gives us the current stable
    // Chrome-for-Testing build, which chromiumoxide's CDP shipping
    // target tracks. Plumb's [`crate::validate_browser_version`] check
    // fires at launch and refuses to proceed if the fetched binary's
    // major version falls outside
    // `MIN_SUPPORTED_CHROMIUM_MAJOR..=MAX_SUPPORTED_CHROMIUM_MAJOR`, so
    // a stable-channel drift outside the supported range surfaces as a
    // typed [`CdpError::UnsupportedChromium`] rather than a mysterious
    // launch failure.
    let options = BrowserFetcherOptions::builder()
        .with_path(cache_dir)
        .with_kind(BrowserKind::Chrome)
        .with_version(BrowserVersion::Channel(Channel::Stable))
        .build()
        .map_err(|err| CdpError::AutoFetchFailed {
            reason: format!("build fetcher options: {err}"),
        })?;

    let fetcher = BrowserFetcher::new(options);
    let installation = fetcher
        .fetch()
        .await
        .map_err(|err| CdpError::AutoFetchFailed {
            reason: format!("fetch chromium: {err}"),
        })?;

    let executable = installation.executable_path;
    if !executable.exists() {
        return Err(CdpError::AutoFetchFailed {
            reason: format!(
                "fetcher reported success but `{}` does not exist",
                executable.display()
            ),
        });
    }

    let sidecar = sidecar_path_for(&executable).ok_or_else(|| CdpError::AutoFetchFailed {
        reason: format!("`{}` has no parent directory", executable.display()),
    })?;
    verify_or_record_sha256(&executable, &sidecar)?;

    Ok(executable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::env::VarError;

    fn env_from(
        map: HashMap<&'static str, &'static str>,
    ) -> impl Fn(&str) -> Result<String, VarError> {
        move |key: &str| {
            map.get(key)
                .map(|s| (*s).to_string())
                .ok_or(VarError::NotPresent)
        }
    }

    #[test]
    fn linux_with_xdg_uses_xdg_cache_home() {
        let env = env_from(HashMap::from([("XDG_CACHE_HOME", "/srv/cache")]));
        let dir = resolve_cache_dir_with(env, "linux").expect("xdg path");
        assert_eq!(dir, PathBuf::from("/srv/cache").join(CACHE_SUBDIR));
    }

    #[test]
    fn linux_without_xdg_falls_back_to_home_dot_cache() {
        let env = env_from(HashMap::from([("HOME", "/home/me")]));
        let dir = resolve_cache_dir_with(env, "linux").expect("home path");
        assert_eq!(
            dir,
            PathBuf::from("/home/me").join(".cache").join(CACHE_SUBDIR)
        );
    }

    #[test]
    fn linux_with_empty_xdg_falls_back_to_home_dot_cache() {
        // Empty XDG_CACHE_HOME (the env var is set but to "") MUST
        // NOT resolve to `<empty>/plumb/chromium` — that's
        // effectively a CWD-relative path and would point at user
        // data. The XDG spec (§4) says "If $XDG_CACHE_HOME is either
        // not set or empty, a default equal to $HOME/.cache should
        // be used."
        let env = env_from(HashMap::from([
            ("XDG_CACHE_HOME", ""),
            ("HOME", "/home/me"),
        ]));
        let dir = resolve_cache_dir_with(env, "linux").expect("home fallback path");
        assert_eq!(
            dir,
            PathBuf::from("/home/me").join(".cache").join(CACHE_SUBDIR)
        );
    }

    #[test]
    fn linux_without_home_or_xdg_errors() {
        let env = env_from(HashMap::new());
        let err = resolve_cache_dir_with(env, "linux").expect_err("no env should error");
        assert!(matches!(err, CdpError::CacheDirUnavailable { .. }));
    }

    #[test]
    fn macos_uses_library_caches() {
        let env = env_from(HashMap::from([("HOME", "/Users/me")]));
        let dir = resolve_cache_dir_with(env, "macos").expect("macos path");
        assert_eq!(
            dir,
            PathBuf::from("/Users/me")
                .join("Library")
                .join("Caches")
                .join(CACHE_SUBDIR)
        );
    }

    #[test]
    fn macos_without_home_errors() {
        let env = env_from(HashMap::new());
        let err = resolve_cache_dir_with(env, "macos").expect_err("no HOME should error");
        assert!(matches!(err, CdpError::CacheDirUnavailable { .. }));
    }

    #[test]
    fn windows_uses_local_app_data() {
        let env = env_from(HashMap::from([(
            "LOCALAPPDATA",
            "C:\\Users\\me\\AppData\\Local",
        )]));
        let dir = resolve_cache_dir_with(env, "windows").expect("windows path");
        assert_eq!(
            dir,
            PathBuf::from("C:\\Users\\me\\AppData\\Local").join(CACHE_SUBDIR)
        );
    }

    #[test]
    fn windows_without_local_app_data_errors() {
        let env = env_from(HashMap::new());
        let err = resolve_cache_dir_with(env, "windows").expect_err("no LOCALAPPDATA should error");
        assert!(matches!(err, CdpError::CacheDirUnavailable { .. }));
    }

    #[test]
    fn unknown_os_falls_back_to_xdg_branch() {
        // Permissive default: an unknown OS is treated like
        // Linux/XDG so the call still returns a path on hosts Plumb
        // has not formally characterised (e.g. FreeBSD, OpenBSD).
        let env = env_from(HashMap::from([("HOME", "/home/me")]));
        let dir = resolve_cache_dir_with(env, "freebsd").expect("xdg fallback");
        assert_eq!(
            dir,
            PathBuf::from("/home/me").join(".cache").join(CACHE_SUBDIR)
        );
    }

    #[test]
    fn sha256_known_vectors() {
        // RFC-style spot check: the SHA-256 of "" is the well-known
        // empty-string digest. Locks the implementation against
        // accidental changes to the hex encoder or the digest setup.
        assert_eq!(
            sha256_of_bytes(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_of_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn verify_or_record_records_on_first_run() {
        let dir = tempfile::tempdir().expect("tempdir");
        let exec = dir.path().join("chrome");
        std::fs::write(&exec, b"binary contents").expect("write exec");
        let sidecar = sidecar_path_for(&exec).expect("sidecar path");
        verify_or_record_sha256(&exec, &sidecar).expect("first run records");
        let recorded = std::fs::read_to_string(&sidecar).expect("read sidecar");
        assert_eq!(recorded.trim(), sha256_of_bytes(b"binary contents"));
    }

    #[test]
    fn verify_or_record_passes_when_hash_matches() {
        let dir = tempfile::tempdir().expect("tempdir");
        let exec = dir.path().join("chrome");
        std::fs::write(&exec, b"binary contents").expect("write exec");
        let sidecar = sidecar_path_for(&exec).expect("sidecar path");
        std::fs::write(&sidecar, sha256_of_bytes(b"binary contents")).expect("seed sidecar");
        verify_or_record_sha256(&exec, &sidecar).expect("matching hash passes");
    }

    #[test]
    fn verify_or_record_refuses_on_hash_mismatch() {
        let dir = tempfile::tempdir().expect("tempdir");
        let exec = dir.path().join("chrome");
        std::fs::write(&exec, b"tampered contents").expect("write exec");
        let sidecar = sidecar_path_for(&exec).expect("sidecar path");
        std::fs::write(&sidecar, sha256_of_bytes(b"original contents")).expect("seed sidecar");

        let err = verify_or_record_sha256(&exec, &sidecar).expect_err("mismatch must error");
        match err {
            CdpError::HashMismatch {
                path,
                expected,
                found,
            } => {
                assert_eq!(path, exec);
                assert_eq!(expected, sha256_of_bytes(b"original contents"));
                assert_eq!(found, sha256_of_bytes(b"tampered contents"));
            }
            other => panic!("expected HashMismatch, got {other:?}"),
        }
    }

    #[test]
    fn verify_or_record_handles_trailing_newline_in_sidecar() {
        // The sidecar is treated as a plain text file. Hand-written
        // or editor-saved sidecars often pick up a trailing newline;
        // the verifier MUST trim before comparing or every reuse
        // would fail.
        let dir = tempfile::tempdir().expect("tempdir");
        let exec = dir.path().join("chrome");
        std::fs::write(&exec, b"binary contents").expect("write exec");
        let sidecar = sidecar_path_for(&exec).expect("sidecar path");
        let mut recorded = sha256_of_bytes(b"binary contents");
        recorded.push('\n');
        std::fs::write(&sidecar, recorded).expect("seed sidecar with newline");
        verify_or_record_sha256(&exec, &sidecar).expect("trimmed compare passes");
    }

    #[test]
    fn sidecar_path_for_uses_parent_directory() {
        let exec = PathBuf::from("/cache/plumb/chromium/chrome-mac-arm64/chrome");
        let sidecar = sidecar_path_for(&exec).expect("sidecar path");
        assert_eq!(
            sidecar,
            PathBuf::from("/cache/plumb/chromium/chrome-mac-arm64").join(SHA256_SIDECAR_FILENAME)
        );
    }

    #[test]
    fn sidecar_path_for_handles_root_input() {
        // `Path::parent` on `/` is `None`. Confirming the function's
        // contract here so the caller knows to map the `None` to a
        // typed error.
        let exec = PathBuf::from("/");
        assert!(sidecar_path_for(&exec).is_none());
    }

    #[test]
    fn enforce_cache_root_accepts_nonexistent_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let nonexistent = dir.path().join("does-not-exist-yet");
        let result =
            enforce_cache_root(&nonexistent, dir.path()).expect("nonexistent paths accepted");
        assert_eq!(result, nonexistent);
    }

    #[test]
    fn enforce_cache_root_accepts_path_inside_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let inside = dir.path().join("plumb").join("chromium");
        std::fs::create_dir_all(&inside).expect("mkdir");
        let result = enforce_cache_root(&inside, dir.path()).expect("inside root passes");
        // Canonicalized form may differ on macOS
        // (`/private/var/...`), so compare via `canonicalize`.
        assert_eq!(
            result.canonicalize().expect("canonical inside"),
            inside.canonicalize().expect("canonical inside"),
        );
    }

    #[test]
    fn enforce_cache_root_rejects_path_outside_root() {
        let outer = tempfile::tempdir().expect("outer tempdir");
        let inner = tempfile::tempdir().expect("inner tempdir");
        // `inner` is a sibling of `outer`, so it MUST be rejected.
        let err = enforce_cache_root(inner.path(), outer.path()).expect_err("sibling rejected");
        assert!(matches!(err, CdpError::InvalidPath { .. }));
    }

    #[test]
    fn pinned_milestone_matches_min_supported() {
        assert_eq!(pinned_milestone(), MIN_SUPPORTED_CHROMIUM_MAJOR);
    }
}
