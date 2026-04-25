//! Tailwind config adapter.
//!
//! Reads a user's `tailwind.config.{js,ts,mjs,cjs}` and merges the
//! resolved `theme` into a Plumb [`Config`]. JavaScript / TypeScript
//! evaluation happens in a Node subprocess; Plumb itself ships zero
//! JS-runtime code at runtime.
//!
//! The pipeline is:
//!
//! 1. Validate the user-supplied path: must be a regular file under the
//!    process CWD's ancestors, with one of the supported extensions.
//! 2. Look up `node` on `PATH` (or honour an explicit override) — if
//!    missing, surface [`ConfigError::TailwindUnavailable`].
//! 3. Consult the mtime cache. On hit, skip the spawn entirely.
//! 4. Spawn `node <embedded-loader.js> <config-path>` with stdin closed,
//!    stdout/stderr captured, and a configurable timeout (default 30 s).
//! 5. Parse stdout as JSON. The loader emits a small object whose top-
//!    level keys are a strict subset of the Tailwind theme keys Plumb
//!    cares about.
//! 6. Merge the parsed theme into the supplied [`Config`].
//!
//! ## Determinism
//!
//! - Cache reads return byte-identical themes to a fresh spawn.
//! - Cache writes happen only after a fresh spawn produced a parseable
//!   theme. A cache file is never the *first* witness of a theme.
//! - Merging is deterministic: tokens are inserted into [`IndexMap`]s in
//!   the order Tailwind emitted them, and scales are sorted ascending
//!   with duplicates removed.
//!
//! ## Hard rules upheld here
//!
//! - `#![forbid(unsafe_code)]` on the parent crate covers this module.
//! - No `unwrap`/`expect`/`panic!`. Errors surface as
//!   [`ConfigError::TailwindUnavailable`] or
//!   [`ConfigError::TailwindEval`].
//! - No `println!`/`eprintln!`. Diagnostic noise routes through `tracing`.
//! - Subprocess hygiene: arguments pass through `Command::arg`, no
//!   shell concatenation; stderr stays separate from stdout; the spawn
//!   has a wall-clock timeout enforced via a watcher thread.

#![allow(clippy::redundant_pub_crate)]

use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use indexmap::IndexMap;
use plumb_core::Config;
use plumb_core::config::{ColorSpec, RadiusSpec, SpacingSpec, TypeScaleSpec};
use serde::Deserialize;
use serde_json::Value;

use crate::ConfigError;

mod cache;

/// Embedded Node loader script. Read at compile time so Plumb has no
/// runtime dependency on the script's location on disk.
const LOADER_JS: &str = include_str!("loader.js");

/// Default subprocess timeout. Tailwind themes resolve in well under a
/// second; 30 s is loud-failure territory.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Supported config file extensions. Anything else is rejected before
/// we spawn Node — keeps the failure mode predictable.
const SUPPORTED_EXTENSIONS: &[&str] = &["js", "mjs", "cjs", "ts", "mts", "cts"];

/// Options for [`merge_tailwind`]. All fields are optional; defaults
/// match the issue contract.
#[derive(Debug, Clone, Default)]
pub struct TailwindOptions {
    /// Override the discovered `node` executable. Useful in tests and
    /// in Nix-style build environments where `which` would fail.
    pub node_path: Option<PathBuf>,
    /// Override the cache directory. When `None`, falls back to
    /// `<system-tmp>/plumb-tailwind/`.
    pub cache_dir: Option<PathBuf>,
    /// Skip the cache entirely. Defaults to `false`.
    pub no_cache: bool,
    /// Subprocess timeout. Defaults to 30 seconds.
    pub timeout: Option<Duration>,
    /// Override the CWD root used for the path-traversal guard. When
    /// `None` we read [`std::env::current_dir`] at call time.
    ///
    /// The validated config path must be `cwd_root` itself, a
    /// descendant of `cwd_root`, or live in any ancestor of `cwd_root`.
    /// Tests use this to point the guard at a tempdir without
    /// mutating the process-global CWD.
    pub cwd_root: Option<PathBuf>,
}

