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
use clap::{ArgAction, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use tracing_subscriber::EnvFilter;

mod banner;
mod commands;

/// Plumb — deterministic design-system linter for rendered websites.
#[derive(Debug, Parser)]
#[command(
    name = "plumb",
    version,
    about = "Deterministic design-system linter for rendered websites.",
    long_about = None,
    propagate_version = true,
    // A bare `plumb` (no subcommand) renders the top-level help — and
    // therefore the brand banner — instead of a usage error. clap prints
    // help to stderr and exits 2 on this path, the same exit code the
    // prior "subcommand required" usage error produced.
    arg_required_else_help = true,
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
        /// Minimum severity to show and count toward the exit code.
        ///
        /// One of `info`, `warn`, `error`, `off`. Ordering is
        /// `error` > `warn` > `info`. The default `warn` shows warnings
        /// and errors and hides info; `error` shows only errors; `info`
        /// shows everything; `off` shows nothing and forces exit 0.
        #[arg(long, value_enum, default_value_t = MinSeverity::Warn)]
        min_severity: MinSeverity,
        /// Show only violations from this rule id (repeatable).
        ///
        /// Applied on top of `--min-severity`. Unknown ids simply match
        /// nothing, so a typo yields an empty, clean (exit 0) run.
        #[arg(long = "rule", value_name = "RULE_ID", action = ArgAction::Append)]
        rules: Vec<String>,
        /// Cap the number of findings shown after severity/rule filtering
        /// and the engine's deterministic sort.
        ///
        /// Pretty output gains a footer counting the hidden findings;
        /// JSON sets a top-level `truncated` flag while `summary` keeps
        /// counting the full filtered set. No cap when unset.
        #[arg(long, value_name = "N")]
        max_findings: Option<usize>,
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
        /// Disable CSS animations and transitions before capture
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
        /// Inject CSS that hides scrollbars before capture. The
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
        /// After the normal lint output, append a suggested
        /// `.plumbignore` block listing one entry per
        /// `(rule_id, selector)` tuple that would suppress every
        /// current violation. Helps adopt Plumb gradually on existing
        /// codebases. Pretty format appends a footer; JSON format adds
        /// a `suggested_ignores` array to the envelope; SARIF format is
        /// unchanged.
        #[arg(long, default_value_t = false)]
        suggest_ignores: bool,
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
        /// Minimum severity to show and count toward the exit code.
        /// Mirrors `plumb lint --min-severity`.
        #[arg(long, value_enum, default_value_t = MinSeverity::Warn)]
        min_severity: MinSeverity,
        /// Show only violations from this rule id (repeatable). Mirrors
        /// `plumb lint --rule`.
        #[arg(long = "rule", value_name = "RULE_ID", action = ArgAction::Append)]
        rules: Vec<String>,
        /// Cap the number of findings shown per cycle. Mirrors
        /// `plumb lint --max-findings`.
        #[arg(long, value_name = "N")]
        max_findings: Option<usize>,
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
        /// Disable CSS animations and transitions before capture.
        #[arg(
            long = "disable-animations",
            default_value_t = true,
            action = ArgAction::Set,
            num_args = 0..=1,
            default_missing_value = "true"
        )]
        disable_animations: bool,
        /// Inject CSS that hides scrollbars before capture.
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
        /// After each rendered cycle, append a suggested `.plumbignore`
        /// block listing one entry per `(rule_id, selector)` tuple that
        /// would suppress every current violation. Mirrors
        /// `plumb lint --suggest-ignores`. Pretty format appends a
        /// footer; JSON format adds a `suggested_ignores` array; SARIF
        /// is unchanged.
        #[arg(long, default_value_t = false)]
        suggest_ignores: bool,
        /// Opt in to downloading Chrome-for-Testing into Plumb's cache
        /// directory when no `--executable-path` is given and no
        /// system Chromium is detected. See `plumb lint --help` for the
        /// trust trade-off.
        #[arg(long = "auto-fetch-chromium", default_value_t = false)]
        auto_fetch_chromium: bool,
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

/// Minimum severity that `plumb lint` shows and counts toward the exit
/// code. Ordering is `error` > `warn` > `info`: a violation is kept when
/// its severity is at or above the selected level.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum MinSeverity {
    /// Show everything: info, warnings, and errors.
    Info,
    /// Show warnings and errors; hide info. The default.
    Warn,
    /// Show only errors.
    Error,
    /// Show nothing and always exit 0.
    Off,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
enum McpTransport {
    /// Serve MCP over stdin/stdout.
    Stdio,
    /// Serve MCP over Streamable HTTP.
    Http,
}

