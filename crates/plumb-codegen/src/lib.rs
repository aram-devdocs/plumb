//! # plumb-codegen
//!
//! Source-tree token inference for Plumb. Walks a project directory,
//! discovers design-token sources (CSS custom properties, Tailwind
//! config files, DTCG token JSON), and bootstraps a best-effort
//! [`plumb_core::Config`].
//!
//! Consumers (`plumb-cli`'s `init --from <path>` command) call
//! [`infer_config`] to walk the tree and [`render_toml`] to serialize
//! the result. Both are deterministic: identical inputs produce
//! byte-identical output across runs and platforms.
//!
//! ## Inference sources (V0)
//!
//! - **CSS custom properties.** Every `:root { --token: value; }`
//!   declaration discovered under `src/`, `styles/`, `app/`, or the
//!   project root is classified by name into the spacing, color,
//!   radius, and type-scale buckets. Implementation lives in
//!   [`plumb_config::scrape_css_properties`].
//! - **Tailwind config files.** Presence of `tailwind.config.{js,ts,
//!   mjs,cjs,mts,cts}` is recorded in the rendered TOML's header
//!   comment so the user knows to wire `extends` once that landed.
//!   The crate never spawns Node; full Tailwind theme resolution is
//!   handled at lint time by `plumb_config::merge_tailwind`.
//! - **DTCG token JSON files.** Files matching `*.tokens.json` or
//!   placed under a `tokens/` directory are merged via
//!   [`plumb_config::merge_dtcg`].
//!
//! ## Determinism contract
//!
//! - Directory entries are sorted by their canonical UTF-8 path before
//!   recursion. The walker visits files in the same order on every
//!   filesystem.
//! - Scales (`spacing.scale`, `radius.scale`, `type.scale`) are sorted
//!   ascending and deduplicated.
//! - Tokens land in [`indexmap::IndexMap`] in discovery order; insertion
//!   order is preserved by serde during TOML serialization.
//! - The walker never reads `SystemTime` / `Instant`. The error
//!   surface never carries a wall-clock value.

#![forbid(unsafe_code)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/aram-devdocs/plumb/main/assets/brand/plumb-mark.svg",
    html_favicon_url = "https://raw.githubusercontent.com/aram-devdocs/plumb/main/theme/favicon.svg"
)]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod classify;
mod render;
mod walk;

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use plumb_config::ConfigError;
use plumb_core::Config;
use thiserror::Error;

pub use render::render_toml;

/// Maximum directory depth the walker descends into the source tree.
///
/// Most design-token directories sit at depth ≤ 3 (`src/styles/tokens.css`).
/// 6 covers monorepos with `apps/<name>/src/styles/...` without spending
/// time on `node_modules`-shaped trees that the walker would otherwise
/// already skip by name.
pub const MAX_WALK_DEPTH: usize = 6;

/// Codegen errors.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CodegenError {
    /// The supplied source directory does not exist.
    #[error("source directory not found: {0}")]
    NotFound(String),
    /// The supplied path exists but is not a directory.
    #[error("source path is not a directory: {0}")]
    NotADirectory(String),
    /// A filesystem error surfaced during the walk.
    #[error("failed to read `{path}`: {source}")]
    Io {
        /// Path that failed to read.
        path: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// A discovered token source failed to parse. The wrapped
    /// [`ConfigError`] carries the span-annotated diagnostic.
    #[error("failed to parse token source: {0}")]
    Source(#[from] ConfigError),
    /// TOML serialization failed.
    #[error("failed to render TOML: {0}")]
    Render(#[from] toml::ser::Error),
}

/// Result of walking a source tree. The [`Config`] field is populated
/// with whatever tokens the inference passes were able to recover; the
/// `summary` field records, in stable order, what each pass discovered
/// so the CLI can surface a one-line note per source.
#[derive(Debug, Clone)]
pub struct InferredConfig {
    /// The inferred config — passed through `serde(deny_unknown_fields)`
    /// so it round-trips cleanly through `toml::to_string` and back.
    pub config: Config,
    /// One human-readable line per inference pass that contributed.
    /// Sorted by `(source_kind, path)` for stable rendering.
    pub summary: Vec<String>,
    /// Token-source files the walker fed to a parser, in the order they
    /// were consumed. Used for the rendered header comment and tests.
    pub sources: Vec<TokenSource>,
}

/// One discovered token-source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSource {
    /// Canonical kind tag for the source.
    pub kind: TokenSourceKind,
    /// Path relative to the input `source_dir`.
    pub relative_path: PathBuf,
}

/// Kind of a discovered token source. Used to drive both the renderer's
/// header comment and the per-source summary order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum TokenSourceKind {
    /// `tailwind.config.{js,ts,mjs,cjs,mts,cts}` at the project root.
    /// V0 records the presence in the header comment; full theme
    /// resolution is on the linter side.
    TailwindConfig,
    /// CSS file containing one or more `:root` blocks.
    CssCustomProperties,
    /// DTCG token document (`*.tokens.json` or `tokens/*.json`).
    Dtcg,
}

