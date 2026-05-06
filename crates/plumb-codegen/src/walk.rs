//! Deterministic source-tree walker.
//!
//! Reads directory entries, filters them through a hard-coded ignore
//! list (`node_modules`, `target`, `.git`, `dist`, `build`, `.next`,
//! `out`, and any dotfile dir), and emits sorted lists of token-source
//! files: Tailwind configs, CSS files, and DTCG token JSON files.

// Items here are crate-private but live inside a private module; the
// `redundant_pub_crate` lint flips between deny on `pub(crate)` and the
// rust-level `unreachable_pub` lint on bare `pub`. Allow the former
// scoped to this module so the items keep the explicit visibility.
#![allow(clippy::redundant_pub_crate)]

use std::path::{Path, PathBuf};

use crate::{CodegenError, MAX_WALK_DEPTH, TAILWIND_CONFIG_NAMES};

/// Directory names hard-skipped during the walk regardless of depth.
/// Sorted alphabetically for code-review readability — order does not
/// affect behavior.
const SKIPPED_DIRS: &[&str] = &[
    ".git",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".turbo",
    "build",
    "coverage",
    "dist",
    "node_modules",
    "out",
    "target",
];

/// Discovered token-source paths, grouped by kind.
///
/// Each list is sorted so the caller-visible output is deterministic.
#[derive(Debug, Default)]
pub(crate) struct Walked {
    /// Tailwind config files discovered at the project root.
    /// V0 looks at root only — nested apps live under
    /// `apps/*/tailwind.config.*` and we do not want to claim a single
    /// inferred config for a polyrepo.
    pub(crate) tailwind_configs: Vec<PathBuf>,
    /// CSS files (`*.css`) the walker fed to the CSS scraper.
    pub(crate) css_files: Vec<PathBuf>,
    /// DTCG token JSON files (`*.tokens.json` or under `tokens/`).
    pub(crate) dtcg_files: Vec<PathBuf>,
}

/// Walk `source_dir` and return a [`Walked`] with token-source paths
/// sorted in a stable order.
pub(crate) fn walk(source_dir: &Path) -> Result<Walked, CodegenError> {
    let mut walked = Walked::default();

    // Tailwind: project root only.
    for name in TAILWIND_CONFIG_NAMES {
        let candidate = source_dir.join(name);
        if candidate.is_file() {
            walked.tailwind_configs.push(candidate);
        }
    }
    walked.tailwind_configs.sort();

    walk_dir(source_dir, source_dir, 0, &mut walked)?;

    walked.css_files.sort();
    walked.dtcg_files.sort();

    Ok(walked)
}

fn walk_dir(
    root: &Path,
    dir: &Path,
    depth: usize,
    walked: &mut Walked,
) -> Result<(), CodegenError> {
    if depth > MAX_WALK_DEPTH {
        return Ok(());
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(source) => {
            return Err(CodegenError::Io {
                path: dir.display().to_string(),
                source,
            });
        }
    };

    let mut sorted: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| CodegenError::Io {
            path: dir.display().to_string(),
            source,
        })?;
        sorted.push(entry.path());
    }
    sorted.sort();

    for path in sorted {
        let file_type = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta.file_type(),
            Err(_) => continue,
        };
        if file_type.is_symlink() {
            // Skip symlinks; they could escape the source tree, and
            // following them would risk cycles.
            continue;
        }
        if file_type.is_dir() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if name.is_empty() || name.starts_with('.') || SKIPPED_DIRS.contains(&name) {
                continue;
            }
            walk_dir(root, &path, depth + 1, walked)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        classify_file(root, &path, walked);
    }

    Ok(())
}

/// Decide which bucket (if any) a single file belongs to.
fn classify_file(root: &Path, path: &Path, walked: &mut Walked) {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return;
    };

    // Skip `tailwind.config.*` here — already discovered at the root.
    if TAILWIND_CONFIG_NAMES.contains(&name) {
        return;
    }

    let lower = name.to_ascii_lowercase();

    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("css"))
    {
        walked.css_files.push(path.to_path_buf());
        return;
    }

    // `.tokens.json` is a compound suffix; `Path::extension` returns
    // `json`, so we still inspect the lower-cased file name to catch
    // the design-token convention.
    if lower.ends_with(".tokens.json") {
        walked.dtcg_files.push(path.to_path_buf());
        return;
    }
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        && in_tokens_dir(root, path)
    {
        walked.dtcg_files.push(path.to_path_buf());
    }
}

