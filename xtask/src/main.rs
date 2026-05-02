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
use serde::Deserialize;

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
    /// Validate the checked-in offline release-readiness local kits.
    ValidateReleaseReadinessKits,
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
const RELEASE_READINESS_README_PATH: &str = "tests/fixtures/release-readiness/README.md";
const RELEASE_READINESS_MANIFEST_PATH: &str = "tests/fixtures/release-readiness/manifest.json";
const REQUIRED_RELEASE_READINESS_KITS: &[&str] = &[
    "minimal",
    "large-dom",
    "responsive",
    "typography",
    "contrast",
    "shadow-z-opacity-padding",
    "dynamic-wait",
    "auth-storage",
    "mcp-inputs",
];
const DISALLOWED_KIT_PATTERNS: &[&str] = &[
    "http://",
    "https://",
    "Date.now",
    "Math.random",
    "performance.now",
    "new Date(",
    "crypto.randomUUID",
    "setTimeout(",
    "fetch(",
    "XMLHttpRequest",
    "WebSocket",
    "EventSource",
    "navigator.serviceWorker",
    "SharedWorker",
    "importScripts(",
];
const REQUIRED_MCP_TOOLS: &[&str] = &["echo", "get_config", "lint_url"];

#[derive(Debug, Deserialize)]
struct ReleaseReadinessManifest {
    version: u64,
    location: String,
    offline_only: bool,
    deterministic: bool,
    shared_gate_targets: Vec<String>,
    kits: Vec<ReleaseReadinessKit>,
}

#[derive(Debug, Deserialize)]
struct ReleaseReadinessKit {
    name: String,
    files: Vec<String>,
    purpose: String,
    cli_examples: Vec<String>,
    mcp_examples: Vec<String>,
}

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
        Cmd::ValidateReleaseReadinessKits => validate_release_readiness_kits(),
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

    // 5. Local release-readiness kits valid.
    validate_release_readiness_kits()?;

    let _ = writeln!(std::io::stdout(), "▸ OK — pre-release gates green.");
    Ok(())
}

fn validate_release_readiness_kits() -> Result<()> {
    let workspace_root = workspace_root();
    let readme_path = workspace_root.join(RELEASE_READINESS_README_PATH);
    let manifest_path = workspace_root.join(RELEASE_READINESS_MANIFEST_PATH);

    validate_release_readiness_readme(&readme_path)?;
    let manifest = load_release_readiness_manifest(&manifest_path)?;
    validate_release_readiness_manifest_header(
        &manifest,
        &manifest_path,
        &workspace_root,
        &readme_path,
    )?;

    let mut names: Vec<&str> = manifest.kits.iter().map(|kit| kit.name.as_str()).collect();
    names.sort_unstable();
    let mut expected = REQUIRED_RELEASE_READINESS_KITS.to_vec();
    expected.sort_unstable();
    if names != expected {
        anyhow::bail!(
            "{} must define exactly the required kit set: {:?}; found {:?}.",
            manifest_path.display(),
            expected,
            names
        );
    }

    for kit in &manifest.kits {
        validate_release_readiness_kit(kit, &workspace_root)?;
    }

    let _ = writeln!(
        std::io::stdout(),
        "▸ release-readiness local kits valid: {} kits, offline-only and reusable by CLI/MCP.",
        manifest.kits.len()
    );
    Ok(())
}

fn validate_release_readiness_readme(readme_path: &Path) -> Result<()> {
    let readme = std::fs::read_to_string(readme_path)
        .with_context(|| format!("read {}", readme_path.display()))?;
    for phrase in [
        "offline-only",
        "deterministic",
        "CLI",
        "MCP",
        "manifest.json",
    ] {
        if !readme.contains(phrase) {
            anyhow::bail!(
                "{} must mention `{phrase}` so the kit contract stays documented.",
                readme_path.display()
            );
        }
    }
    Ok(())
}