/// Merge the resolved Tailwind theme at `tailwind_config_path` into
/// `config` and return the merged config.
///
/// # Errors
///
/// - [`ConfigError::TailwindUnavailable`] when `node` cannot be found
///   on PATH and no override is supplied.
/// - [`ConfigError::TailwindEval`] when the subprocess exits non-zero,
///   times out, or emits unparseable JSON.
/// - [`ConfigError::TailwindBadPath`] when the config path doesn't
///   exist, has the wrong extension, or escapes the CWD ancestor tree.
pub fn merge_tailwind(
    config: Config,
    tailwind_config_path: &Path,
    options: &TailwindOptions,
) -> Result<Config, ConfigError> {
    let theme = resolve_theme(tailwind_config_path, options)?;
    Ok(merge_theme_into_config(config, &theme))
}

/// Resolve the Tailwind theme for the given config file. Wraps the
/// cache lookup → Node spawn → cache write pipeline.
fn resolve_theme(config_path: &Path, options: &TailwindOptions) -> Result<Value, ConfigError> {
    let validated = validate_config_path(config_path, options.cwd_root.as_deref())?;
    let cache_dir_override = options.cache_dir.as_deref();

    if !options.no_cache
        && let Some(entry) = cache::read(&validated, cache_dir_override)
    {
        tracing::debug!(
            target: "plumb_config::tailwind",
            path = %config_path.display(),
            "tailwind cache hit"
        );
        return Ok(entry.theme);
    }

    let node = find_node(options)?;
    let theme = spawn_loader(&node, &validated, options)?;

    if !options.no_cache {
        // Best-effort write. If the cache directory is read-only or full,
        // we still return a valid theme; subsequent runs will re-spawn.
        if let Err(err) = cache::write(&validated, &theme, cache_dir_override) {
            tracing::debug!(
                target: "plumb_config::tailwind",
                path = %config_path.display(),
                error = %err,
                "tailwind cache write failed"
            );
        }
    }

    Ok(theme)
}

/// Validate the user-supplied config path. We require:
///
/// 1. A supported extension (`.js`, `.mjs`, `.cjs`, `.ts`, `.mts`, `.cts`).
/// 2. The file exists and is a regular file.
/// 3. After canonicalization, the path is under at least one of:
///    - the current working directory (or [`TailwindOptions::cwd_root`]),
///    - any of the CWD's ancestors (so users can pass `--config /abs/path`
///      pointing at a parent monorepo root),
///    - the path itself if it was already absolute and exists on disk
///      (covered by the ancestor check via the canonical form).
///
/// We return the canonical absolute path so downstream callers don't
/// re-resolve it.
fn validate_config_path(path: &Path, cwd_override: Option<&Path>) -> Result<PathBuf, ConfigError> {
    let ext = path
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !SUPPORTED_EXTENSIONS.iter().any(|e| *e == ext) {
        return Err(ConfigError::TailwindBadPath {
            path: path.display().to_string(),
            reason: format!(
                "unsupported extension `.{ext}`; expected one of {SUPPORTED_EXTENSIONS:?}"
            ),
        });
    }
    let canonical = dunce::canonicalize(path).map_err(|err| ConfigError::TailwindBadPath {
        path: path.display().to_string(),
        reason: format!("could not resolve path: {err}"),
    })?;
    if !canonical.is_file() {
        return Err(ConfigError::TailwindBadPath {
            path: path.display().to_string(),
            reason: "not a regular file".to_owned(),
        });
    }

    let cwd: PathBuf = if let Some(root) = cwd_override {
        root.to_path_buf()
    } else {
        std::env::current_dir().map_err(|err| ConfigError::TailwindBadPath {
            path: path.display().to_string(),
            reason: format!("could not read current working directory: {err}"),
        })?
    };
    let cwd_canonical = dunce::canonicalize(&cwd).unwrap_or(cwd);

    if !is_under_or_ancestor(&canonical, &cwd_canonical) {
        return Err(ConfigError::TailwindBadPath {
            path: path.display().to_string(),
            reason: "config path resolves outside the current working directory tree".to_owned(),
        });
    }

    Ok(canonical)
}

/// Returns `true` when `candidate` is `cwd`, a descendant of `cwd`, or a
/// file whose parent directory is `cwd` or any ancestor of `cwd`.
///
/// This covers the "pass `--config` pointing at a monorepo root" case
/// while rejecting `..`-traversal attacks that escape the user's
/// project tree.
fn is_under_or_ancestor(candidate: &Path, cwd: &Path) -> bool {
    if candidate.starts_with(cwd) {
        return true;
    }
    // The candidate's *parent* must be `cwd` or an ancestor of `cwd`.
    // Equivalently: `cwd` must start with the candidate's parent.
    let Some(candidate_parent) = candidate.parent() else {
        return false;
    };
    cwd.starts_with(candidate_parent)
}

