//! Per-site execution. Build → serve → lint × 3 → assert.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::Duration;

use indexmap::IndexMap;

use crate::HarnessError;
use crate::expected::{Expected, WaitFor};
use crate::server::StaticServer;

const DEFAULT_LINT_TIMEOUT_SECS: u64 = 120;
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const CLEANUP_RETRIES: usize = 100;
const DIAGNOSTIC_LIMIT: usize = 4096;

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
    /// Timeout for each child `plumb lint` invocation.
    pub lint_timeout: Duration,
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
            lint_timeout: Duration::from_secs(DEFAULT_LINT_TIMEOUT_SECS),
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
        let stdout = run_lint(name, &url, config, expected.wait_for.as_ref(), run_idx)?;
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
    run_idx: usize,
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
    let tmp_dir = isolated_tmp_dir(name, run_idx, std::process::id());
    std::fs::create_dir_all(&tmp_dir).map_err(|err| HarnessError::Lint {
        site: name.to_owned(),
        reason: format!("create isolated TMPDIR `{}`: {err}", tmp_dir.display()),
    })?;
    set_child_temp_env(&mut cmd, &tmp_dir);
    let command_preview = format_command(&cmd);
    tracing::info!(
        site = %name,
        run = run_idx,
        timeout_secs = config.lint_timeout.as_secs(),
        command = %command_preview,
        tmp_dir = %tmp_dir.display(),
        "harness — running plumb lint",
    );

    let output = run_command_with_timeout(&mut cmd, config.lint_timeout).map_err(|err| {
        cleanup_tmp_dir(&tmp_dir);
        HarnessError::Lint {
            site: name.to_owned(),
            reason: format!(
                "spawn or wait for plumb binary `{}`: {err}; command={command_preview}",
                config.plumb_bin.display()
            ),
        }
    })?;
    cleanup_tmp_dir(&tmp_dir);

    let (status, stdout, stderr, pid) = match output {
        ChildOutput::Exited {
            status,
            stdout,
            stderr,
            pid,
        } => (status, stdout, stderr, pid),
        ChildOutput::TimedOut {
            stdout,
            stderr,
            pid,
        } => {
            return Err(HarnessError::Lint {
                site: name.to_owned(),
                reason: format!(
                    "plumb lint timed out after {:?}; pid={pid}; run={run_idx}; command={command_preview}; stdout_bytes={}; stderr=\n{}",
                    config.lint_timeout,
                    stdout.len(),
                    diagnostic_text(&stderr),
                ),
            });
        }
    };

    // PRD §13.3: 0 = clean, 1 = one or more violations at/above the
    // default `--min-severity warn` threshold, 2 = CLI / infrastructure
    // failure. The fixtures intentionally produce warnings, so exit 1 is
    // the steady state; anything outside {0, 1} is an infrastructure
    // failure.
    let code = status.code();
    let allowed = matches!(code, Some(0 | 1));
    if !allowed {
        return Err(HarnessError::Lint {
            site: name.to_owned(),
            reason: format!(
                "plumb exited with code {code:?}; pid={pid}; run={run_idx}; command={command_preview}; stdout_bytes={}; stderr=\n{}",
                stdout.len(),
                diagnostic_text(&stderr),
            ),
        });
    }
    Ok(stdout)
}

enum ChildOutput {
    Exited {
        status: ExitStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        pid: u32,
    },
    TimedOut {
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        pid: u32,
    },
}

fn run_command_with_timeout(
    cmd: &mut Command,
    timeout: Duration,
) -> Result<ChildOutput, std::io::Error> {
    configure_child_process(cmd);
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let pid = child.id();

    // Reader threads must start before waiting so large JSON/stdout or
    // stderr output cannot fill an OS pipe and deadlock the child.
    let stdout_handle = child.stdout.take().map(spawn_reader);
    let stderr_handle = child.stderr.take().map(spawn_reader);

    match wait_with_timeout(&mut child, timeout) {
        WaitOutcome::Exited(status) => Ok(ChildOutput::Exited {
            status,
            stdout: drain_reader(stdout_handle),
            stderr: drain_reader(stderr_handle),
            pid,
        }),
        WaitOutcome::TimedOut => {
            kill_child_process_tree(&mut child, pid);
            Ok(ChildOutput::TimedOut {
                stdout: drain_reader(stdout_handle),
                stderr: drain_reader(stderr_handle),
                pid,
            })
        }
        WaitOutcome::Errored(err) => {
            kill_child_process_tree(&mut child, pid);
            let _ = drain_reader(stdout_handle);
            let _ = drain_reader(stderr_handle);
            Err(err)
        }
    }
}