impl TokenSourceKind {
    /// Stable, lower-case label used in summaries and rendered header
    /// comments.
    fn label(self) -> &'static str {
        match self {
            Self::TailwindConfig => "tailwind",
            Self::CssCustomProperties => "css",
            Self::Dtcg => "dtcg",
        }
    }
}

/// File extensions of supported Tailwind config files. Order does not
/// matter for matching but the first hit (in walker order) is what we
/// surface in the rendered TOML.
const TAILWIND_CONFIG_NAMES: &[&str] = &[
    "tailwind.config.ts",
    "tailwind.config.mts",
    "tailwind.config.cts",
    "tailwind.config.js",
    "tailwind.config.mjs",
    "tailwind.config.cjs",
];

/// Walk `source_dir` and infer a [`plumb_core::Config`] from the
/// design-token sources it finds.
///
/// The walker is bounded by [`MAX_WALK_DEPTH`] and skips
/// `node_modules`, `target`, `dist`, `build`, `.next`, `out`, and any
/// dotfile directory.
///
/// # Errors
///
/// - [`CodegenError::NotFound`] if `source_dir` does not exist.
/// - [`CodegenError::NotADirectory`] if `source_dir` is not a directory.
/// - [`CodegenError::Io`] if a directory entry cannot be read.
/// - [`CodegenError::Source`] if a discovered token source fails to
///   parse. Parse errors carry the source span via
///   [`plumb_config::ConfigError`].
pub fn infer_config(source_dir: &Path) -> Result<InferredConfig, CodegenError> {
    if !source_dir.exists() {
        return Err(CodegenError::NotFound(source_dir.display().to_string()));
    }
    if !source_dir.is_dir() {
        return Err(CodegenError::NotADirectory(
            source_dir.display().to_string(),
        ));
    }

    let walked = walk::walk(source_dir)?;

    let mut config = Config::default();
    let mut summary: Vec<(u8, String, String)> = Vec::new();
    let mut sources: Vec<TokenSource> = Vec::new();

    // Tailwind config — record presence only. Theme resolution is the
    // linter's job (it spawns Node lazily on `plumb lint`).
    for tailwind_path in &walked.tailwind_configs {
        let relative = relative_to(source_dir, tailwind_path);
        sources.push(TokenSource {
            kind: TokenSourceKind::TailwindConfig,
            relative_path: relative.clone(),
        });
        summary.push((
            order_tag(TokenSourceKind::TailwindConfig),
            display_path(&relative),
            format!("tailwind config at {}", display_path(&relative)),
        ));
    }

    // CSS custom properties.
    if !walked.css_files.is_empty() {
        let scrapes = plumb_config::scrape_css_properties(&walked.css_files)?;
        // Record one summary line per CSS file the scraper emitted from.
        let mut by_file: IndexMap<PathBuf, classify::PerFileStats> = IndexMap::new();
        for scrape in &scrapes {
            by_file
                .entry(scrape.source.clone())
                .or_default()
                .increment(&scrape.value);
        }
        classify::classify_css_scrapes(&scrapes, &mut config);
        // Drain the per-file stats in insertion order (= scraper order =
        // sorted walk order).
        for (path, file_stats) in by_file {
            let relative = relative_to(source_dir, &path);
            sources.push(TokenSource {
                kind: TokenSourceKind::CssCustomProperties,
                relative_path: relative.clone(),
            });
            summary.push((
                order_tag(TokenSourceKind::CssCustomProperties),
                display_path(&relative),
                format!(
                    "css custom properties from {} ({} colors, {} dimensions, {} other)",
                    display_path(&relative),
                    file_stats.colors,
                    file_stats.dimensions,
                    file_stats.other,
                ),
            ));
        }
    }

    // DTCG token JSON.
    for dtcg_path in &walked.dtcg_files {
        let contents = std::fs::read_to_string(dtcg_path).map_err(|source| CodegenError::Io {
            path: dtcg_path.display().to_string(),
            source,
        })?;
        let source = plumb_config::DtcgSource {
            path: dtcg_path.clone(),
            contents,
        };
        let import = plumb_config::merge_dtcg(&mut config, &source)?;
        let relative = relative_to(source_dir, dtcg_path);
        sources.push(TokenSource {
            kind: TokenSourceKind::Dtcg,
            relative_path: relative.clone(),
        });
        summary.push((
            order_tag(TokenSourceKind::Dtcg),
            display_path(&relative),
            format!(
                "dtcg tokens from {} (+{} colors, +{} spacing, +{} type sizes, +{} radii)",
                display_path(&relative),
                import.color_added,
                import.spacing_added,
                import.type_size_added,
                import.radius_added,
            ),
        ));
    }

    // Sort scales ascending with duplicates removed — deterministic
    // output regardless of file walk order.
    sort_and_dedup(&mut config.spacing.scale);
    sort_and_dedup(&mut config.type_scale.scale);
    sort_and_dedup(&mut config.radius.scale);

    // Stable summary order: `(kind tag, relative path)`.
    summary.sort();
    let summary = summary.into_iter().map(|(_, _, line)| line).collect();

    Ok(InferredConfig {
        config,
        summary,
        sources,
    })
}

