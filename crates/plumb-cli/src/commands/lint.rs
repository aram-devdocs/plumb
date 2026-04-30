//! `plumb lint <url>` — the critical path.
//!
//! Wires CLI → config loader → driver (fake for `plumb-fake://`) →
//! engine → formatter → stdout.
//!
//! The orchestrator builds one [`Target`] per requested viewport and
//! calls [`BrowserDriver::snapshot_all`] exactly once, so a real
//! Chromium driver launches the browser only once per CLI invocation
//! (PRD §10.3).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use plumb_cdp::{BrowserDriver, ChromiumDriver, ChromiumOptions, FakeDriver, Target, is_fake_url};
use plumb_core::{Config, Severity, ViewportKey};
use thiserror::Error;

use crate::commands::{OutputFormat, selector as selector_filter};

/// CLI-side errors that never need to leak across the
/// `commands::lint` boundary. Bubbles up to `main` via `anyhow::Error`,
/// where `report_error` formats it onto stderr and `main` returns
/// `ExitCode::from(2)` per PRD §13.3 ("CLI / infrastructure failure").
#[derive(Debug, Error)]
enum LintError {
    /// One or more `--viewport` values were not present in `config.viewports`.
    /// `unknown` is preserved in flag-input order; `available` is sorted
    /// alphabetically for stable rendering.
    #[error("unknown viewport(s): {}. configured viewports: {}", .unknown.join(", "), .available.join(", "))]
    UnknownViewports {
        unknown: Vec<String>,
        available: Vec<String>,
    },
    /// `--viewport` was passed but `config.viewports` is empty (no
    /// `[viewports]` section in `plumb.toml`, or no config at all).
    /// Refuse to silently fall back to the default desktop because the
    /// user explicitly named a viewport — see issue #119.
    #[error(
        "--viewport {} requested but no [viewports] are configured. add [viewports.{}] to plumb.toml or remove the flag to use the default 1280x800 desktop.",
        .names.join(", "),
        .names.first().map_or("<name>", String::as_str)
    )]
    ViewportFlagWithoutConfig { names: Vec<String> },
}

pub async fn run(
    url: String,
    config_path: Option<PathBuf>,
    executable_path: Option<PathBuf>,
    format: OutputFormat,
    viewports: Vec<String>,
    selector: Option<String>,
) -> Result<ExitCode> {
    tracing::debug!(url = %url, format = %format, viewports = ?viewports, selector = ?selector, "lint");

    let config = load_config(config_path.as_deref())?;
    let targets = resolve_targets(&url, &config, &viewports).map_err(anyhow::Error::from)?;

    let snapshots = if is_fake_url(&url) {
        let driver = FakeDriver;
        driver
            .snapshot_all(targets)
            .await
            .map_err(anyhow::Error::from)?
    } else {
        let driver = ChromiumDriver::new(ChromiumOptions {
            executable_path,
            ..ChromiumOptions::default()
        });
        driver
            .snapshot_all(targets)
            .await
            .map_err(anyhow::Error::from)?
    };

    // PRD §15.4 — apply `--selector` between snapshot collection and
    // rule dispatch. Per-snapshot: any viewport whose subtree has no
    // matches surfaces as a CLI / infrastructure error (exit 2) so
    // "filter ran, no violations" stays distinct from "filter failed".
    let snapshots = if let Some(sel) = selector.as_deref() {
        snapshots
            .into_iter()
            .map(|snap| selector_filter::filter_snapshot(snap, sel))
            .collect::<Result<Vec<_>, _>>()
            .map_err(anyhow::Error::from)?
    } else {
        snapshots
    };

    let violations = plumb_core::run_many(snapshots.iter(), &config);

    let out = match format {
        OutputFormat::Pretty => plumb_format::pretty(&violations),
        OutputFormat::Json => plumb_format::json(&violations).context("serialize JSON")?,
        OutputFormat::Sarif => {
            plumb_format::sarif_with_rules(&violations, &plumb_core::builtin_rule_metadata())
                .context("serialize SARIF")?
        }
    };
    // CLI is the one place writing to stdout is permitted — hence the
    // crate-level allow(clippy::print_stdout) above.
    #[allow(clippy::print_stdout)]
    {
        print!("{out}");
    }

    Ok(exit_code_for(&violations))
}