#[cfg(unix)]
fn configure_child_process(cmd: &mut Command) {
    use std::os::unix::process::CommandExt as _;

    cmd.process_group(0);
}

#[cfg(not(unix))]
fn configure_child_process(_cmd: &mut Command) {}

#[cfg(unix)]
fn kill_child_process_tree(child: &mut std::process::Child, pid: u32) {
    signal_process_group(pid, "TERM");
    wait_for_child_exit(child, Duration::from_millis(500));
    signal_process_group(pid, "KILL");
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(not(unix))]
fn kill_child_process_tree(child: &mut std::process::Child, _pid: u32) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(unix)]
fn signal_process_group(pid: u32, signal: &str) {
    let group = format!("-{pid}");
    let _ = Command::new("kill")
        .arg(format!("-{signal}"))
        .arg(group)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(unix)]
fn wait_for_child_exit(child: &mut std::process::Child, timeout: Duration) {
    let max_ticks = (timeout.as_millis() / POLL_INTERVAL.as_millis()).max(1);
    for _ in 0..max_ticks {
        match child.try_wait() {
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Ok(Some(_)) | Err(_) => return,
        }
    }
}

fn spawn_reader<R>(mut reader: R) -> thread::JoinHandle<std::io::Result<Vec<u8>>>
where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Ok(buf)
    })
}

fn drain_reader(handle: Option<thread::JoinHandle<std::io::Result<Vec<u8>>>>) -> Vec<u8> {
    match handle {
        None => Vec::new(),
        Some(h) => match h.join() {
            Ok(Ok(buf)) => buf,
            Ok(Err(_)) | Err(_) => Vec::new(),
        },
    }
}

enum WaitOutcome {
    Exited(ExitStatus),
    Errored(std::io::Error),
    TimedOut,
}

fn wait_with_timeout(child: &mut std::process::Child, timeout: Duration) -> WaitOutcome {
    let max_ticks = (timeout.as_millis() / POLL_INTERVAL.as_millis()).max(1);
    for _ in 0..max_ticks {
        match child.try_wait() {
            Ok(Some(status)) => return WaitOutcome::Exited(status),
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(err) => return WaitOutcome::Errored(err),
        }
    }
    WaitOutcome::TimedOut
}

fn isolated_tmp_dir(site: &str, run_idx: usize, process_id: u32) -> PathBuf {
    let safe_site = site
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    short_temp_root().join(format!("pe2e-{safe_site}-{process_id}-{run_idx}"))
}

fn short_temp_root() -> PathBuf {
    #[cfg(unix)]
    {
        PathBuf::from("/tmp")
    }
    #[cfg(windows)]
    {
        for key in ["TMP", "TEMP"] {
            if let Some(path) = std::env::var_os(key) {
                return PathBuf::from(path);
            }
        }
        PathBuf::from(r"C:\Temp")
    }
    #[cfg(not(any(unix, windows)))]
    {
        PathBuf::from("tmp")
    }
}

fn set_child_temp_env(cmd: &mut Command, tmp_dir: &Path) {
    cmd.env("TMPDIR", tmp_dir);
    cmd.env("TMP", tmp_dir);
    cmd.env("TEMP", tmp_dir);
}

fn cleanup_tmp_dir(path: &Path) {
    for _ in 0..CLEANUP_RETRIES {
        match std::fs::remove_dir_all(path) {
            Ok(()) => return,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
            Err(_) => thread::sleep(POLL_INTERVAL),
        }
    }
    let _ = std::fs::remove_dir_all(path);
}

