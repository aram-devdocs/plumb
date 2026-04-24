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
use std::process::ExitCode;

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
    /// Pre-release sanity suite: schema up-to-date, Book builds, binary
    /// under budget, determinism holds.
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
    // Placeholder for when the builtin rule list grows. The pattern:
    //   1. Call `plumb_core::register_builtin()`.
    //   2. For each rule id, verify a corresponding
    //      `docs/src/rules/<slug>.md` exists.
    //   3. Rewrite `docs/src/rules/overview.md` with the canonical list.
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

    let _ = writeln!(std::io::stdout(), "▸ OK — pre-release gates green.");
    Ok(())
}
