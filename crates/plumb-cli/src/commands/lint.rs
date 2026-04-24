//! `plumb lint <url>` — the critical path.
//!
//! Wires CLI → config loader → driver (fake for `plumb-fake://`) →
//! engine → formatter → stdout.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use plumb_cdp::{BrowserDriver, ChromiumDriver, ChromiumOptions, FakeDriver, Target, is_fake_url};
use plumb_core::{Config, Severity, ViewportKey};

use crate::commands::OutputFormat;

pub async fn run(
    url: String,
    config_path: Option<PathBuf>,
    executable_path: Option<PathBuf>,
    format: OutputFormat,
) -> Result<ExitCode> {
    tracing::debug!(url = %url, format = %format, "lint");

    let config = load_config(config_path.as_deref())?;

    let snapshot = if is_fake_url(&url) {
        let driver = FakeDriver;
        let target = Target {
            url: url.clone(),
            viewport: ViewportKey::new("desktop"),
            width: 1280,
            height: 800,
            device_pixel_ratio: 1.0,
        };
        driver.snapshot(target).await.map_err(anyhow::Error::from)?
    } else {
        let driver = ChromiumDriver::new(ChromiumOptions { executable_path });
        let target = Target {
            url: url.clone(),
            viewport: ViewportKey::new("desktop"),
            width: 1280,
            height: 800,
            device_pixel_ratio: 1.0,
        };
        driver.snapshot(target).await.map_err(anyhow::Error::from)?
    };

    let violations = plumb_core::run(&snapshot, &config);

    let out = match format {
        OutputFormat::Pretty => plumb_format::pretty(&violations),
        OutputFormat::Json => plumb_format::json(&violations).context("serialize JSON")?,
        OutputFormat::Sarif => plumb_format::sarif(&violations).context("serialize SARIF")?,
    };
    // CLI is the one place writing to stdout is permitted — hence the
    // crate-level allow(clippy::print_stdout) above.
    #[allow(clippy::print_stdout)]
    {
        print!("{out}");
    }

    Ok(exit_code_for(&violations))
}

fn load_config(path: Option<&Path>) -> Result<Config> {
    if let Some(explicit) = path {
        return plumb_config::load(explicit)
            .with_context(|| format!("load config from {}", explicit.display()));
    }
    // Default: look for `plumb.toml` in CWD. Fall back to defaults
    // if not present so `plumb lint plumb-fake://hello` works out
    // of the box in a fresh checkout.
    let default = PathBuf::from("plumb.toml");
    if default.exists() {
        plumb_config::load(&default).context("load ./plumb.toml")
    } else {
        tracing::debug!("no plumb.toml in CWD; using defaults");
        Ok(Config::default())
    }
}

fn exit_code_for(violations: &[plumb_core::Violation]) -> ExitCode {
    // PRD §13.3 exit-code mapping:
    //   0 — no violations
    //   1 — errors present
    //   2 — reserved for CLI/infra failures (handled in main)
    //   3 — warnings only (no errors)
    let mut has_error = false;
    let mut has_warning = false;
    for v in violations {
        match v.severity {
            Severity::Error => has_error = true,
            Severity::Warning => has_warning = true,
            Severity::Info => {}
        }
    }
    if has_error {
        ExitCode::from(1)
    } else if has_warning {
        ExitCode::from(3)
    } else {
        ExitCode::SUCCESS
    }
}
