//! `plumb init` — write a starter `plumb.toml`.
//!
//! Detects Tailwind (and, as a hint, Next.js) in the current directory
//! and branches between two scaffolds: a generic template, or a
//! Tailwind-flavoured template that records the discovered config in
//! its header comment.
//!
//! Detection is filesystem-only — no JS evaluation, no env var reads.
//! `tailwind.config.{ts,mts,cts,js,mjs,cjs}` in CWD or a `tailwindcss`
//! entry in `package.json`'s `dependencies` / `devDependencies` /
//! `peerDependencies` triggers the Tailwind template. Next.js without
//! Tailwind keeps the generic template — we hint at it in the summary
//! line but don't switch flavours.

use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};

const GENERIC_TEMPLATE: &str = include_str!("../../../../examples/plumb.toml");
const TAILWIND_TEMPLATE: &str = include_str!("../../../../examples/plumb-tailwind.toml");
const TAILWIND_PLACEHOLDER: &str = "{{TAILWIND_CONFIG}}";

const TAILWIND_CONFIG_NAMES: &[&str] = &[
    "tailwind.config.ts",
    "tailwind.config.mts",
    "tailwind.config.cts",
    "tailwind.config.js",
    "tailwind.config.mjs",
    "tailwind.config.cjs",
];

/// What `detect` discovered about the current directory.
#[derive(Debug, Clone, Default)]
struct Detection {
    /// Bare filename of the discovered Tailwind config, if any.
    tailwind_config: Option<String>,
    /// `next` listed in `package.json` deps.
    has_next: bool,
    /// `tailwindcss` listed in `package.json` deps.
    has_tailwind_dep: bool,
}

impl Detection {
    fn is_tailwind_project(&self) -> bool {
        self.tailwind_config.is_some() || self.has_tailwind_dep
    }
}

/// Run `plumb init`. Returns `ExitCode::SUCCESS` on a fresh write.
///
/// # Errors
///
/// Returns an error if the current directory cannot be read, if
/// `plumb.toml` already exists and `force` is `false`, or if the file
/// cannot be written.
pub fn run(force: bool) -> Result<ExitCode> {
    let cwd = std::env::current_dir().context("read current working directory")?;
    let target = cwd.join("plumb.toml");
    if target.exists() && !force {
        bail!(
            "{} already exists; pass --force to overwrite.",
            target.display()
        );
    }
    let detection = detect(&cwd);
    let (content, summary) = render(&detection);
    std::fs::write(&target, content.as_bytes())
        .with_context(|| format!("write {}", target.display()))?;
    #[allow(clippy::print_stdout)]
    {
        println!("Wrote {}. {summary}", target.display());
    }
    Ok(ExitCode::SUCCESS)
}

/// Inspect `cwd` for Tailwind / Next.js signals.
fn detect(cwd: &Path) -> Detection {
    let mut detection = Detection::default();
    for name in TAILWIND_CONFIG_NAMES {
        if cwd.join(name).is_file() {
            detection.tailwind_config = Some((*name).to_string());
            break;
        }
    }
    let (has_next, has_tailwind_dep) = read_package_deps(&cwd.join("package.json"));
    detection.has_next = has_next;
    detection.has_tailwind_dep = has_tailwind_dep;
    detection
}

/// Parse `package.json` for `next` and `tailwindcss` entries across the
/// three dependency tables. Missing or malformed files yield
/// `(false, false)` — detection is best-effort.
fn read_package_deps(path: &Path) -> (bool, bool) {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return (false, false);
    };
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return (false, false);
    };
    let mut has_next = false;
    let mut has_tailwind = false;
    for table in ["dependencies", "devDependencies", "peerDependencies"] {
        let Some(map) = parsed.get(table).and_then(|v| v.as_object()) else {
            continue;
        };
        if map.contains_key("next") {
            has_next = true;
        }
        if map.contains_key("tailwindcss") {
            has_tailwind = true;
        }
    }
    (has_next, has_tailwind)
}