/// Decide which viewports to snapshot.
///
/// Branches:
///
/// 1. `config.viewports` is empty and `viewports_arg` is empty → fall
///    back to a single `desktop` 1280x800 target (the walking-skeleton
///    default that keeps `plumb lint plumb-fake://hello` working in a
///    fresh checkout with no `plumb.toml`).
/// 2. `config.viewports` is empty and `viewports_arg` is non-empty →
///    return [`LintError::ViewportFlagWithoutConfig`]. The user named
///    viewports that don't exist; silently substituting `desktop`
///    would run the lint with the wrong viewport and hide the
///    mismatch (issue #119). The populated-config path already errors
///    on unknown names; this branch makes the empty-config path
///    consistent.
/// 3. `config.viewports` is non-empty and `viewports_arg` is empty →
///    one target per configured viewport, in `IndexMap` insertion
///    order (preserves the determinism invariant).
/// 4. Both are non-empty → filter the configured set down to the
///    named viewports. Any unknown name produces
///    [`LintError::UnknownViewports`].
fn resolve_targets(
    url: &str,
    config: &Config,
    viewports_arg: &[String],
) -> Result<Vec<Target>, LintError> {
    if config.viewports.is_empty() {
        if !viewports_arg.is_empty() {
            return Err(LintError::ViewportFlagWithoutConfig {
                names: viewports_arg.to_vec(),
            });
        }
        return Ok(vec![Target {
            url: url.to_owned(),
            viewport: ViewportKey::new("desktop"),
            width: 1280,
            height: 800,
            device_pixel_ratio: 1.0,
        }]);
    }

    if viewports_arg.is_empty() {
        return Ok(config
            .viewports
            .iter()
            .map(|(name, spec)| Target {
                url: url.to_owned(),
                viewport: ViewportKey::new(name.clone()),
                width: spec.width,
                height: spec.height,
                device_pixel_ratio: spec.device_pixel_ratio,
            })
            .collect());
    }

    let unknown: Vec<String> = viewports_arg
        .iter()
        .filter(|name| !config.viewports.contains_key(name.as_str()))
        .cloned()
        .collect();
    if !unknown.is_empty() {
        let mut available: Vec<String> = config.viewports.keys().cloned().collect();
        available.sort();
        return Err(LintError::UnknownViewports { unknown, available });
    }

    Ok(viewports_arg
        .iter()
        .filter_map(|name| {
            config.viewports.get(name.as_str()).map(|spec| Target {
                url: url.to_owned(),
                viewport: ViewportKey::new(name.clone()),
                width: spec.width,
                height: spec.height,
                device_pixel_ratio: spec.device_pixel_ratio,
            })
        })
        .collect())
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

#[cfg(test)]
mod tests {
    use super::{LintError, resolve_targets};
    use plumb_core::Config;
    use plumb_core::config::ViewportSpec;

    fn config_with(viewports: &[(&str, u32, u32)]) -> Config {
        let mut config = Config::default();
        for (name, width, height) in viewports {
            config.viewports.insert(
                (*name).to_owned(),
                ViewportSpec {
                    width: *width,
                    height: *height,
                    device_pixel_ratio: 1.0,
                },
            );
        }
        config
    }

    #[test]
    fn empty_config_yields_single_default_desktop_target() {
        let config = Config::default();
        let targets = resolve_targets("plumb-fake://hello", &config, &[])
            .expect("default fallback never errors");
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].viewport.as_str(), "desktop");
        assert_eq!(targets[0].width, 1280);
        assert_eq!(targets[0].height, 800);
    }

    /// When `config.viewports` is empty and the user passed `--viewport
    /// NAME`, the orchestrator refuses to silently fall back to the
    /// default desktop — that hid an entire run behind the wrong
    /// viewport (issue #119). Input order is preserved on the error so
    /// the message matches the user's flag order.
    #[test]
    fn empty_config_with_viewport_flag_errors() {
        let config = Config::default();
        let err = resolve_targets(
            "plumb-fake://hello",
            &config,
            &["mobile".to_owned(), "tablet".to_owned()],
        )
        .expect_err("empty config + viewport flag must error");
        match err {
            LintError::ViewportFlagWithoutConfig { names } => {
                assert_eq!(names, vec!["mobile", "tablet"]);
            }
            other @ LintError::UnknownViewports { .. } => {
                panic!("expected ViewportFlagWithoutConfig, got {other:?}")
            }
        }
    }

    #[test]
    fn configured_viewports_preserve_indexmap_order() {
        let config = config_with(&[("mobile", 375, 812), ("desktop", 1280, 800)]);
        let targets = resolve_targets("plumb-fake://hello", &config, &[]).expect("resolve targets");
        let names: Vec<&str> = targets.iter().map(|t| t.viewport.as_str()).collect();
        assert_eq!(names, vec!["mobile", "desktop"]);
    }

    #[test]
    fn filter_to_named_viewport_returns_single_target() {
        let config = config_with(&[("mobile", 375, 812), ("desktop", 1280, 800)]);
        let targets = resolve_targets("plumb-fake://hello", &config, &["mobile".to_owned()])
            .expect("mobile is configured");
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].viewport.as_str(), "mobile");
        assert_eq!(targets[0].width, 375);
    }

    #[test]
    fn unknown_viewport_lists_available_alphabetically() {
        let config = config_with(&[("mobile", 375, 812), ("desktop", 1280, 800)]);
        let err = resolve_targets("plumb-fake://hello", &config, &["bogus".to_owned()])
            .expect_err("bogus is not configured");
        match err {
            LintError::UnknownViewports { unknown, available } => {
                assert_eq!(unknown, vec!["bogus"]);
                assert_eq!(available, vec!["desktop", "mobile"]);
            }
            other @ LintError::ViewportFlagWithoutConfig { .. } => {
                panic!("expected UnknownViewports, got {other:?}")
            }
        }
    }

    #[test]
    fn unknown_viewport_reports_only_missing_names() {
        let config = config_with(&[("mobile", 375, 812), ("desktop", 1280, 800)]);
        let err = resolve_targets(
            "plumb-fake://hello",
            &config,
            &["mobile".to_owned(), "bogus".to_owned()],
        )
        .expect_err("bogus is not configured");
        match err {
            LintError::UnknownViewports { unknown, available } => {
                assert_eq!(unknown, vec!["bogus"]);
                assert_eq!(available, vec!["desktop", "mobile"]);
            }
            other @ LintError::ViewportFlagWithoutConfig { .. } => {
                panic!("expected UnknownViewports, got {other:?}")
            }
        }
    }

    #[test]
    fn unknown_viewport_preserves_input_order_for_unknown_names() {
        let config = config_with(&[("mobile", 375, 812), ("desktop", 1280, 800)]);
        let err = resolve_targets(
            "plumb-fake://hello",
            &config,
            &["bogus".to_owned(), "alpha".to_owned()],
        )
        .expect_err("neither name is configured");
        match err {
            LintError::UnknownViewports { unknown, available } => {
                assert_eq!(unknown, vec!["bogus", "alpha"]);
                assert_eq!(available, vec!["desktop", "mobile"]);
            }
            other @ LintError::ViewportFlagWithoutConfig { .. } => {
                panic!("expected UnknownViewports, got {other:?}")
            }
        }
    }
}