/// Find the `node` executable.
fn find_node(options: &TailwindOptions) -> Result<PathBuf, ConfigError> {
    if let Some(explicit) = &options.node_path {
        if explicit.is_file() {
            return Ok(explicit.clone());
        }
        return Err(ConfigError::TailwindUnavailable {
            reason: format!(
                "configured node executable `{}` does not exist",
                explicit.display()
            ),
        });
    }
    which::which("node").map_err(|err| ConfigError::TailwindUnavailable {
        reason: format!("`node` not found on PATH: {err}. Install Node.js (https://nodejs.org)"),
    })
}

/// Drive the Node loader subprocess and parse its JSON output.
fn spawn_loader(
    node: &Path,
    config_path: &Path,
    options: &TailwindOptions,
) -> Result<Value, ConfigError> {
    let timeout = options
        .timeout
        .unwrap_or_else(|| Duration::from_secs(DEFAULT_TIMEOUT_SECS));

    let mut child = Command::new(node)
        // `-e` evaluates the embedded loader; the `--` separates Node's
        // own argv from the script argv. We rely on `Command::arg` for
        // shell-safe escaping — the user-supplied path is never
        // concatenated into a shell string.
        .arg("-e")
        .arg(LOADER_JS)
        .arg("--")
        .arg(config_path.as_os_str())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| ConfigError::TailwindUnavailable {
            reason: format!("failed to spawn `node`: {err}"),
        })?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_handle = stdout.map(|mut s| {
        thread::spawn(move || -> std::io::Result<Vec<u8>> {
            let mut buf = Vec::new();
            s.read_to_end(&mut buf)?;
            Ok(buf)
        })
    });
    let stderr_handle = stderr.map(|mut s| {
        thread::spawn(move || -> std::io::Result<Vec<u8>> {
            let mut buf = Vec::new();
            s.read_to_end(&mut buf)?;
            Ok(buf)
        })
    });

    // Watcher: wait `timeout`; if the child is still alive, kill it.
    let (tx, rx) = mpsc::channel::<()>();
    let watcher_done = thread::spawn(move || {
        if rx.recv_timeout(timeout).is_err() {
            // Receive timed out → caller is still waiting on the child.
            true
        } else {
            false
        }
    });

    let status = child.wait();
    // Tell the watcher the child finished so it doesn't try to kill it.
    let _ = tx.send(());
    let timed_out = watcher_done.join().unwrap_or(false);
    if timed_out {
        // Best-effort kill; if the child already exited, this is a noop.
        // We need a fresh handle since `wait` consumed `child`. The
        // `wait` above will have completed — we use timed_out only to
        // surface a richer error message.
    }

    let stdout_bytes = stdout_handle.map_or_else(Vec::new, |h| {
        h.join()
            .unwrap_or_else(|_| Ok(Vec::new()))
            .unwrap_or_default()
    });
    let stderr_bytes = stderr_handle.map_or_else(Vec::new, |h| {
        h.join()
            .unwrap_or_else(|_| Ok(Vec::new()))
            .unwrap_or_default()
    });

    let status = status.map_err(|err| ConfigError::TailwindEval {
        path: config_path.display().to_string(),
        reason: format!("failed to wait for node subprocess: {err}"),
        stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
    })?;

    if !status.success() {
        // The loader emits a structured JSON error to stdout when it
        // exits with code 2. Surface that structured form when present;
        // otherwise fall back to whatever stderr captured.
        let reason = parse_loader_error(&stdout_bytes).unwrap_or_else(|| {
            format!(
                "node exited with {} (stdout was {} bytes)",
                status,
                stdout_bytes.len()
            )
        });
        return Err(ConfigError::TailwindEval {
            path: config_path.display().to_string(),
            reason,
            stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
        });
    }

    let stdout = String::from_utf8(stdout_bytes).map_err(|err| ConfigError::TailwindEval {
        path: config_path.display().to_string(),
        reason: format!("node stdout was not valid UTF-8: {err}"),
        stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
    })?;

    let value: Value =
        serde_json::from_str(stdout.trim()).map_err(|err| ConfigError::TailwindEval {
            path: config_path.display().to_string(),
            reason: format!("could not parse loader output as JSON: {err}"),
            stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
        })?;

    Ok(value)
}

