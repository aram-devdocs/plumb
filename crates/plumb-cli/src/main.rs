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

// `Lint` carries 16 inline fields (#74-#77 PRD §15 capture knobs).
// Boxing them would force clap field bindings to thread through `Box<T>`,
// breaking the macro ergonomics, and the enum is constructed exactly once
// per process (in `Cli::parse`). Allow the size disparity here.
#[allow(clippy::large_enum_variant)]
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
        /// Wait for a CSS selector to appear in the page before
        /// capturing the snapshot. Useful for SPAs whose first paint
        /// arrives after the initial network idle event. Compatible
        /// with `--wait-ms`; the selector wait runs first, then the
        /// additional sleep.
        #[arg(long, value_name = "CSS_SELECTOR")]
        wait_for: Option<String>,
        /// Sleep N milliseconds after navigation (and after
        /// `--wait-for`, if both are passed) before capturing the
        /// snapshot. Belt-and-suspenders for SPAs whose deferred
        /// rendering does not finish in the same tick.
        #[arg(long, value_name = "MS")]
        wait_ms: Option<u64>,
        /// Pre-set a cookie before navigation, in `name=value` form.
        /// Repeatable.
        #[arg(long = "cookie", value_name = "NAME=VALUE", action = ArgAction::Append)]
        cookies: Vec<String>,
        /// Add an extra HTTP header to every outgoing request, in
        /// `name: value` form. Repeatable.
        #[arg(long = "header", value_name = "NAME:VALUE", action = ArgAction::Append)]
        headers: Vec<String>,
        /// Path to a JavaScript file evaluated on every new document
        /// before navigation (CDP `Page.addScriptToEvaluateOnNewDocument`).
        /// Used for setting `window.localStorage` or attaching
        /// `Authorization` headers via `fetch` interception. Refused
        /// when the path resolves outside the current working directory.
        #[arg(long, value_name = "PATH")]
        auth_script: Option<PathBuf>,
        /// Path to a Playwright `storage-state.json`. Cookies in the
        /// file are installed before navigation; localStorage entries
        /// for the matching origin are written via `evaluate` after
        /// navigation.
        #[arg(long, value_name = "PATH")]
        storage_state: Option<PathBuf>,
        /// Disable CSS animations and transitions before navigation
        /// for byte-stable snapshots. The driver already does this by
        /// default; pass `--disable-animations false` to opt out.
        #[arg(
            long = "disable-animations",
            default_value_t = true,
            action = ArgAction::Set,
            num_args = 0..=1,
            default_missing_value = "true"
        )]
        disable_animations: bool,
        /// Inject CSS that hides scrollbars before navigation. The
        /// driver already does this by default; pass
        /// `--hide-scrollbars false` to opt out.
        #[arg(
            long = "hide-scrollbars",
            default_value_t = true,
            action = ArgAction::Set,
            num_args = 0..=1,
            default_missing_value = "true"
        )]
        hide_scrollbars: bool,
        /// Pin the device-pixel ratio used by
        /// `Emulation.setDeviceMetricsOverride`. When unset, the
        /// configured viewport's `device_pixel_ratio` is used. Useful
        /// for stress-testing rules against hidpi rendering.
        #[arg(long, value_name = "FACTOR")]
        dpr: Option<f64>,
        /// Opt in to downloading Chrome-for-Testing into Plumb's cache
        /// directory when no `--executable-path` is given and no
        /// system Chromium is detected. The download happens once;
        /// subsequent runs reuse the cached binary and verify its
        /// SHA-256 against an installed sidecar. Off by default —
        /// auto-fetch downloads and executes a third-party binary,
        /// so passing this flag is the explicit acknowledgement of
        /// trust.
        #[arg(long = "auto-fetch-chromium", default_value_t = false)]
        auto_fetch_chromium: bool,
    },
    /// Write a starter `plumb.toml` to the current directory.
    Init {
        /// Overwrite an existing `plumb.toml`.
        #[arg(long)]
        force: bool,
        /// Infer the starter config from a project tree. The walker
        /// discovers CSS custom properties, Tailwind configs, and DTCG
        /// token JSON files under the given path and bootstraps a
        /// `plumb.toml` from them.
        #[arg(long, value_name = "PATH")]
        from: Option<PathBuf>,
    },
    /// Print the long-form documentation for a rule.
    Explain {
        /// Rule id, e.g. `spacing/grid-conformance`.
        rule: String,
    },
    /// Emit the JSON Schema for `plumb.toml` on stdout.
    Schema,
    /// Run the MCP server on stdio or HTTP.
    Mcp {
        /// MCP transport. Defaults to stdio to preserve existing behavior.
        #[arg(long, value_enum, default_value_t = McpTransport::Stdio)]
        transport: McpTransport,
        /// TCP port for the HTTP transport.
        #[arg(long, default_value_t = 4242)]
        port: u16,
    },
    /// Watch the local source tree and re-run `plumb lint` on changes.
    ///
    /// Mirrors `plumb lint`'s flags. Filesystem events are debounced
    /// with a 250 ms window so a single cycle covers a burst of edits
    /// (editor save → temp swap → atomic rename). Press Ctrl-C to exit.
    Watch {
        /// URL to lint on each cycle. Defaults to `plumb-fake://hello`
        /// so a fresh checkout demos the loop without a Chromium binary.
        #[arg(default_value = "plumb-fake://hello")]
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
        /// Restrict the run to the named viewports (repeatable).
        #[arg(long = "viewport", value_name = "NAME", action = ArgAction::Append)]
        viewports: Vec<String>,
        /// Restrict linting to elements matching this CSS selector.
        #[arg(long, value_name = "CSS_SELECTOR")]
        selector: Option<String>,
        /// Wait for a CSS selector to appear in the page before
        /// capturing the snapshot.
        #[arg(long, value_name = "CSS_SELECTOR")]
        wait_for: Option<String>,
        /// Sleep N milliseconds after navigation before capturing.
        #[arg(long, value_name = "MS")]
        wait_ms: Option<u64>,
        /// Pre-set a cookie before navigation, in `name=value` form.
        #[arg(long = "cookie", value_name = "NAME=VALUE", action = ArgAction::Append)]
        cookies: Vec<String>,
        /// Add an extra HTTP header to every outgoing request.
        #[arg(long = "header", value_name = "NAME:VALUE", action = ArgAction::Append)]
        headers: Vec<String>,
        /// Path to a JavaScript file evaluated on every new document
        /// before navigation.
        #[arg(long, value_name = "PATH")]
        auth_script: Option<PathBuf>,
        /// Path to a Playwright `storage-state.json`.
        #[arg(long, value_name = "PATH")]
        storage_state: Option<PathBuf>,
        /// Disable CSS animations and transitions before navigation.
        #[arg(
            long = "disable-animations",
            default_value_t = true,
            action = ArgAction::Set,
            num_args = 0..=1,
            default_missing_value = "true"
        )]
        disable_animations: bool,
        /// Inject CSS that hides scrollbars before navigation.
        #[arg(
            long = "hide-scrollbars",
            default_value_t = true,
            action = ArgAction::Set,
            num_args = 0..=1,
            default_missing_value = "true"
        )]
        hide_scrollbars: bool,
        /// Pin the device-pixel ratio used by the driver.
        #[arg(long, value_name = "FACTOR")]
        dpr: Option<f64>,
        /// Directory to watch. Repeatable. Defaults to the current
        /// working directory when absent.
        #[arg(long = "path", value_name = "PATH", action = ArgAction::Append)]
        watch_paths: Vec<PathBuf>,
        /// Run a single lint cycle and exit instead of entering the
        /// filesystem watch loop. Hidden — exists for integration tests
        /// and ad-hoc shell use.
        #[arg(long, hide = true)]
        once: bool,
    },
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

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
enum McpTransport {
    /// Serve MCP over stdin/stdout.
    Stdio,
    /// Serve MCP over Streamable HTTP.
    Http,
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
                wait_for,
                wait_ms,
                cookies,
                headers,
                auth_script,
                storage_state,
                disable_animations,
                hide_scrollbars,
                dpr,
                auto_fetch_chromium,
            } => {
                commands::lint::run(commands::lint::LintArgs {
                    url,
                    config_path: config,
                    executable_path,
                    format: format.into(),
                    output_path: output,
                    viewports,
                    selector,
                    wait_for,
                    wait_ms,
                    cookies,
                    headers,
                    auth_script,
                    storage_state,
                    disable_animations,
                    hide_scrollbars,
                    dpr,
                    auto_fetch_chromium,
                })
                .await
            }
            Command::Init { force, from } => commands::init::run(force, from.as_deref()),
            Command::Explain { rule } => commands::explain::run(&rule),
            Command::Schema => commands::schema::run(),
            Command::Mcp { transport, port } => commands::mcp::run(transport, port).await,
            Command::Watch {
                url,
                config,
                executable_path,
                format,
                viewports,
                selector,
                wait_for,
                wait_ms,
                cookies,
                headers,
                auth_script,
                storage_state,
                disable_animations,
                hide_scrollbars,
                dpr,
                watch_paths,
                once,
            } => {
                commands::watch::run(commands::watch::WatchArgs {
                    lint: commands::lint::LintArgs {
                        url,
                        config_path: config,
                        executable_path,
                        format: format.into(),
                        output_path: None,
                        viewports,
                        selector,
                        wait_for,
                        wait_ms,
                        cookies,
                        headers,
                        auth_script,
                        storage_state,
                        disable_animations,
                        hide_scrollbars,
                        dpr,
                    },
                    watch_paths,
                    once,
                })
                .await
            }
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
