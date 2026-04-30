//! # plumb
//!
//! Plumb command-line interface. The binary entry point — the only crate
//! in the workspace that prints to stdout/stderr, and the only place
//! `anyhow` is permitted.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]
// The CLI is the one place wall-clock time is legitimately allowed
// (startup logs, --verbose timings). The engine and libraries forbid it.
#![allow(clippy::disallowed_methods)]
// Binary crate — nothing is "reachable" externally, and `pub(crate)`
// trips `redundant_pub_crate` (clippy) while bare `pub` trips
// `unreachable_pub` (rustc). Allow the former and keep everything bare `pub`.
#![allow(unreachable_pub)]

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use tracing_subscriber::EnvFilter;

mod commands;

/// Plumb — deterministic design-system linter for rendered websites.
#[derive(Debug, Parser)]
#[command(
    name = "plumb",
    version,
    about = "Deterministic design-system linter for rendered websites.",
    long_about = None,
    propagate_version = true,
)]
struct Cli {
    /// Increase log verbosity. Pass `-v` for debug, `-vv` for trace.
    #[arg(short, long, action = ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Lint a URL against your design-system spec.
    Lint {
        /// URL to lint. The `plumb-fake://hello` scheme exercises the
        /// deterministic fake driver.
        url: String,
        /// Path to the config file. Defaults to `plumb.toml` in CWD.
        #[arg(long, short = 'c')]
        config: Option<PathBuf>,
        /// Explicit Chrome or Chromium executable path.
        #[arg(long, value_name = "PATH")]
        executable_path: Option<PathBuf>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = Format::Pretty)]
        format: Format,
        /// Write the rendered lint output to a file instead of stdout.
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,
        /// Restrict the run to the named viewports (repeatable).
        ///
        /// Defaults to every viewport configured in `plumb.toml`, or
        /// to a single 1280x800 `desktop` viewport when none are
        /// configured.
        #[arg(long = "viewport", value_name = "NAME", action = ArgAction::Append)]
        viewports: Vec<String>,
        /// Restrict linting to elements matching this CSS selector and
        /// their descendants. When provided, snapshots are filtered
        /// before rule dispatch.
        #[arg(long, value_name = "CSS_SELECTOR")]
        selector: Option<String>,
    },
    /// Write a starter `plumb.toml` to the current directory.
    Init {
        /// Overwrite an existing `plumb.toml`.
        #[arg(long)]
        force: bool,
    },
    /// Print the long-form documentation for a rule.
    Explain {
        /// Rule id, e.g. `spacing/grid-conformance`.
        rule: String,
    },
    /// Emit the JSON Schema for `plumb.toml` on stdout.
    Schema,
    /// Run the MCP server on stdio.
    Mcp,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Format {
    /// Human-readable TTY output.
    Pretty,
    /// Canonical pretty-printed JSON.
    Json,
    /// SARIF 2.1.0.
    Sarif,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose);
    miette::set_panic_hook();

    match run(cli) {
        Ok(code) => code,
        Err(err) => {
            let _ = report_error(&err);
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(anyhow::Error::from)?;

    rt.block_on(async move {
        match cli.command {
            Command::Lint {
                url,
                config,
                executable_path,
                format,
                output,
                viewports,
                selector,
            } => {
                commands::lint::run(
                    url,
                    config,
                    executable_path,
                    format.into(),
                    output,
                    viewports,
                    selector,
                )
                .await
            }
            Command::Init { force } => commands::init::run(force),
            Command::Explain { rule } => commands::explain::run(&rule),
            Command::Schema => commands::schema::run(),
            Command::Mcp => commands::mcp::run().await,
        }
    })
}

fn init_tracing(verbose: u8) {
    let default_filter = match verbose {
        0 => "plumb=info,warn",
        1 => "plumb=debug,warn",
        _ => "plumb=trace,debug",
    };
    let env = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    let use_ansi = std::io::stderr().is_terminal();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(env)
        .with_writer(std::io::stderr)
        .with_ansi(use_ansi)
        .try_init();
}

fn report_error(err: &anyhow::Error) -> std::io::Result<()> {
    use std::io::Write;
    let mut stderr = std::io::stderr().lock();
    writeln!(stderr, "error: {err}")?;
    for cause in err.chain().skip(1) {
        writeln!(stderr, "  caused by: {cause}")?;
    }
    Ok(())
}
