//! `plumb-e2e` — runs the harness against one or all fixtures in
//! `e2e-sites/`.

#![forbid(unsafe_code)]
#![allow(unreachable_pub)]
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context as _;
use clap::Parser;
use plumb_e2e::{
    HarnessConfig, RunReport, find_workspace_root, run_site, sites::SITES, sites::lookup,
};

/// Run the Plumb e2e harness against the framework fixture matrix.
#[derive(Debug, Parser)]
#[command(name = "plumb-e2e", about, version)]
struct Cli {
    /// Comma-or-multi-value list of site slugs to run. Mutually
    /// exclusive with `--all`.
    #[arg(long, value_delimiter = ',', conflicts_with = "all")]
    site: Vec<String>,

    /// Run every site in `e2e-sites/`.
    #[arg(long)]
    all: bool,

    /// Path to the locally built `plumb` binary. Defaults to
    /// `target/release/plumb` if it exists, otherwise `target/debug/plumb`.
    #[arg(long)]
    plumb_bin: Option<PathBuf>,

    /// Optional Chromium executable path passed through to
    /// `plumb lint --executable-path`.
    #[arg(long)]
    chrome_path: Option<PathBuf>,

    /// Skip the `just build` step in each fixture. Use this when the
    /// `dist/` directories are already up to date.
    #[arg(long)]
    no_build: bool,

    /// Number of lint runs to compare for byte-equality. Defaults to 3.
    #[arg(long, default_value_t = 3)]
    determinism_runs: usize,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("plumb_e2e=info")),
        )
        .with_target(false)
        .init();

    match real_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:?}");
            ExitCode::from(1)
        }
    }
}

fn real_main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();
    let workspace_root = find_workspace_root(None)?;
    let plumb_bin = resolve_plumb_bin(&workspace_root, cli.plumb_bin.as_deref())?;

    let mut config = HarnessConfig::new(workspace_root, plumb_bin);
    config.chrome_path = cli.chrome_path;
    config.build_first = !cli.no_build;
    config.determinism_runs = cli.determinism_runs;

    let sites: Vec<String> = if cli.all || cli.site.is_empty() {
        SITES.iter().map(|s| s.name.to_owned()).collect()
    } else {
        for name in &cli.site {
            if lookup(name).is_none() {
                return Err(anyhow::anyhow!(
                    "unknown site `{name}`. Known: {}",
                    SITES.iter().map(|s| s.name).collect::<Vec<_>>().join(", "),
                ));
            }
        }
        cli.site
    };

    let mut reports: Vec<RunReport> = Vec::with_capacity(sites.len());
    let mut failures: Vec<(String, String)> = Vec::new();
    for name in &sites {
        match run_site(name, &config) {
            Ok(report) => {
                println!(
                    "PASS  {name}  target_violations={total}  non_target={non}  by_rule_id={by:?}",
                    total = report.total_target,
                    non = report.non_target,
                    by = report.by_rule_id,
                );
                reports.push(report);
            }
            Err(err) => {
                eprintln!("FAIL  {name}  {err}");
                failures.push((name.clone(), err.to_string()));
            }
        }
    }

    if !failures.is_empty() {
        return Err(anyhow::anyhow!(
            "{n} site(s) failed: {names}",
            n = failures.len(),
            names = failures
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }
    println!("OK    {n} site(s) passed", n = reports.len());
    Ok(())
}

fn resolve_plumb_bin(
    workspace_root: &std::path::Path,
    explicit: Option<&std::path::Path>,
) -> Result<PathBuf, anyhow::Error> {
    if let Some(path) = explicit {
        let abs = if path.is_absolute() {
            path.to_path_buf()
        } else {
            workspace_root.join(path)
        };
        // On Windows, `target/release/plumb` (no `.exe`) won't resolve.
        // Try appending `EXE_SUFFIX` before failing.
        let resolved = if abs.is_file() {
            abs
        } else {
            let with_suffix = with_exe_suffix(&abs);
            if with_suffix.is_file() {
                with_suffix
            } else {
                return Err(anyhow::anyhow!(
                    "plumb binary not found at `{}` (--plumb-bin override)",
                    abs.display(),
                ));
            }
        };
        // Normalize separators so log lines and child-process invocations
        // don't carry the mixed `\` / `/` form that callers may pass on
        // Windows.
        return Ok(resolved.canonicalize().unwrap_or(resolved));
    }
    let bin_name = format!("plumb{}", std::env::consts::EXE_SUFFIX);
    for profile in ["release", "debug"] {
        let candidate = workspace_root.join("target").join(profile).join(&bin_name);
        if candidate.is_file() {
            return Ok(candidate.canonicalize().unwrap_or(candidate));
        }
    }
    Err(anyhow::anyhow!(
        "no plumb binary found at target/release/{bin_name} or target/debug/{bin_name}. \
         Build it first (`cargo build --release -p plumb-cli`) or pass --plumb-bin.",
    ))
    .context("resolve plumb binary path")
}

/// Append `std::env::consts::EXE_SUFFIX` to `path` if it doesn't already
/// end with it. On Unix this is a no-op (suffix is empty); on Windows it
/// turns `…/plumb` into `…/plumb.exe`.
fn with_exe_suffix(path: &std::path::Path) -> PathBuf {
    let suffix = std::env::consts::EXE_SUFFIX;
    if suffix.is_empty() {
        return path.to_path_buf();
    }
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case(suffix.trim_start_matches('.')) => path.to_path_buf(),
        _ => {
            let mut s = path.as_os_str().to_owned();
            s.push(suffix);
            PathBuf::from(s)
        }
    }
}
