//! # xtask
//!
//! Plumb's developer task runner. Code-generation and pre-release tasks
//! that benefit from being real Rust (type-safe, no shell quoting) live
//! here. Shell-only tasks stay in `justfile`.
//!
//! Invoke with `cargo xtask <subcommand>` (alias declared in
//! `.cargo/config.toml`).

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]
#![allow(unreachable_pub)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "xtask", about = "Plumb developer task runner.")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Emit the canonical JSON Schema for `plumb.toml` to
    /// `schemas/plumb.toml.json` (creating the directory if missing).
    Schema {
        /// Output path. Defaults to `schemas/plumb.toml.json`.
        #[arg(long, default_value = "schemas/plumb.toml.json")]
        out: PathBuf,
    },
    /// Regenerate the list of built-in rules under `docs/src/rules/`
    /// index so it matches `register_builtin()`.
    SyncRulesIndex,
    /// Validate every runbook spec under `docs/runbooks/*.yaml` against
    /// `schemas/runbook-spec.json`. Delegates to the Python generator's
    /// `--validate-only` mode to reuse the JSON-Schema machinery there.
    ValidateRunbooks {
        /// Override the runbooks directory.
        #[arg(long, default_value = "docs/runbooks")]
        dir: PathBuf,
    },
    /// Validate the docs landing page demo asset and CTA targets.
    ValidateLandingPage,
    /// Pre-release sanity suite: schema up-to-date, rules-index in sync,
    /// every runbook spec valid.
    PreRelease,
}

const LANDING_PAGE_PATH: &str = "docs/src/introduction.md";
const LANDING_DEMO_ASSET_PATH: &str = "docs/src/demo-terminal.svg";
const LANDING_DEMO_LIMIT_BYTES: u64 = 2 * 1024 * 1024;
const REQUIRED_DEMO_EMBED_SRCS: &[&str] = &["demo-terminal.svg"];
const REQUIRED_INSTALL_CTA_LINKS: &[&str] = &[
    "./install.md#install-script-macos--linux--windows",
    "./install.md#cargo",
    "./install.md#homebrew",
    "./install.md#build-from-source",
];

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let _ = writeln!(std::io::stderr(), "error: {err:#}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.cmd {
        Cmd::Schema { out } => emit_schema(&out),
        Cmd::SyncRulesIndex => sync_rules_index(),
        Cmd::ValidateRunbooks { dir } => validate_runbooks(&dir),
        Cmd::ValidateLandingPage => validate_landing_page(),
        Cmd::PreRelease => pre_release(),
    }
}

fn emit_schema(out: &Path) -> Result<()> {
    let schema = plumb_config::emit_schema().map_err(|e| anyhow::anyhow!("{e}"))?;
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(out, &schema).with_context(|| format!("write {}", out.display()))?;
    let _ = writeln!(
        std::io::stdout(),
        "▸ wrote {} ({} bytes)",
        out.display(),
        schema.len()
    );
    Ok(())
}

fn sync_rules_index() -> Result<()> {
    let rules = plumb_core::register_builtin();
    let missing: Vec<String> = rules
        .iter()
        .filter_map(|r| {
            let slug = r.id().replace('/', "-");
            let path = PathBuf::from(format!("docs/src/rules/{slug}.md"));
            if path.exists() {
                None
            } else {
                Some(r.id().to_owned())
            }
        })
        .collect();
    if !missing.is_empty() {
        anyhow::bail!(
            "rules without docs: {missing:?}. Run `plumb explain <id>` or create the docs pages."
        );
    }
    let _ = writeln!(
        std::io::stdout(),
        "▸ {} built-in rule(s); all have docs pages.",
        rules.len()
    );
    Ok(())
}