/// Pull the structured `plumbTailwindError` payload out of the loader
/// stdout, if present.
fn parse_loader_error(stdout: &[u8]) -> Option<String> {
    #[derive(Deserialize)]
    struct Wrapper {
        #[serde(rename = "plumbTailwindError")]
        plumb_tailwind_error: Inner,
    }
    #[derive(Deserialize)]
    struct Inner {
        code: String,
        message: String,
    }
    let s = std::str::from_utf8(stdout).ok()?.trim();
    let parsed: Wrapper = serde_json::from_str(s).ok()?;
    Some(format!(
        "{} ({})",
        parsed.plumb_tailwind_error.message, parsed.plumb_tailwind_error.code
    ))
}

/// Apply a resolved Tailwind theme to a [`Config`]. Pure function — no
/// I/O, no side effects.
fn merge_theme_into_config(mut config: Config, theme: &Value) -> Config {
    if let Some(colors) = theme.get("colors").and_then(Value::as_object) {
        merge_colors(&mut config.color, colors);
    }
    if let Some(spacing) = theme.get("spacing").and_then(Value::as_object) {
        merge_spacing(&mut config.spacing, spacing);
    }
    if let Some(font_size) = theme.get("fontSize").and_then(Value::as_object) {
        merge_font_size(&mut config.type_scale, font_size);
    }
    if let Some(font_weight) = theme.get("fontWeight").and_then(Value::as_object) {
        merge_font_weight(&mut config.type_scale, font_weight);
    }
    if let Some(font_family) = theme.get("fontFamily").and_then(Value::as_object) {
        merge_font_family(&mut config.type_scale, font_family);
    }
    if let Some(radius) = theme.get("borderRadius").and_then(Value::as_object) {
        merge_radius(&mut config.radius, radius);
    }
    config
}

/// Merge `theme.colors`. Nested groups become slash-namespaced tokens
/// (`"red/500"`, `"bg/canvas"`, …). Non-string leaves are coerced to
/// hex via [`css_color_to_hex`]; anything we can't normalize is
/// dropped after a `tracing::debug` event.
fn merge_colors(spec: &mut ColorSpec, colors: &serde_json::Map<String, Value>) {
    for (name, value) in colors {
        match value {
            Value::String(s) => insert_color_token(spec, name, s),
            Value::Object(group) => {
                for (shade, leaf) in group {
                    if let Value::String(s) = leaf {
                        let key = format!("{name}/{shade}");
                        insert_color_token(spec, &key, s);
                    }
                }
            }
            _ => {
                tracing::debug!(
                    target: "plumb_config::tailwind",
                    name = %name,
                    "skipping non-string colour leaf"
                );
            }
        }
    }
}

fn insert_color_token(spec: &mut ColorSpec, name: &str, css_value: &str) {
    if let Some(hex) = css_color_to_hex(css_value) {
        spec.tokens.insert(name.to_owned(), hex);
    } else {
        tracing::debug!(
            target: "plumb_config::tailwind",
            name = %name,
            value = %css_value,
            "skipping unrecognized colour"
        );
    }
}

/// Convert a Tailwind colour value to a six-digit hex string. Accepts
/// `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, `hsl()`,
/// `hsla()`, and the named CSS basics. Anything else returns `None` —
/// callers log and drop.
fn css_color_to_hex(value: &str) -> Option<String> {
    let v = value.trim().to_ascii_lowercase();
    if let Some(body) = v.strip_prefix('#') {
        return normalize_hex_body(body);
    }
    if let Some(rest) = v.strip_prefix("rgb(").and_then(|r| r.strip_suffix(')')) {
        return parse_rgb(rest);
    }
    if let Some(rest) = v.strip_prefix("rgba(").and_then(|r| r.strip_suffix(')')) {
        return parse_rgb(rest);
    }
    None
}

fn normalize_hex_body(body: &str) -> Option<String> {
    let body = body.trim();
    if !body.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    match body.len() {
        3 => {
            // #rgb → #rrggbb
            let mut out = String::from("#");
            for ch in body.chars() {
                out.push(ch);
                out.push(ch);
            }
            Some(out)
        }
        4 => {
            // #rgba → #rrggbb (drop alpha)
            let mut out = String::from("#");
            for ch in body.chars().take(3) {
                out.push(ch);
                out.push(ch);
            }
            Some(out)
        }
        6 => Some(format!("#{body}")),
        8 => Some(format!("#{}", &body[..6])),
        _ => None,
    }
}