/// Lower numbers sort earlier in the rendered summary. Tailwind first
/// (it's the framework signal), then CSS, then DTCG.
fn order_tag(kind: TokenSourceKind) -> u8 {
    match kind {
        TokenSourceKind::TailwindConfig => 0,
        TokenSourceKind::CssCustomProperties => 1,
        TokenSourceKind::Dtcg => 2,
    }
}

/// Compute `path` relative to `base`, falling back to `path` when the
/// strip fails (e.g. an absolute path the walker handed back verbatim
/// because canonicalization was not possible).
fn relative_to(base: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(base)
        .map_or_else(|_| path.to_path_buf(), Path::to_path_buf)
}

/// Render a path with forward slashes regardless of host OS so summaries
/// and TOML headers are byte-identical across Windows / Linux / macOS.
fn display_path(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn sort_and_dedup<T: Ord>(values: &mut Vec<T>) {
    values.sort();
    values.dedup();
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn missing_source_dir_errors() {
        let err = infer_config(Path::new("/nonexistent/plumb/codegen/test"))
            .expect_err("infer_config should fail on missing path");
        assert!(matches!(err, CodegenError::NotFound(_)));
    }

    #[test]
    fn non_directory_errors() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not-a-dir.txt");
        std::fs::write(&file, "hello").unwrap();
        let err = infer_config(&file).expect_err("infer_config should fail on file path");
        assert!(matches!(err, CodegenError::NotADirectory(_)));
    }

    #[test]
    fn empty_dir_returns_default_config() {
        let dir = tempfile::tempdir().unwrap();
        let inferred = infer_config(dir.path()).unwrap();
        assert!(inferred.summary.is_empty());
        assert!(inferred.sources.is_empty());
        assert!(inferred.config.color.tokens.is_empty());
        assert!(inferred.config.spacing.scale.is_empty());
    }

    #[test]
    fn detects_tailwind_config() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("tailwind.config.ts"),
            "export default { content: [] };\n",
        )
        .unwrap();
        let inferred = infer_config(dir.path()).unwrap();
        assert_eq!(inferred.sources.len(), 1);
        assert_eq!(inferred.sources[0].kind, TokenSourceKind::TailwindConfig);
        assert_eq!(
            inferred.sources[0].relative_path,
            Path::new("tailwind.config.ts")
        );
    }

    #[test]
    fn classifies_css_custom_properties_into_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let styles = dir.path().join("styles");
        std::fs::create_dir_all(&styles).unwrap();
        std::fs::write(
            styles.join("tokens.css"),
            r":root {
              --color-bg: #ffffff;
              --color-fg: #0b0b0b;
              --color-accent: #0b7285;
              --space-xs: 4px;
              --space-sm: 8px;
              --radius-md: 8px;
            }",
        )
        .unwrap();
        let inferred = infer_config(dir.path()).unwrap();
        assert_eq!(inferred.config.color.tokens.len(), 3);
        assert_eq!(
            inferred.config.color.tokens.get("color-bg"),
            Some(&"#ffffff".to_owned())
        );
        assert_eq!(inferred.config.spacing.scale, vec![4, 8]);
        assert_eq!(inferred.config.radius.scale, vec![8]);
    }

    #[test]
    fn skips_node_modules_and_dotfile_dirs() {
        let dir = tempfile::tempdir().unwrap();
        // Should be skipped.
        for skipped in ["node_modules", "target", ".git", "dist", "build"] {
            let nested = dir.path().join(skipped).join("nested");
            std::fs::create_dir_all(&nested).unwrap();
            std::fs::write(
                nested.join("trap.css"),
                ":root { --color-trap: #ff0000; }\n",
            )
            .unwrap();
        }
        let inferred = infer_config(dir.path()).unwrap();
        assert!(inferred.config.color.tokens.is_empty());
        assert!(inferred.sources.is_empty());
    }

    #[test]
    fn deterministic_across_runs() {
        let dir = tempfile::tempdir().unwrap();
        let styles = dir.path().join("src/styles");
        std::fs::create_dir_all(&styles).unwrap();
        std::fs::write(
            styles.join("a.css"),
            ":root { --color-a: #aabbcc; --space-xs: 4px; }",
        )
        .unwrap();
        std::fs::write(
            styles.join("b.css"),
            ":root { --color-b: #112233; --space-sm: 8px; }",
        )
        .unwrap();
        let one = infer_config(dir.path()).unwrap();
        let two = infer_config(dir.path()).unwrap();
        assert_eq!(one.summary, two.summary);
        assert_eq!(one.config.color.tokens, two.config.color.tokens);
        assert_eq!(one.config.spacing.scale, two.config.spacing.scale);
    }

    #[test]
    fn merges_dtcg_token_files() {
        let dir = tempfile::tempdir().unwrap();
        let dtcg = r##"{
          "color": {
            "primary": { "$type": "color", "$value": "#0b7285" }
          },
          "spacing": {
            "xs": { "$type": "dimension", "$value": "4px" }
          }
        }"##;
        std::fs::write(dir.path().join("design.tokens.json"), dtcg).unwrap();
        let inferred = infer_config(dir.path()).unwrap();
        assert_eq!(
            inferred.config.color.tokens.get("color/primary"),
            Some(&"#0b7285".to_owned())
        );
        assert!(inferred.config.spacing.tokens.contains_key("spacing/xs"));
        assert_eq!(inferred.sources.len(), 1);
        assert_eq!(inferred.sources[0].kind, TokenSourceKind::Dtcg);
    }

    #[test]
    fn order_tag_orders_kinds_predictably() {
        assert!(
            order_tag(TokenSourceKind::TailwindConfig)
                < order_tag(TokenSourceKind::CssCustomProperties)
        );
        assert!(order_tag(TokenSourceKind::CssCustomProperties) < order_tag(TokenSourceKind::Dtcg));
    }

    #[test]
    fn display_path_uses_forward_slashes() {
        let p = Path::new("src").join("styles").join("tokens.css");
        assert_eq!(display_path(&p), "src/styles/tokens.css");
    }

    #[test]
    fn label_lookup_is_stable() {
        assert_eq!(TokenSourceKind::TailwindConfig.label(), "tailwind");
        assert_eq!(TokenSourceKind::CssCustomProperties.label(), "css");
        assert_eq!(TokenSourceKind::Dtcg.label(), "dtcg");
    }
}