fn validate_runbooks(dir: &Path) -> Result<()> {
    if !dir.exists() {
        // Empty runbooks dir is valid — nothing to check yet.
        let _ = writeln!(
            std::io::stdout(),
            "▸ {} does not exist; 0 runbook specs to validate.",
            dir.display()
        );
        return Ok(());
    }

    let script = PathBuf::from(".agents/skills/gh-runbook/scripts/generate_runbook.py");
    if !script.exists() {
        anyhow::bail!(
            "gh-runbook generator script missing at {}; cannot validate specs.",
            script.display()
        );
    }

    let mut specs: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("read dir {}", dir.display()))? {
        let entry = entry.context("read_dir entry")?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|e| e == "yaml" || e == "yml") {
            specs.push(path);
        }
    }
    specs.sort();

    if specs.is_empty() {
        let _ = writeln!(
            std::io::stdout(),
            "▸ 0 runbook specs under {}; nothing to validate.",
            dir.display()
        );
        return Ok(());
    }

    let mut failures: Vec<String> = Vec::new();
    for spec in &specs {
        let output = Command::new("python3")
            .arg(&script)
            .arg(spec)
            .arg("--validate-only")
            .output()
            .with_context(|| format!("invoke generator on {}", spec.display()))?;
        if output.status.success() {
            let _ = writeln!(std::io::stdout(), "  ok: {}", spec.display());
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            failures.push(format!("{}: {}", spec.display(), stderr.trim()));
        }
    }

    if !failures.is_empty() {
        for f in &failures {
            let _ = writeln!(std::io::stderr(), "  fail: {f}");
        }
        anyhow::bail!("{} runbook spec(s) failed validation", failures.len());
    }

    let _ = writeln!(
        std::io::stdout(),
        "▸ {} runbook spec(s) valid.",
        specs.len()
    );
    Ok(())
}

fn pre_release() -> Result<()> {
    let _ = writeln!(std::io::stdout(), "▸ Pre-release sanity suite");

    // 1. Schema is up-to-date.
    let current = plumb_config::emit_schema().map_err(|e| anyhow::anyhow!("{e}"))?;
    let committed_path = PathBuf::from("schemas/plumb.toml.json");
    if committed_path.exists() {
        let committed = std::fs::read_to_string(&committed_path)
            .with_context(|| format!("read {}", committed_path.display()))?;
        if committed.trim() != current.trim() {
            anyhow::bail!(
                "schemas/plumb.toml.json is stale — run `cargo xtask schema` and commit."
            );
        }
    }

    // 2. Rules index in sync.
    sync_rules_index()?;

    // 3. Landing page demo asset + CTA targets valid.
    validate_landing_page()?;

    // 4. Runbook specs valid (skips cleanly if docs/runbooks/ doesn't exist yet).
    validate_runbooks(Path::new("docs/runbooks"))?;

    let _ = writeln!(std::io::stdout(), "▸ OK — pre-release gates green.");
    Ok(())
}

fn validate_landing_page() -> Result<()> {
    let landing_page = Path::new(LANDING_PAGE_PATH);
    let landing_src = std::fs::read_to_string(landing_page)
        .with_context(|| format!("read {}", landing_page.display()))?;

    let demo_asset = Path::new(LANDING_DEMO_ASSET_PATH);
    if !demo_asset.exists() {
        anyhow::bail!(
            "landing page demo asset missing at {}; add a checked-in asset.",
            demo_asset.display()
        );
    }

    let demo_size = std::fs::metadata(demo_asset)
        .with_context(|| format!("stat {}", demo_asset.display()))?
        .len();
    if demo_size > LANDING_DEMO_LIMIT_BYTES {
        anyhow::bail!(
            "landing page demo asset is {demo_size} bytes; limit is {LANDING_DEMO_LIMIT_BYTES} bytes."
        );
    }

    validate_no_remote_embeds(landing_page, &landing_src)?;

    let embed_sources = extract_html_sources(&landing_src);
    for src in REQUIRED_DEMO_EMBED_SRCS {
        if !embed_sources.iter().any(|candidate| candidate == src) {
            anyhow::bail!("landing page is missing required demo embed source `{src}`.");
        }
    }
    for src in &embed_sources {
        validate_local_markdown_link(landing_page, src)?;
    }

    let links = extract_markdown_links(&landing_src);
    for href in REQUIRED_INSTALL_CTA_LINKS {
        if !links.iter().any(|link| link == href) {
            anyhow::bail!("landing page is missing required install CTA link `{href}`.");
        }
    }

    for href in &links {
        validate_local_markdown_link(landing_page, href)?;
    }

    let _ = writeln!(
        std::io::stdout(),
        "▸ landing page valid: demo asset present ({} bytes), no remote embeds, {} local link(s) checked.",
        demo_size,
        links.len()
    );
    Ok(())
}

fn validate_no_remote_embeds(path: &Path, src: &str) -> Result<()> {
    for (index, raw_line) in src.lines().enumerate() {
        let line = raw_line.trim();
        let has_embed_tag = ["<img", "<video", "<iframe", "<embed", "<object", "<source"]
            .iter()
            .any(|tag| line.contains(tag));
        if has_embed_tag
            && [
                "src=\"http",
                "src='http",
                "data=\"http",
                "data='http",
                "poster=\"http",
                "poster='http",
            ]
            .iter()
            .any(|pattern| line.contains(pattern))
        {
            anyhow::bail!(
                "{}:{} contains a remote embed; use checked-in docs assets only.",
                path.display(),
                index + 1
            );
        }
    }
    Ok(())
}