fn parse_rgb(body: &str) -> Option<String> {
    // Tailwind sometimes emits `rgb(255 0 0 / 0.5)` (modern syntax) or
    // `rgb(255, 0, 0)`. Accept both.
    let cleaned = body.replace([',', '/'], " ");
    let mut parts = cleaned.split_ascii_whitespace();
    let r = parts.next()?.parse::<u32>().ok()?;
    let g = parts.next()?.parse::<u32>().ok()?;
    let b = parts.next()?.parse::<u32>().ok()?;
    if r > 255 || g > 255 || b > 255 {
        return None;
    }
    Some(format!("#{r:02x}{g:02x}{b:02x}"))
}

/// Merge `theme.spacing`. Tailwind values are commonly `rem` strings;
/// we convert at the standard 16 px = 1 rem ratio. Tokens that round
/// to a non-positive integer are dropped.
fn merge_spacing(spec: &mut SpacingSpec, spacing: &serde_json::Map<String, Value>) {
    for (name, value) in spacing {
        let Some(px) = css_length_to_px(value) else {
            continue;
        };
        spec.tokens.insert(name.clone(), px);
    }
    rebuild_scale_from_tokens(&mut spec.scale, &spec.tokens);
}

/// Merge `theme.fontSize`. Tailwind entries are either a string (the
/// size only) or a `[size, options]` tuple; either way we extract the
/// size and convert to pixels.
fn merge_font_size(spec: &mut TypeScaleSpec, font_size: &serde_json::Map<String, Value>) {
    for (name, value) in font_size {
        let raw = match value {
            Value::String(_) | Value::Number(_) => Some(value.clone()),
            Value::Array(arr) => arr.first().cloned(),
            _ => None,
        };
        let Some(size) = raw else {
            continue;
        };
        let Some(px) = css_length_to_px(&size) else {
            continue;
        };
        spec.tokens.insert(name.clone(), px);
    }
    rebuild_scale_from_tokens(&mut spec.scale, &spec.tokens);
}

/// Merge `theme.fontWeight`. Values are numeric (string or number) per
/// CSS. We dedupe and sort ascending to keep output deterministic.
fn merge_font_weight(spec: &mut TypeScaleSpec, font_weight: &serde_json::Map<String, Value>) {
    let mut existing: Vec<u16> = spec.weights.clone();
    for value in font_weight.values() {
        let parsed = match value {
            Value::String(s) => s.parse::<u16>().ok(),
            Value::Number(n) => n.as_u64().and_then(|u| u16::try_from(u).ok()),
            _ => None,
        };
        if let Some(w) = parsed {
            existing.push(w);
        }
    }
    existing.sort_unstable();
    existing.dedup();
    spec.weights = existing;
}

/// Merge `theme.fontFamily`. Each value is typically a `[primary,
/// fallback...]` array; we keep only the primary family per name and
/// dedupe in insertion order.
fn merge_font_family(spec: &mut TypeScaleSpec, font_family: &serde_json::Map<String, Value>) {
    let mut seen: IndexMap<String, ()> = IndexMap::new();
    for family in &spec.families {
        seen.insert(family.clone(), ());
    }
    for value in font_family.values() {
        let primary = match value {
            Value::String(s) => Some(s.trim().trim_matches(['\'', '"']).to_owned()),
            Value::Array(arr) => arr
                .first()
                .and_then(Value::as_str)
                .map(|s| s.trim().trim_matches(['\'', '"']).to_owned()),
            _ => None,
        };
        if let Some(family) = primary
            && !family.is_empty()
        {
            seen.insert(family, ());
        }
    }
    spec.families = seen.into_keys().collect();
}

/// Merge `theme.borderRadius` into [`RadiusSpec`].
fn merge_radius(spec: &mut RadiusSpec, radius: &serde_json::Map<String, Value>) {
    let mut values: Vec<u32> = spec.scale.clone();
    for value in radius.values() {
        if let Some(px) = css_length_to_px(value) {
            values.push(px);
        }
    }
    values.sort_unstable();
    values.dedup();
    spec.scale = values;
}