fn main() -> ExitCode {
    let cli = match parse_cli() {
        Ok(cli) => cli,
        // `clap::Error::exit` prints help/version/usage to the correct
        // stream and exits with clap's own code (0 for `--help`/`--version`,
        // 2 for usage / missing-subcommand). It diverges, so this arm
        // never falls through.
        Err(err) => err.exit(),
    };
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

/// Parse argv into [`Cli`], attaching the TTY-aware brand banner to the
/// help output first.
///
/// This mirrors `Cli::parse()` (build the command, match argv, hydrate
/// the struct) but injects the banner via [`banner::brand`] between the
/// first two steps so the colour decision is made at runtime rather than
/// baked into a derive attribute. The banner only renders when clap
/// displays help, so it never reaches the `mcp` stdio stream.
fn parse_cli() -> Result<Cli, clap::Error> {
    let command = banner::brand(Cli::command(), banner::should_color());
    let matches = command.try_get_matches()?;
    Cli::from_arg_matches(&matches)
}

// `Command::Lint` and `Command::Watch` carry the full PRD §15 capture-knob
// surface inline (16+ fields each), so the dispatch arm is unavoidably
// long. Splitting it would obscure the 1:1 clap-flag-to-LintArgs/WatchArgs
// mapping a casual reader expects to find in this file. Allow the
// length here rather than fragmenting the readable shape.
#[allow(clippy::too_many_lines)]
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
                min_severity,
                rules,
                max_findings,
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
                suggest_ignores,
                auto_fetch_chromium,
            } => {
                commands::lint::run(commands::lint::LintArgs {
                    url,
                    config_path: config,
                    executable_path,
                    format: format.into(),
                    min_severity: min_severity.into(),
                    rule_ids: rules,
                    max_findings,
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
                    suggest_ignores,
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
                min_severity,
                rules,
                max_findings,
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
                suggest_ignores,
                auto_fetch_chromium,
                watch_paths,
                once,
            } => {
                commands::watch::run(commands::watch::WatchArgs {
                    lint: commands::lint::LintArgs {
                        url,
                        config_path: config,
                        executable_path,
                        format: format.into(),
                        min_severity: min_severity.into(),
                        rule_ids: rules,
                        max_findings,
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
                        suggest_ignores,
                        auto_fetch_chromium,
                    },
                    watch_paths,
                    once,
                })
                .await
            }
        }
    })
}

/// Default `tracing` filter string for a given `--verbose` count.
///
/// The `chromiumoxide::handler=error` override silences the recurring
/// `WS Invalid message: data did not match any variant of untagged enum
/// Message` warning that fires on every successful real-URL lint
/// (issue #244) — it's a benign upstream parser warning for CDP events
/// the handler doesn't model. The broader `chromiumoxide=warn` keeps
/// genuine driver-level warnings visible. `RUST_LOG=trace` (or any
/// explicit `RUST_LOG`) takes over and re-exposes the silenced events
/// for debugging.
fn default_log_filter(verbose: u8) -> &'static str {
    match verbose {
        0 => "plumb=info,warn,chromiumoxide=warn,chromiumoxide::handler=error",
        1 => "plumb=debug,warn,chromiumoxide=warn,chromiumoxide::handler=error",
        _ => "plumb=trace,debug",
    }
}

fn init_tracing(verbose: u8) {
    let default_filter = default_log_filter(verbose);
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

#[cfg(test)]
mod tests {
    use super::default_log_filter;
    use tracing_subscriber::EnvFilter;

    /// Every default-filter string MUST parse as a valid `EnvFilter`
    /// directive. A typo here would silently fall back to the empty
    /// filter at runtime; this test pins each branch to compile time.
    #[test]
    fn default_log_filters_parse_for_each_verbosity() {
        for v in [0u8, 1, 2, 5] {
            let filter = default_log_filter(v);
            EnvFilter::try_new(filter)
                .expect("default tracing filter must be a valid EnvFilter directive");
        }
    }

    /// The default filter MUST silence the chromiumoxide handler
    /// warning that floods stderr on every successful real-URL lint
    /// (issue #244). The override appears in the default-verbosity
    /// branch and the `-v` branch; `-vv` (trace) is intentionally
    /// noisier so debug callers see everything.
    #[test]
    fn default_filter_silences_chromiumoxide_handler_warn() {
        for v in [0u8, 1] {
            let filter = default_log_filter(v);
            assert!(
                filter.contains("chromiumoxide::handler=error"),
                "verbose={v} filter must mute chromiumoxide::handler WARN"
            );
        }
    }
}