fn validate_local_markdown_link(base_file: &Path, href: &str) -> Result<()> {
    if href.starts_with("http://")
        || href.starts_with("https://")
        || href.starts_with("mailto:")
        || href.starts_with('#')
    {
        return Ok(());
    }

    let (relative_target, anchor) = href
        .split_once('#')
        .map_or((href, None), |(path, anchor)| (path, Some(anchor)));
    let target_path = base_file
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(relative_target);
    if !target_path.exists() {
        anyhow::bail!(
            "landing page link `{href}` points to missing file `{}`.",
            target_path.display()
        );
    }

    if let Some(anchor) = anchor {
        let target_src = std::fs::read_to_string(&target_path)
            .with_context(|| format!("read {}", target_path.display()))?;
        let anchors = collect_markdown_anchors(&target_src);
        if !anchors.iter().any(|candidate| candidate == anchor) {
            anyhow::bail!(
                "landing page link `{href}` points to missing anchor `#{anchor}` in `{}`.",
                target_path.display()
            );
        }
    }

    Ok(())
}

fn extract_markdown_links(src: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut in_code_fence = false;

    for line in src.lines() {
        if line.trim_start().starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence {
            continue;
        }

        let mut rest = line;
        while let Some(start) = rest.find("](") {
            let after = &rest[start + 2..];
            if let Some(end) = after.find(')') {
                links.push(after[..end].to_owned());
                rest = &after[end + 1..];
            } else {
                break;
            }
        }
    }

    links
}

fn extract_html_sources(src: &str) -> Vec<String> {
    let mut sources = Vec::new();

    for line in src.lines() {
        let mut rest = line;
        while let Some(start) = rest.find("src=") {
            let after = &rest[start + 4..];
            let Some(quote) = after.chars().next() else {
                break;
            };
            if quote != '"' && quote != '\'' {
                rest = after;
                continue;
            }

            let value_start = quote.len_utf8();
            let value = &after[value_start..];
            if let Some(end) = value.find(quote) {
                sources.push(value[..end].to_owned());
                rest = &value[end + quote.len_utf8()..];
            } else {
                break;
            }
        }
    }

    sources
}

fn collect_markdown_anchors(src: &str) -> Vec<String> {
    let mut anchors = Vec::new();
    let mut in_code_fence = false;

    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence || !trimmed.starts_with('#') {
            continue;
        }

        let heading = trimmed.trim_start_matches('#').trim();
        if !heading.is_empty() {
            anchors.push(markdown_anchor_slug(heading));
        }
    }

    anchors
}

fn markdown_anchor_slug(heading: &str) -> String {
    heading
        .chars()
        .flat_map(char::to_lowercase)
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() {
                Some(ch)
            } else if ch.is_ascii_whitespace() || ch == '-' {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        collect_markdown_anchors, extract_html_sources, extract_markdown_links,
        markdown_anchor_slug, validate_no_remote_embeds,
    };
    use std::path::Path;

    #[test]
    fn heading_slug_matches_install_anchor_shape() {
        let slug = markdown_anchor_slug("Install script (macOS / Linux / Windows)");
        assert_eq!(slug, "install-script-macos--linux--windows");
    }

    #[test]
    fn extracts_markdown_links_outside_code_fences() {
        let src = r"
[Install](./install.md#cargo)

```md
[ignored](./ignored.md)
```

[Quick start](./quickstart.md)
";
        let links = extract_markdown_links(src);
        assert_eq!(links, vec!["./install.md#cargo", "./quickstart.md"]);
    }

    #[test]
    fn extracts_html_sources() {
        let src = r#"
<p align="center">
  <img src="demo-terminal.svg" alt="demo" width="720" />
  <source src='clip.webm' type="video/webm" />
</p>
"#;
        let sources = extract_html_sources(src);
        assert_eq!(sources, vec!["demo-terminal.svg", "clip.webm"]);
    }

    #[test]
    fn collects_heading_anchors_outside_code_fences() {
        let src = r"
# Install
## Cargo

```md
# Ignored
```
";
        let anchors = collect_markdown_anchors(src);
        assert_eq!(anchors, vec!["install", "cargo"]);
    }

    #[test]
    fn rejects_remote_embeds() {
        let err = validate_no_remote_embeds(
            Path::new("docs/src/introduction.md"),
            r#"<img src="https://example.com/demo.svg" alt="bad" />"#,
        )
        .expect_err("remote embeds must fail");
        assert!(err.to_string().contains("remote embed"));
    }
}
