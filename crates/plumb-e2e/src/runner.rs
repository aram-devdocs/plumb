//! Per-site execution. Build → serve → lint × 3 → assert.

use std::path::{Path, PathBuf};
use std::process::Command;

use indexmap::IndexMap;

use crate::HarnessError;
use crate::expected::{Expected, WaitFor};
use crate::server::StaticServer;

/// Runtime configuration for a harness invocation.
#[derive(Debug, Clone)]
pub struct HarnessConfig {
    /// Workspace root (parent of `crates/` and `e2e-sites/`).
    pub workspace_root: PathBuf,
    /// Absolute path to the locally built `plumb` binary.
    pub plumb_bin: PathBuf,
    /// Optional override for the Chromium executable. Maps to
    /// `plumb lint --executable-path`.
    pub chrome_path: Option<PathBuf>,
    /// Whether to build each fixture before linting it (`just build`
    /// inside the fixture). CI sets this to `true`; local re-runs may
    /// pass `false` to save time.
    pub build_first: bool,
    /// How many lint runs to compare for byte-equality. The default
    /// is 3, matching `just determinism-check`.
    pub determinism_runs: usize,
}

impl HarnessConfig {
    /// Construct from a workspace root + a plumb binary path. All other
    /// fields default to safe values (`build_first = true`,
    /// `determinism_runs = 3`).
    #[must_use]
    pub fn new(workspace_root: PathBuf, plumb_bin: PathBuf) -> Self {
        Self {
            workspace_root,
            plumb_bin,
            chrome_path: None,
            build_first: true,
            determinism_runs: 3,
        }
    }
}

/// Outcome of a single site run.
#[derive(Debug, Clone)]
pub struct RunReport {
    /// Site slug, e.g. `html-css`.
    pub site: String,
    /// Counts grouped by `rule_id` for the target rules only.
    pub by_rule_id: IndexMap<String, usize>,
    /// Total target-rule violation count.
    pub total_target: usize,
    /// Count of non-target violations (logged for visibility, not
    /// asserted on).
    pub non_target: usize,
}

/// Run the full harness pipeline for a single site.
///
/// # Errors
///
/// Returns [`HarnessError`] on any failure: missing fixture, build
/// failure, bind failure, lint failure, byte-equality drift, or
/// per-rule count mismatch.
pub fn run_site(name: &str, config: &HarnessConfig) -> Result<RunReport, HarnessError> {
    let site_dir = config.workspace_root.join("e2e-sites").join(name);
    if !site_dir.is_dir() {
        return Err(HarnessError::Workspace(format!(
            "fixture `{name}` not found at {}",
            site_dir.display()
        )));
    }
    let expected = Expected::load(&site_dir.join("expected.json")).map_err(|source| {
        HarnessError::Expected {
            site: name.to_owned(),
            source,
        }
    })?;

    if config.build_first {
        run_build(name, &site_dir)?;
    }

    let dist = site_dir.join("dist");
    let server = StaticServer::bind(dist).map_err(|source| HarnessError::Bind {
        site: name.to_owned(),
        source,
    })?;
    let url = server.base_url();
    tracing::info!(site = %name, url = %url, "harness — server bound");

    // Run the lint determinism_runs times and assert byte-equality.
    let mut outputs = Vec::with_capacity(config.determinism_runs);
    for run_idx in 0..config.determinism_runs {
        let stdout = run_lint(name, &url, config, expected.wait_for.as_ref())?;
        tracing::debug!(
            site = %name,
            run = run_idx,
            bytes = stdout.len(),
            "harness — lint run captured",
        );
        outputs.push(stdout);
    }
    if outputs.windows(2).any(|w| w[0] != w[1]) {
        return Err(HarnessError::NonDeterministic {
            site: name.to_owned(),
        });
    }

    let counts =
        parse_counts(&outputs[0], &expected.target_rules).map_err(|reason| HarnessError::Lint {
            site: name.to_owned(),
            reason,
        })?;

    // Assert per-rule counts match expected.
    for rule_id in &expected.target_rules {
        let expected_count = expected.by_rule_id.get(rule_id).copied().unwrap_or(0);
        let actual_count = counts.targeted.get(rule_id).copied().unwrap_or(0);
        if expected_count != actual_count {
            return Err(HarnessError::CountMismatch {
                site: name.to_owned(),
                rule_id: rule_id.clone(),
                expected: expected_count,
                actual: actual_count,
            });
        }
    }
    let total_target: usize = counts.targeted.values().sum();
    if total_target != expected.total_target_violations {
        return Err(HarnessError::CountMismatch {
            site: name.to_owned(),
            rule_id: String::from("<total>"),
            expected: expected.total_target_violations,
            actual: total_target,
        });
    }

    Ok(RunReport {
        site: name.to_owned(),
        by_rule_id: counts.targeted,
        total_target,
        non_target: counts.non_target,
    })
}