fn load_release_readiness_manifest(manifest_path: &Path) -> Result<ReleaseReadinessManifest> {
    let manifest_src = std::fs::read_to_string(manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;
    serde_json::from_str(&manifest_src)
        .with_context(|| format!("parse {}", manifest_path.display()))
}

fn validate_release_readiness_manifest_header(
    manifest: &ReleaseReadinessManifest,
    manifest_path: &Path,
    workspace_root: &Path,
    readme_path: &Path,
) -> Result<()> {
    if manifest.version != 1 {
        anyhow::bail!(
            "{} must stay at version 1 until this validator is updated for a new manifest schema.",
            manifest_path.display()
        );
    }
    if manifest.location != "tests/fixtures/release-readiness" {
        anyhow::bail!(
            "{} has unexpected location `{}`.",
            manifest_path.display(),
            manifest.location
        );
    }
    if !manifest.offline_only || !manifest.deterministic {
        anyhow::bail!(
            "{} must declare offline_only=true and deterministic=true.",
            manifest_path.display()
        );
    }
    if manifest.shared_gate_targets != ["cli", "mcp"] {
        anyhow::bail!(
            "{} must declare shared_gate_targets exactly as [\"cli\", \"mcp\"].",
            manifest_path.display()
        );
    }
    if workspace_root.join(&manifest.location) != readme_path.parent().unwrap_or(workspace_root) {
        anyhow::bail!(
            "{} must point at the checked-in local-kit directory only.",
            manifest_path.display()
        );
    }
    Ok(())
}

fn validate_release_readiness_kit(kit: &ReleaseReadinessKit, workspace_root: &Path) -> Result<()> {
    if kit.files.is_empty() {
        anyhow::bail!("kit `{}` must list at least one checked-in file.", kit.name);
    }
    if kit.purpose.trim().is_empty() {
        anyhow::bail!("kit `{}` must include a non-empty purpose.", kit.name);
    }
    if kit.cli_examples.is_empty() || kit.mcp_examples.is_empty() {
        anyhow::bail!(
            "kit `{}` must include both CLI and MCP reuse metadata.",
            kit.name
        );
    }
    validate_release_readiness_kit_contract(kit)?;
    validate_release_readiness_examples(kit)?;
    validate_release_readiness_kit_files(kit, workspace_root)
}

fn validate_release_readiness_kit_contract(kit: &ReleaseReadinessKit) -> Result<()> {
    if kit.name == "dynamic-wait"
        && (!kit.purpose.contains("wait-ms >= 50") || !kit.purpose.contains(".ready-card"))
    {
        anyhow::bail!(
            "kit `dynamic-wait` must document the `wait-ms >= 50` / `.ready-card` capture contract."
        );
    }
    Ok(())
}

fn validate_release_readiness_examples(kit: &ReleaseReadinessKit) -> Result<()> {
    for example in &kit.cli_examples {
        if !example.contains("file://") && !example.contains("cat ") {
            anyhow::bail!(
                "kit `{}` has a CLI example that does not stay local/offline: `{example}`.",
                kit.name
            );
        }
    }
    for example in &kit.mcp_examples {
        if !example.contains("lint_url")
            && !example.contains("get_config")
            && !example.contains("echo")
        {
            anyhow::bail!(
                "kit `{}` has an MCP example that does not reference a current MCP surface: `{example}`.",
                kit.name
            );
        }
    }
    Ok(())
}

fn validate_release_readiness_kit_files(
    kit: &ReleaseReadinessKit,
    workspace_root: &Path,
) -> Result<()> {
    for file in &kit.files {
        let path = workspace_root.join(file);
        if !path.exists() {
            anyhow::bail!("kit `{}` references missing file `{file}`.", kit.name);
        }
        validate_release_readiness_file(&path)?;
    }
    Ok(())
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn validate_release_readiness_file(path: &Path) -> Result<()> {
    let src = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;

    for pattern in DISALLOWED_KIT_PATTERNS {
        if *pattern == "setTimeout(" && is_dynamic_wait_fixture(path) {
            continue;
        }
        if src.contains(pattern) {
            anyhow::bail!(
                "{} contains disallowed offline/determinism pattern `{pattern}`.",
                path.display()
            );
        }
    }
    if src.contains("<link ") {
        anyhow::bail!(
            "{} contains a <link> tag; use inline checked-in styling only.",
            path.display()
        );
    }
    if src.contains("<img ") || src.contains("<iframe ") || src.contains("<object ") {
        anyhow::bail!(
            "{} contains an externalizable embed tag; keep the kits self-contained text/CSS/DOM fixtures.",
            path.display()
        );
    }
    if path
        .file_name()
        .is_some_and(|name| name == "mcp-inputs.json")
    {
        validate_release_readiness_mcp_inputs(&src, path)?;
    }
    if is_dynamic_wait_fixture(path) {
        validate_dynamic_wait_fixture(&src, path)?;
    }
    if path.extension().is_some_and(|ext| ext == "html") && !src.contains("<!DOCTYPE html>") {
        anyhow::bail!("{} must remain an HTML5 fixture.", path.display());
    }
    Ok(())
}

fn is_dynamic_wait_fixture(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name == "dynamic-wait.html")
}

fn validate_dynamic_wait_fixture(src: &str, path: &Path) -> Result<()> {
    if !src.contains("setTimeout(") || !src.contains("}, 50);") {
        anyhow::bail!(
            "{} must keep exactly one explicit 50 ms delayed mutation for wait-gate coverage.",
            path.display()
        );
    }
    if !src.contains(".ready-card") && !src.contains("ready-card") {
        anyhow::bail!(
            "{} must keep a stable ready marker such as `.ready-card` for wait-for style capture.",
            path.display()
        );
    }
    Ok(())
}

fn validate_release_readiness_mcp_inputs(src: &str, path: &Path) -> Result<()> {
    let value: serde_json::Value =
        serde_json::from_str(src).with_context(|| format!("parse {}", path.display()))?;
    let requests = value
        .get("requests")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("{} must contain a `requests` array.", path.display()))?;

    for tool in REQUIRED_MCP_TOOLS {
        if !requests.iter().any(|entry| {
            entry
                .get("tool")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|name| name == *tool)
        }) {
            anyhow::bail!(
                "{} must include a `{tool}` request so the MCP reuse contract stays covered.",
                path.display()
            );
        }
    }

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
    let mut in_code_fence = false;

    for (index, raw_line) in src.lines().enumerate() {
        if raw_line.trim_start().starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence {
            continue;
        }

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
    let mut in_code_fence = false;

    for raw_line in src.lines() {
        if raw_line.trim_start().starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence {
            continue;
        }

        let line = raw_line;
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
        RELEASE_READINESS_MANIFEST_PATH, ReleaseReadinessKit, ReleaseReadinessManifest,
        collect_markdown_anchors, extract_html_sources, extract_markdown_links,
        markdown_anchor_slug, validate_local_markdown_link, validate_no_remote_embeds,
        validate_release_readiness_file, validate_release_readiness_kit_contract, workspace_root,
    };
    use std::{fs, path::Path};
    use tempfile::tempdir;

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
    fn ignores_html_sources_inside_code_fences() {
        let src = r#"
```html
<img src="ignored.svg" alt="ignored" />
```

<img src="demo-terminal.svg" alt="demo" />
"#;
        let sources = extract_html_sources(src);
        assert_eq!(sources, vec!["demo-terminal.svg"]);
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

    #[test]
    fn ignores_remote_embeds_inside_code_fences() {
        validate_no_remote_embeds(
            Path::new("docs/src/introduction.md"),
            r#"
```html
<img src="https://example.com/demo.svg" alt="example" />
```
"#,
        )
        .expect("fenced examples must be ignored");
    }

    #[test]
    fn validates_local_markdown_link_when_file_and_anchor_exist() {
        let dir = tempdir().expect("tempdir");
        let landing_page = dir.path().join("index.md");
        let target = dir.path().join("install.md");

        fs::write(&landing_page, "[Install](./install.md#cargo)\n").expect("write landing page");
        fs::write(&target, "# Install\n## Cargo\n").expect("write target markdown");

        validate_local_markdown_link(&landing_page, "./install.md#cargo")
            .expect("existing file and anchor must pass");
    }

    #[test]
    fn rejects_local_markdown_link_when_anchor_does_not_match() {
        let dir = tempdir().expect("tempdir");
        let landing_page = dir.path().join("index.md");
        let target = dir.path().join("install.md");

        fs::write(&landing_page, "[Install](./install.md#cargo)\n").expect("write landing page");
        fs::write(&target, "# Install\n## Quick start\n").expect("write target markdown");

        let err = validate_local_markdown_link(&landing_page, "./install.md#cargo")
            .expect_err("anchor mismatch must fail");
        assert!(err.to_string().contains("missing anchor `#cargo`"));
    }

    #[test]
    fn release_readiness_manifest_parses() {
        let path = workspace_root().join(RELEASE_READINESS_MANIFEST_PATH);
        let src = fs::read_to_string(path).expect("read manifest");
        let manifest: ReleaseReadinessManifest =
            serde_json::from_str(&src).expect("manifest should parse");
        assert_eq!(manifest.version, 1);
        assert!(!manifest.kits.is_empty(), "kits should not be empty");
        let dynamic_wait = manifest
            .kits
            .iter()
            .find(|kit| kit.name == "dynamic-wait")
            .expect("dynamic-wait kit present");
        assert!(dynamic_wait.purpose.contains("wait-ms >= 50"));
        assert!(dynamic_wait.purpose.contains(".ready-card"));
    }

    #[test]
    fn release_readiness_mcp_inputs_must_cover_required_tools() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("mcp-inputs.json");
        fs::write(
            &path,
            r#"{"requests":[{"tool":"echo"},{"tool":"get_config"}]}"#,
        )
        .expect("write mcp inputs");
        let err = validate_release_readiness_file(&path).expect_err("missing lint_url must fail");
        assert!(err.to_string().contains("lint_url"));
    }

    #[test]
    fn release_readiness_file_rejects_remote_url() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("remote.html");
        fs::write(
            &path,
            "<!DOCTYPE html><img src=\"https://example.com/x.png\" alt=\"x\" />",
        )
        .expect("write fixture");
        let err = validate_release_readiness_file(&path).expect_err("remote URL must fail");
        assert!(err.to_string().contains("https://"));
    }

    #[test]
    fn release_readiness_dynamic_wait_contract_requires_wait_note() {
        let kit = ReleaseReadinessKit {
            name: "dynamic-wait".to_owned(),
            files: vec!["tests/fixtures/release-readiness/dynamic-wait.html".to_owned()],
            purpose: "deterministic delayed DOM mutation".to_owned(),
            cli_examples: vec!["plumb lint file://fixture --format json".to_owned()],
            mcp_examples: vec!["lint_url {\"url\":\"file://fixture\"}".to_owned()],
        };
        let err = validate_release_readiness_kit_contract(&kit)
            .expect_err("dynamic-wait note must be required");
        assert!(err.to_string().contains("wait-ms >= 50"));
    }

    #[test]
    fn release_readiness_file_rejects_timeout_outside_dynamic_wait_fixture() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("timeout.html");
        fs::write(
            &path,
            "<!DOCTYPE html><script>setTimeout(() => { document.body.dataset.state = \"ready\"; }, 50);</script>",
        )
        .expect("write fixture");
        let err = validate_release_readiness_file(&path)
            .expect_err("setTimeout should be rejected outside dynamic-wait");
        assert!(err.to_string().contains("setTimeout("));
    }
}
