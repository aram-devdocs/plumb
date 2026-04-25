//! mtime-based cache for resolved Tailwind themes.
//!
//! ## Determinism
//!
//! The cache is a performance optimization, not a correctness contract.
//! Reads return the cached `theme` value byte-for-byte. Writes happen
//! only after a successful Node spawn produced a valid theme. The
//! decision tree is:
//!
//! 1. Stat the user's Tailwind config file → `mtime_unix_ms`.
//! 2. Hash the absolute config path with SHA-256 → cache filename.
//! 3. If `<cache_dir>/<hash>.json` exists and its `mtime_unix_ms`
//!    matches, deserialize and return. Otherwise the caller spawns Node.
//!
//! Mismatched or unreadable cache entries are treated as a miss; we
//! never error out of a corrupted cache.
//!
//! ## No env access
//!
//! This module never reads `TMPDIR` / `TEMP` / `TMP` or any other
//! process-global state. The caller passes the cache directory in
//! explicitly. When no directory is supplied, the caller treats it as
//! "cache disabled" and the functions in this module are not invoked.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTimeError;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Wire format for the cache file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct CacheEntry {
    /// Modification time of the source config file in milliseconds since
    /// the Unix epoch. This is the cache key beyond the filename hash.
    pub(super) mtime_unix_ms: u128,
    /// The resolved theme JSON object. Stored as `serde_json::Value`
    /// so the cache file is self-describing — no schema migrations
    /// required when Plumb adds new theme keys.
    pub(super) theme: serde_json::Value,
}

/// SHA-256 hex digest of the absolute config path. Stable across runs.
pub(super) fn config_path_hash(path: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.as_os_str().as_encoded_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        // hex-encode without an extra dependency.
        const TABLE: &[u8; 16] = b"0123456789abcdef";
        let upper = TABLE[(byte >> 4) as usize];
        let lower = TABLE[(byte & 0x0f) as usize];
        hex.push(char::from(upper));
        hex.push(char::from(lower));
    }
    hex
}

/// Compute the absolute path of the cache entry for a given config.
pub(super) fn cache_path_for(config_path: &Path, dir: &Path) -> PathBuf {
    let hash = config_path_hash(config_path);
    dir.join(format!("{hash}.json"))
}

/// Read the file's mtime as milliseconds since the Unix epoch.
pub(super) fn mtime_unix_ms(path: &Path) -> Result<u128, MtimeError> {
    let meta = fs::metadata(path).map_err(MtimeError::Io)?;
    let modified = meta.modified().map_err(MtimeError::Io)?;
    let dur = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(MtimeError::SystemTime)?;
    Ok(dur.as_millis())
}

/// Failure modes when reading a file's mtime.
#[derive(Debug)]
pub(super) enum MtimeError {
    /// Underlying I/O error (file missing, permission denied, etc.).
    Io(io::Error),
    /// File mtime predates the Unix epoch — shouldn't happen on any
    /// modern filesystem but we surface it instead of panicking.
    SystemTime(SystemTimeError),
}

impl std::fmt::Display for MtimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::SystemTime(err) => write!(f, "{err}"),
        }
    }
}

/// Look up a cached theme for `config_path`. Returns `None` on any cache
/// miss (file missing, JSON malformed, mtime mismatch). Never errors;
/// cache problems are silent.
pub(super) fn read(config_path: &Path, dir: &Path) -> Option<CacheEntry> {
    let mtime = mtime_unix_ms(config_path).ok()?;
    let cache_path = cache_path_for(config_path, dir);
    let bytes = fs::read(&cache_path).ok()?;
    let entry: CacheEntry = serde_json::from_slice(&bytes).ok()?;
    if entry.mtime_unix_ms == mtime {
        Some(entry)
    } else {
        None
    }
}

/// Persist a resolved theme to the cache. Best-effort — failures are
/// logged at `debug` level and otherwise swallowed; the caller already
/// has a valid theme, so a missing cache entry just costs a re-spawn
/// next time.
pub(super) fn write(
    config_path: &Path,
    theme: &serde_json::Value,
    dir: &Path,
) -> Result<(), io::Error> {
    let mtime = mtime_unix_ms(config_path).map_err(|err| match err {
        MtimeError::Io(io) => io,
        MtimeError::SystemTime(_) => io::Error::other("config mtime predates the Unix epoch"),
    })?;
    let entry = CacheEntry {
        mtime_unix_ms: mtime,
        theme: theme.clone(),
    };
    let cache_path = cache_path_for(config_path, dir);
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_vec(&entry).map_err(io::Error::other)?;
    // Write to a sibling temp file then rename, so a partial write is
    // never observable. The OS rename is atomic on POSIX and Windows
    // (`MoveFileEx` with `MOVEFILE_REPLACE_EXISTING` semantics under
    // `fs::rename` on stable Rust).
    let tmp_path = cache_path.with_extension("json.tmp");
    fs::write(&tmp_path, &serialized)?;
    fs::rename(&tmp_path, &cache_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_path_hash_is_stable_and_64_chars() {
        let path = Path::new("/tmp/example/tailwind.config.js");
        let h1 = config_path_hash(path);
        let h2 = config_path_hash(path);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
        assert!(
            h1.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    #[test]
    fn config_path_hash_distinguishes_paths() {
        let a = config_path_hash(Path::new("/a/tailwind.config.js"));
        let b = config_path_hash(Path::new("/b/tailwind.config.js"));
        assert_ne!(a, b);
    }

    #[test]
    fn cache_round_trip_through_temp_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg_path = dir.path().join("tailwind.config.js");
        std::fs::write(&cfg_path, "module.exports = {};").expect("write config");

        let theme = serde_json::json!({"colors": {"red": "#ff0000"}});
        write(&cfg_path, &theme, dir.path()).expect("write cache");

        let entry = read(&cfg_path, dir.path()).expect("hit cache");
        assert_eq!(entry.theme, theme);
    }

    #[test]
    fn cache_miss_when_mtime_changes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg_path = dir.path().join("tailwind.config.js");
        std::fs::write(&cfg_path, "module.exports = {};").expect("write config");
        let theme = serde_json::json!({"colors": {"red": "#ff0000"}});
        write(&cfg_path, &theme, dir.path()).expect("write cache");

        // Bump mtime by rewriting the config file with later content.
        // sleep-free: set mtime explicitly via filetime if available;
        // the simpler path is to rewrite and accept that filesystem
        // resolution is millisecond-or-coarser — write a guaranteed
        // distinct mtime by using `set_modified` from std.
        let later = std::time::UNIX_EPOCH + std::time::Duration::from_secs(2_000_000_000);
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&cfg_path)
            .expect("open");
        file.set_modified(later).expect("set mtime");
        drop(file);

        assert!(
            read(&cfg_path, dir.path()).is_none(),
            "mtime change should invalidate cache"
        );
    }
}