fn run_build(name: &str, site_dir: &Path) -> Result<(), HarnessError> {
    tracing::info!(site = %name, dir = %site_dir.display(), "harness — running just build");
    let status = Command::new("just")
        .arg("build")
        .current_dir(site_dir)
        .status()
        .map_err(|err| HarnessError::Build {
            site: name.to_owned(),
            source: anyhow::anyhow!("spawn `just build`: {err}"),
        })?;
    if !status.success() {
        return Err(HarnessError::Build {
            site: name.to_owned(),
            source: anyhow::anyhow!("`just build` exited with status {status}"),
        });
    }
    Ok(())
}

fn run_lint(
    name: &str,
    url: &str,
    config: &HarnessConfig,
    wait_for: Option<&WaitFor>,
) -> Result<Vec<u8>, HarnessError> {
    let plumb_config = config.workspace_root.join("e2e-sites").join("plumb.toml");
    let mut cmd = Command::new(&config.plumb_bin);
    cmd.arg("lint")
        .arg(url)
        .arg("--config")
        .arg(&plumb_config)
        .arg("--format")
        .arg("json");
    if let Some(path) = &config.chrome_path {
        cmd.arg("--executable-path").arg(path);
    }
    if let Some(gate) = wait_for {
        cmd.arg("--wait-for").arg(&gate.selector);
        cmd.arg("--wait-ms").arg(gate.timeout_ms.to_string());
    }
    let output = cmd.output().map_err(|err| HarnessError::Lint {
        site: name.to_owned(),
        reason: format!("spawn plumb binary `{}`: {err}", config.plumb_bin.display()),
    })?;
    // PRD §13.3: 0 = clean, 1 = errors, 3 = warnings only. The fixtures
    // intentionally produce warnings, so 3 is the steady state. Any
    // other code is an infrastructure failure.
    let code = output.status.code();
    let allowed = matches!(code, Some(0 | 1 | 3));
    if !allowed {
        return Err(HarnessError::Lint {
            site: name.to_owned(),
            reason: format!(
                "plumb exited with code {code:?}; stderr=\n{}",
                String::from_utf8_lossy(&output.stderr),
            ),
        });
    }
    Ok(output.stdout)
}

#[derive(Debug)]
struct Counts {
    targeted: IndexMap<String, usize>,
    non_target: usize,
}

fn parse_counts(stdout: &[u8], target_rules: &[String]) -> Result<Counts, String> {
    let value: serde_json::Value =
        serde_json::from_slice(stdout).map_err(|e| format!("parse JSON output: {e}"))?;
    let violations = value
        .get("violations")
        .and_then(|v| v.as_array())
        .ok_or_else(|| String::from("missing `violations` array"))?;

    let mut targeted: IndexMap<String, usize> = IndexMap::new();
    for rule in target_rules {
        targeted.insert(rule.clone(), 0);
    }
    let mut non_target = 0usize;
    for v in violations {
        let Some(rule_id) = v.get("rule_id").and_then(|s| s.as_str()) else {
            continue;
        };
        if target_rules.iter().any(|r| r == rule_id) {
            if let Some(slot) = targeted.get_mut(rule_id) {
                *slot += 1;
            }
        } else {
            non_target += 1;
        }
    }
    Ok(Counts {
        targeted,
        non_target,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_counts;

    #[test]
    fn parse_counts_buckets_by_rule_id() {
        let json = br#"{
            "violations": [
                { "rule_id": "a/b" },
                { "rule_id": "a/b" },
                { "rule_id": "c/d" },
                { "rule_id": "z/other" }
            ]
        }"#;
        let counts = parse_counts(json, &["a/b".into(), "c/d".into()]).expect("parse");
        assert_eq!(counts.targeted.get("a/b").copied(), Some(2));
        assert_eq!(counts.targeted.get("c/d").copied(), Some(1));
        assert_eq!(counts.non_target, 1);
    }

    #[test]
    fn parse_counts_handles_empty_violations() {
        let json = br#"{ "violations": [] }"#;
        let counts = parse_counts(json, &["a/b".into()]).expect("parse");
        assert_eq!(counts.targeted.get("a/b").copied(), Some(0));
        assert_eq!(counts.non_target, 0);
    }

    #[test]
    fn parse_counts_rejects_missing_array() {
        let json = br#"{ "stats": {} }"#;
        let err = parse_counts(json, &[]).expect_err("must error");
        assert!(err.contains("violations"));
    }
}
