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
    /// Pre-release sanity suite: schema up-to-date, rules-index in sync,
    /// every runbook spec valid.
    PreRelease,
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

    // 3. Runbook specs valid (skips cleanly if docs/runbooks/ doesn't exist yet).
    validate_runbooks(Path::new("docs/runbooks"))?;

    let _ = writeln!(std::io::stdout(), "▸ OK — pre-release gates green.");
    Ok(())
}