fn format_command(cmd: &Command) -> String {
    std::iter::once(cmd.get_program())
        .chain(cmd.get_args())
        .map(shell_quote)
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(arg: &std::ffi::OsStr) -> String {
    let s = arg.to_string_lossy();
    if s.is_empty() {
        return String::from("''");
    }
    if s.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':' | '=' | ',' | '@')
    }) {
        return s.into_owned();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn diagnostic_text(bytes: &[u8]) -> String {
    let prefix = if bytes.len() > DIAGNOSTIC_LIMIT {
        &bytes[..DIAGNOSTIC_LIMIT]
    } else {
        bytes
    };
    let text = String::from_utf8_lossy(prefix);
    if bytes.len() > DIAGNOSTIC_LIMIT {
        format!(
            "{text}\n<truncated: showing {DIAGNOSTIC_LIMIT} of {} bytes>",
            bytes.len()
        )
    } else {
        text.into_owned()
    }
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
    use std::process::Command;

    use super::{
        DIAGNOSTIC_LIMIT, diagnostic_text, format_command, isolated_tmp_dir, parse_counts,
        set_child_temp_env,
    };

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

    #[test]
    fn format_command_quotes_arguments_with_spaces() {
        let mut cmd = Command::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
        cmd.arg("lint").arg("http://127.0.0.1:4173/");

        let formatted = format_command(&cmd);

        assert_eq!(
            formatted,
            "'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' lint http://127.0.0.1:4173/"
        );
    }

    #[test]
    fn isolated_tmp_dir_sanitizes_site_name() {
        let path = isolated_tmp_dir("vue/fixture", 2, 1234);

        assert!(path.to_string_lossy().ends_with("pe2e-vue-fixture-1234-2"));
    }

    #[test]
    #[cfg(unix)]
    fn isolated_tmp_dir_uses_short_unix_root() {
        let path = isolated_tmp_dir("html-css", 0, 1234);

        assert!(path.starts_with("/tmp"));
    }

    #[test]
    fn diagnostic_text_truncates_long_output() {
        let bytes = vec![b'a'; DIAGNOSTIC_LIMIT + 2];

        let text = diagnostic_text(&bytes);

        assert!(text.contains("<truncated: showing"));
        assert!(text.ends_with(&format!("of {} bytes>", DIAGNOSTIC_LIMIT + 2)));
    }

    #[test]
    fn set_child_temp_env_sets_unix_and_windows_vars() {
        let mut cmd = Command::new("plumb");
        let tmp = std::path::Path::new("/workspace/target/plumb-e2e-tmp/run");
        let expected = tmp.as_os_str().to_owned();

        set_child_temp_env(&mut cmd, tmp);

        let envs = cmd
            .get_envs()
            .filter_map(|(key, value)| value.map(|v| (key.to_owned(), v.to_owned())))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(envs.get(std::ffi::OsStr::new("TMPDIR")), Some(&expected));
        assert_eq!(envs.get(std::ffi::OsStr::new("TMP")), Some(&expected));
        assert_eq!(envs.get(std::ffi::OsStr::new("TEMP")), Some(&expected));
    }

    #[test]
    #[cfg(unix)]
    fn command_timeout_kills_stuck_child() {
        use std::time::Duration;

        use super::{ChildOutput, run_command_with_timeout};

        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("printf ready; sleep 5; printf never >&2");

        let output =
            run_command_with_timeout(&mut cmd, Duration::from_millis(50)).expect("run command");

        let ChildOutput::TimedOut { stdout, stderr, .. } = output else {
            panic!("child should time out");
        };
        assert_eq!(stdout, b"ready");
        assert!(stderr.is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn command_timeout_kills_process_group() {
        use std::time::Duration;

        use super::{ChildOutput, run_command_with_timeout};

        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("(trap '' TERM; printf child-ready; sleep 1; printf late) & wait");

        let output =
            run_command_with_timeout(&mut cmd, Duration::from_millis(50)).expect("run command");

        let ChildOutput::TimedOut { stdout, .. } = output else {
            panic!("child should time out");
        };
        assert_eq!(stdout, b"child-ready");
    }

    #[test]
    #[cfg(unix)]
    fn command_output_drains_large_stdout() {
        use std::time::Duration;

        use super::{ChildOutput, run_command_with_timeout};

        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("yes x | head -c 131072");

        let output =
            run_command_with_timeout(&mut cmd, Duration::from_secs(5)).expect("run command");

        let ChildOutput::Exited { status, stdout, .. } = output else {
            panic!("child should exit");
        };
        assert!(status.success());
        assert_eq!(stdout.len(), 131_072);
    }
}