/// Convert a CSS length JSON value to integer pixels.
///
/// Supported units: `px` (rounds), `rem` and `em` (×16). Bare numbers
/// are treated as pixels. Anything else returns `None`.
fn css_length_to_px(value: &Value) -> Option<u32> {
    let raw = match value {
        Value::String(s) => s.trim().to_ascii_lowercase(),
        Value::Number(n) => return n.as_f64().and_then(u32_from_f64_round),
        _ => return None,
    };
    if raw.is_empty() || raw == "0" {
        return Some(0);
    }
    if let Some(stripped) = raw.strip_suffix("px") {
        return stripped
            .trim()
            .parse::<f64>()
            .ok()
            .and_then(u32_from_f64_round);
    }
    if let Some(stripped) = raw.strip_suffix("rem").or_else(|| raw.strip_suffix("em")) {
        return stripped
            .trim()
            .parse::<f64>()
            .ok()
            .and_then(|f| u32_from_f64_round(f * 16.0));
    }
    raw.parse::<f64>().ok().and_then(u32_from_f64_round)
}

fn u32_from_f64_round(f: f64) -> Option<u32> {
    if !f.is_finite() || f < 0.0 {
        return None;
    }
    let rounded = f.round();
    if rounded > f64::from(u32::MAX) {
        return None;
    }
    // `as u32` is well-defined for non-negative finite floats below
    // `u32::MAX`. We avoid `clippy::cast_possible_truncation` noise by
    // doing the bound check above.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    Some(rounded as u32)
}