/// Build the file contents and the one-line stdout summary for a given
/// detection result.
fn render(detection: &Detection) -> (String, String) {
    if detection.is_tailwind_project() {
        let config_label = detection
            .tailwind_config
            .clone()
            .unwrap_or_else(|| "tailwind.config.js".to_string());
        let content = TAILWIND_TEMPLATE.replace(TAILWIND_PLACEHOLDER, &config_label);
        let summary = if detection.tailwind_config.is_some() {
            if detection.has_next {
                format!("Tailwind config detected at ./{config_label} (Next.js project).")
            } else {
                format!("Tailwind config detected at ./{config_label}.")
            }
        } else if detection.has_next {
            "Tailwind config detected via package.json (Next.js project).".to_string()
        } else {
            "Tailwind config detected via package.json.".to_string()
        };
        (content, summary)
    } else {
        let summary = if detection.has_next {
            "Generic template (Next.js detected, no framework styles found).".to_string()
        } else {
            "Generic template.".to_string()
        };
        (GENERIC_TEMPLATE.to_string(), summary)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn render_generic_when_nothing_detected() {
        let detection = Detection::default();
        let (content, summary) = render(&detection);
        assert_eq!(content, GENERIC_TEMPLATE);
        assert!(summary.starts_with("Generic template"));
        assert!(!summary.contains("Tailwind"));
    }

    #[test]
    fn render_tailwind_substitutes_config_path() {
        let detection = Detection {
            tailwind_config: Some("tailwind.config.ts".to_string()),
            has_next: true,
            has_tailwind_dep: true,
        };
        let (content, summary) = render(&detection);
        assert!(content.contains("./tailwind.config.ts"));
        assert!(!content.contains(TAILWIND_PLACEHOLDER));
        assert!(summary.contains("Tailwind config detected"));
        assert!(summary.contains("./tailwind.config.ts"));
        assert!(summary.contains("Next.js"));
    }

    #[test]
    fn render_tailwind_dep_alone_triggers_template() {
        let detection = Detection {
            tailwind_config: None,
            has_next: false,
            has_tailwind_dep: true,
        };
        let (content, summary) = render(&detection);
        assert!(content.contains("Tailwind"));
        assert!(!content.contains(TAILWIND_PLACEHOLDER));
        assert!(summary.contains("Tailwind config detected"));
    }

    #[test]
    fn render_next_alone_keeps_generic_template() {
        let detection = Detection {
            tailwind_config: None,
            has_next: true,
            has_tailwind_dep: false,
        };
        let (content, summary) = render(&detection);
        assert_eq!(content, GENERIC_TEMPLATE);
        assert!(!summary.contains("Tailwind"));
        assert!(summary.contains("Next.js"));
    }

    #[test]
    fn read_package_deps_handles_missing_file() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let (has_next, has_tw) = read_package_deps(&dir.path().join("missing.json"));
        assert!(!has_next);
        assert!(!has_tw);
    }

    #[test]
    fn read_package_deps_walks_all_three_tables() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("package.json");
        std::fs::write(
            &path,
            r#"{
                "dependencies": { "react": "18" },
                "devDependencies": { "tailwindcss": "3.4" },
                "peerDependencies": { "next": "14" }
            }"#,
        )
        .expect("write");
        let (has_next, has_tw) = read_package_deps(&path);
        assert!(has_next);
        assert!(has_tw);
    }

    #[test]
    fn read_package_deps_tolerates_malformed_json() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let path = dir.path().join("package.json");
        std::fs::write(&path, "{ not json").expect("write");
        let (has_next, has_tw) = read_package_deps(&path);
        assert!(!has_next);
        assert!(!has_tw);
    }

    #[test]
    fn detect_finds_typescript_config_first() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        std::fs::write(dir.path().join("tailwind.config.ts"), "").expect("write ts");
        std::fs::write(dir.path().join("tailwind.config.js"), "").expect("write js");
        let detection = detect(dir.path());
        assert_eq!(
            detection.tailwind_config.as_deref(),
            Some("tailwind.config.ts")
        );
    }
}