/// True if `path` lives under a directory literally named `tokens` at
/// any depth between `root` and the file. The check uses ASCII
/// case-insensitive comparison so `Tokens/` works on case-sensitive
/// filesystems too.
fn in_tokens_dir(root: &Path, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        return false;
    };
    rel.components().any(|c| {
        c.as_os_str()
            .to_str()
            .is_some_and(|s| s.eq_ignore_ascii_case("tokens"))
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn walk_finds_tailwind_root_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("tailwind.config.js"), "module.exports={};").unwrap();
        // Nested tailwind.config.* should NOT be picked up by the
        // walker (V0 heuristic: root-only).
        let nested = dir.path().join("apps").join("docs");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("tailwind.config.js"), "module.exports={};").unwrap();

        let walked = walk(dir.path()).unwrap();
        assert_eq!(walked.tailwind_configs.len(), 1);
        assert_eq!(
            walked.tailwind_configs[0],
            dir.path().join("tailwind.config.js")
        );
    }

    #[test]
    fn walk_collects_css_in_sorted_order() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("styles")).unwrap();
        std::fs::write(dir.path().join("styles/b.css"), ":root {}").unwrap();
        std::fs::write(dir.path().join("styles/a.css"), ":root {}").unwrap();
        std::fs::write(dir.path().join("z.css"), ":root {}").unwrap();

        let walked = walk(dir.path()).unwrap();
        assert_eq!(walked.css_files.len(), 3);
        // Walker emits children before siblings of the parent dir, but
        // each call to `read_dir` returns sorted output. The end result
        // is depth-first sorted: `styles/a.css`, `styles/b.css`, `z.css`.
        assert_eq!(walked.css_files[0], dir.path().join("styles/a.css"));
        assert_eq!(walked.css_files[1], dir.path().join("styles/b.css"));
        assert_eq!(walked.css_files[2], dir.path().join("z.css"));
    }

    #[test]
    fn walk_picks_up_dtcg_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("design.tokens.json"), "{}").unwrap();
        std::fs::create_dir_all(dir.path().join("tokens")).unwrap();
        std::fs::write(dir.path().join("tokens/colors.json"), "{}").unwrap();
        // Lone `colors.json` outside a `tokens/` dir is ignored.
        std::fs::write(dir.path().join("colors.json"), "{}").unwrap();

        let walked = walk(dir.path()).unwrap();
        assert_eq!(walked.dtcg_files.len(), 2);
    }

    #[test]
    fn walk_skips_hard_blocked_dirs() {
        let dir = tempfile::tempdir().unwrap();
        for skipped in SKIPPED_DIRS {
            let nested = dir.path().join(skipped).join("inner");
            std::fs::create_dir_all(&nested).unwrap();
            std::fs::write(nested.join("trap.css"), ":root {}").unwrap();
            std::fs::write(nested.join("design.tokens.json"), "{}").unwrap();
        }
        let walked = walk(dir.path()).unwrap();
        assert!(walked.css_files.is_empty(), "skipped CSS leaked through");
        assert!(walked.dtcg_files.is_empty(), "skipped DTCG leaked through");
    }

    #[test]
    fn walk_respects_max_depth() {
        let dir = tempfile::tempdir().unwrap();
        let mut deep = dir.path().to_path_buf();
        for level in 0..(MAX_WALK_DEPTH + 2) {
            deep = deep.join(format!("d{level}"));
        }
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("buried.css"), ":root {}").unwrap();
        let walked = walk(dir.path()).unwrap();
        assert!(walked.css_files.is_empty());
    }

    #[test]
    fn walk_skips_dotfile_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let dot = dir.path().join(".vscode");
        std::fs::create_dir_all(&dot).unwrap();
        std::fs::write(dot.join("settings.css"), ":root {}").unwrap();
        let walked = walk(dir.path()).unwrap();
        assert!(walked.css_files.is_empty());
    }
}