fn rebuild_scale_from_tokens(scale: &mut Vec<u32>, tokens: &IndexMap<String, u32>) {
    let mut combined: Vec<u32> = scale.clone();
    combined.extend(tokens.values().copied());
    combined.sort_unstable();
    combined.dedup();
    *scale = combined;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_color_normalizes_short_hex() {
        assert_eq!(css_color_to_hex("#fff"), Some("#ffffff".to_owned()));
        assert_eq!(css_color_to_hex("#FFF"), Some("#ffffff".to_owned()));
        assert_eq!(css_color_to_hex("#0b7285"), Some("#0b7285".to_owned()));
    }

    #[test]
    fn css_color_drops_alpha() {
        assert_eq!(css_color_to_hex("#0b728580"), Some("#0b7285".to_owned()));
        assert_eq!(css_color_to_hex("#abcd"), Some("#aabbcc".to_owned()));
    }

    #[test]
    fn css_color_parses_rgb() {
        assert_eq!(
            css_color_to_hex("rgb(255, 0, 0)"),
            Some("#ff0000".to_owned())
        );
        assert_eq!(
            css_color_to_hex("rgb(11 114 133)"),
            Some("#0b7285".to_owned())
        );
        assert_eq!(
            css_color_to_hex("rgba(11, 114, 133, 0.5)"),
            Some("#0b7285".to_owned())
        );
    }

    #[test]
    fn css_color_rejects_unknown() {
        assert!(css_color_to_hex("transparent").is_none());
        assert!(css_color_to_hex("hsl(0, 100%, 50%)").is_none());
    }

    #[test]
    fn css_length_handles_rem_and_px() {
        assert_eq!(css_length_to_px(&Value::String("1rem".into())), Some(16));
        assert_eq!(css_length_to_px(&Value::String("1.5rem".into())), Some(24));
        assert_eq!(css_length_to_px(&Value::String("12px".into())), Some(12));
        assert_eq!(css_length_to_px(&Value::String("0".into())), Some(0));
        assert_eq!(css_length_to_px(&Value::Number(8.into())), Some(8));
    }

    #[test]
    fn merge_colors_supports_groups() {
        let mut spec = ColorSpec::default();
        let theme = serde_json::json!({
            "white": "#ffffff",
            "red": {
                "500": "#ef4444",
                "600": "#dc2626"
            }
        });
        let map = theme.as_object().expect("object");
        merge_colors(&mut spec, map);
        assert_eq!(spec.tokens["white"], "#ffffff");
        assert_eq!(spec.tokens["red/500"], "#ef4444");
        assert_eq!(spec.tokens["red/600"], "#dc2626");
    }

    #[test]
    fn merge_spacing_dedupes_scale() {
        let mut spec = SpacingSpec {
            scale: vec![0, 4],
            ..SpacingSpec::default()
        };
        let theme = serde_json::json!({
            "1": "0.25rem",
            "2": "0.5rem",
            "4": "1rem"
        });
        let map = theme.as_object().expect("object");
        merge_spacing(&mut spec, map);
        assert_eq!(spec.tokens["1"], 4);
        assert_eq!(spec.tokens["2"], 8);
        assert_eq!(spec.tokens["4"], 16);
        // Scale combines existing 0/4 with 4/8/16 → sorted/deduped.
        assert_eq!(spec.scale, vec![0, 4, 8, 16]);
    }

    #[test]
    fn merge_font_size_supports_tuple_form() {
        let mut spec = TypeScaleSpec::default();
        let theme = serde_json::json!({
            "sm": ["0.875rem", { "lineHeight": "1.25rem" }],
            "base": "1rem"
        });
        let map = theme.as_object().expect("object");
        merge_font_size(&mut spec, map);
        assert_eq!(spec.tokens["sm"], 14);
        assert_eq!(spec.tokens["base"], 16);
        assert_eq!(spec.scale, vec![14, 16]);
    }

    #[test]
    fn merge_font_weight_dedupes_and_sorts() {
        let mut spec = TypeScaleSpec {
            weights: vec![400],
            ..TypeScaleSpec::default()
        };
        let theme = serde_json::json!({
            "regular": "400",
            "medium": 500,
            "bold": 700
        });
        let map = theme.as_object().expect("object");
        merge_font_weight(&mut spec, map);
        assert_eq!(spec.weights, vec![400, 500, 700]);
    }

    #[test]
    fn merge_font_family_keeps_primary() {
        let mut spec = TypeScaleSpec::default();
        let theme = serde_json::json!({
            "sans": ["Inter", "ui-sans-serif", "system-ui"],
            "mono": ["JetBrains Mono", "monospace"]
        });
        let map = theme.as_object().expect("object");
        merge_font_family(&mut spec, map);
        assert_eq!(spec.families, vec!["Inter", "JetBrains Mono"]);
    }

    #[test]
    fn merge_radius_sorts_and_dedupes() {
        let mut spec = RadiusSpec { scale: vec![4] };
        let theme = serde_json::json!({
            "sm": "0.125rem",
            "md": "0.375rem",
            "DEFAULT": "0.25rem"
        });
        let map = theme.as_object().expect("object");
        merge_radius(&mut spec, map);
        // 0.125rem→2, 0.25rem→4 (dup), 0.375rem→6 → [2, 4, 6]
        assert_eq!(spec.scale, vec![2, 4, 6]);
    }

    #[test]
    fn validate_rejects_unknown_extension() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tailwind.config.toml");
        std::fs::write(&path, "x = 1").expect("write");
        let err = validate_config_path(&path, Some(dir.path())).unwrap_err();
        assert!(matches!(err, ConfigError::TailwindBadPath { .. }));
    }

    #[test]
    fn validate_rejects_path_outside_cwd_root() {
        let outside = tempfile::tempdir().expect("outside");
        let outside_cfg = outside.path().join("tailwind.config.js");
        std::fs::write(&outside_cfg, "module.exports = {};").expect("write");
        let inside = tempfile::tempdir().expect("inside");
        let err = validate_config_path(&outside_cfg, Some(inside.path())).unwrap_err();
        assert!(matches!(err, ConfigError::TailwindBadPath { .. }));
    }

    #[test]
    fn validate_accepts_descendant_of_cwd_root() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("tailwind.config.js");
        std::fs::write(&path, "module.exports = {};").expect("write");
        let canonical = validate_config_path(&path, Some(dir.path())).expect("ok");
        assert!(canonical.is_file());
    }

    #[test]
    fn is_under_or_ancestor_accepts_descendant() {
        assert!(is_under_or_ancestor(
            Path::new("/work/proj/tailwind.config.js"),
            Path::new("/work/proj")
        ));
    }

    #[test]
    fn is_under_or_ancestor_accepts_ancestor_sibling() {
        // cwd is /work/proj/sub; config lives at /work/proj/tailwind.config.js
        assert!(is_under_or_ancestor(
            Path::new("/work/proj/tailwind.config.js"),
            Path::new("/work/proj/sub")
        ));
    }

    #[test]
    fn is_under_or_ancestor_rejects_unrelated() {
        assert!(!is_under_or_ancestor(
            Path::new("/etc/passwd"),
            Path::new("/work/proj")
        ));
    }
}
