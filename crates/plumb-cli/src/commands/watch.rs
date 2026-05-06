//! `plumb watch` — re-run `plumb lint` on filesystem changes.
//!
//! The watch loop is a thin shell around [`crate::commands::lint::run`]:
//!
//! 1. Resolve the directories to watch (`--path`, falling back to CWD).
//! 2. Read `.plumbignore` (one substring per line) for excludes.
//! 3. Subscribe to `notify` events via [`notify_debouncer_full`], which
//!    collapses bursts of related events into a single
//!    [`DebouncedEvent`] after the 250 ms debounce window.
//! 4. On each cycle, count the changed files (post-ignore), invoke
//!    `lint::run`, and emit a one-line status to stderr:
//!
//!    ```text
//!    watching… changed: <N> files; lint: <M> violations; took <T> ms
//!    ```
//!
//! Cancellation is via [`tokio::signal::ctrl_c`].
//!
//! The hidden `--once` flag short-circuits the watcher, runs a single
//! cycle, and returns. It exists for integration tests and ad-hoc
//! shell use; production users always run the loop.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use notify::{EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebouncedEvent, Debouncer, RecommendedCache, new_debouncer};
use thiserror::Error;
use tokio::sync::mpsc::UnboundedSender;

use crate::commands::lint::{LintArgs, run as run_lint};

/// Debounce window. The acceptance-criteria value lives here as a
/// constant so the integration test (or a future tuning knob) can
/// reference it.
const DEBOUNCE: Duration = Duration::from_millis(250);

/// Aggregated args for [`run`]. Owns a [`LintArgs`] template that the
/// watcher rebuilds (via [`clone_lint_args`]) for every cycle.
#[derive(Debug)]
pub struct WatchArgs {
    /// The lint flags to apply on every cycle.
    pub lint: LintArgs,
    /// Directories to watch. Empty means "the current working
    /// directory".
    pub watch_paths: Vec<PathBuf>,
    /// Run a single cycle and exit instead of entering the watcher.
    pub once: bool,
}

/// Errors local to `commands::watch`. Bubble up as `anyhow::Error` so
/// the binary returns exit code 2 ("CLI / infrastructure failure",
/// PRD §13.3).
#[derive(Debug, Error)]
enum WatchError {
    /// `--path <dir>` resolved to a path that is not a directory.
    #[error("watch path is not a directory: {0}")]
    NotADirectory(PathBuf),
}

/// Run the `plumb watch` subcommand.
///
/// # Errors
///
/// Returns an error when:
///
/// - A `--path` argument refuses to resolve to a directory.
/// - The OS-level filesystem watcher fails to initialise.
/// - The inner `lint` cycle returns an unrecoverable error.
pub async fn run(args: WatchArgs) -> Result<ExitCode> {
    let WatchArgs {
        lint,
        watch_paths,
        once,
    } = args;

    let resolved_paths = resolve_watch_paths(&watch_paths)?;
    let ignore = PlumbIgnore::load(&resolved_paths)?;

    // Cycle 0 — every watch run lints once on startup so the user has
    // an immediate baseline. The status line for this cycle reports
    // `changed: 0 files`; the loop body uses the same helper for
    // every subsequent cycle.
    let exit_code = run_one_cycle(&lint, 0).await?;

    if once {
        return Ok(exit_code);
    }

    // The notify watcher is sync and emits events on its own thread.
    // Bridge into tokio via an unbounded async channel; the bridge
    // closure forwards each `DebouncedEvent` and exits silently when
    // the receiver hangs up (Ctrl-C tore the loop down).
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<DebouncedEvent>();
    let _debouncer = spawn_watcher(&resolved_paths, tx)?;

    tracing::info!(paths = ?resolved_paths, "watching for changes");

    // The watcher runs until Ctrl-C. The `select!` is biased toward
    // cancellation: a pending signal preempts an in-flight event
    // drain, but never the cycle itself (we await the lint).
    loop {
        tokio::select! {
            biased;
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("ctrl-c received, exiting watch loop");
                return Ok(exit_code);
            }
            maybe_event = rx.recv() => {
                let Some(first) = maybe_event else {
                    // Channel closed — watcher dropped, treat as exit.
                    return Ok(exit_code);
                };
                let mut paths = collect_event_paths(&first, &ignore);
                // Drain any other debounced events that arrived in
                // the same tick so a single cycle covers a burst.
                while let Ok(extra) = rx.try_recv() {
                    paths.extend(collect_event_paths(&extra, &ignore));
                }
                paths.sort();
                paths.dedup();
                if paths.is_empty() {
                    // Every event was ignored; skip the lint cycle.
                    continue;
                }
                let _ = run_one_cycle(&lint, paths.len()).await?;
            }
        }
    }
}

/// Run a single lint cycle and emit the watch status line on stderr.
async fn run_one_cycle(lint: &LintArgs, changed: usize) -> Result<ExitCode> {
    let started = Instant::now();
    let cloned = clone_lint_args(lint);
    let (violations, exit_code) = capture_lint_metrics(cloned).await?;
    let elapsed_ms = started.elapsed().as_millis();
    // The CLI is the one place writing to stderr is permitted — hence
    // the scoped allow. The status line goes to stderr so a piped
    // consumer of the lint stdout payload (json/sarif) still gets a
    // clean stream.
    #[allow(clippy::print_stderr)]
    {
        eprintln!(
            "watching… changed: {changed} files; lint: {violations} violations; took {elapsed_ms} ms"
        );
    }
    Ok(exit_code)
}

/// Run the inner lint and recover an approximate violation count for
/// the status line.
///
/// `lint::run` writes the rendered output to stdout itself; the
/// process-level counter we care about for the status line is the
/// PRD §13.3 exit-code bucket: errors / warnings-only / clean. We
/// expose that as a 0-or-1 count today; a finer-grained number can
/// land in a follow-up by threading the count out of `lint::run`.
async fn capture_lint_metrics(args: LintArgs) -> Result<(usize, ExitCode)> {
    let exit_code = run_lint(args).await?;
    let bucket = format!("{exit_code:?}");
    let count = usize::from(bucket.contains('1') || bucket.contains('3'));
    Ok((count, exit_code))
}

/// Resolve and validate the user-supplied `--path` flags, defaulting
/// to CWD when none were provided.
fn resolve_watch_paths(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    if paths.is_empty() {
        let cwd = std::env::current_dir().context("read current working directory")?;
        return Ok(vec![cwd]);
    }
    let mut out = Vec::with_capacity(paths.len());
    for p in paths {
        if !p.is_dir() {
            return Err(WatchError::NotADirectory(p.clone()).into());
        }
        out.push(p.clone());
    }
    Ok(out)
}

/// `.plumbignore` reader. The format is intentionally minimal:
///
/// - One pattern per line.
/// - Blank lines and `#`-prefixed lines are skipped.
/// - Each pattern is treated as a literal substring of the absolute
///   path of the changed file. Globbing is not supported — keep it
///   simple until users ask for more.
struct PlumbIgnore {
    patterns: Vec<String>,
}

impl PlumbIgnore {
    /// Load `.plumbignore` files from each watched root. A missing
    /// file is not an error; many projects won't ship one.
    fn load(roots: &[PathBuf]) -> Result<Self> {
        let mut patterns = Vec::new();
        for root in roots {
            let candidate = root.join(".plumbignore");
            if !candidate.exists() {
                continue;
            }
            let contents = std::fs::read_to_string(&candidate)
                .with_context(|| format!("read {}", candidate.display()))?;
            for raw in contents.lines() {
                let line = raw.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                patterns.push(line.to_owned());
            }
        }
        // Always ignore noisy build-output and VCS directories so a
        // fresh project Just Works without authoring `.plumbignore`.
        for builtin in DEFAULT_IGNORES {
            patterns.push((*builtin).to_owned());
        }
        Ok(Self { patterns })
    }

    fn matches(&self, path: &Path) -> bool {
        let s = path.to_string_lossy();
        self.patterns.iter().any(|pat| s.contains(pat.as_str()))
    }
}

/// Built-in ignore patterns. Keep this list tight — broad patterns
/// silently swallow real edits.
const DEFAULT_IGNORES: &[&str] = &[
    "/.git/",
    "/target/",
    "/node_modules/",
    "/.idea/",
    "/.vscode/",
];

/// Pull every path off a [`DebouncedEvent`] that survives the ignore
/// filter and is the kind of change that warrants a re-lint. We
/// accept create / modify / remove / rename and drop access-only
/// events (some platforms emit `Access(Read)` on every stat).
fn collect_event_paths(ev: &DebouncedEvent, ignore: &PlumbIgnore) -> Vec<PathBuf> {
    if !is_actionable(ev.event.kind) {
        return Vec::new();
    }
    ev.event
        .paths
        .iter()
        .filter(|p| !ignore.matches(p))
        .cloned()
        .collect()
}

fn is_actionable(kind: EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_)
            | EventKind::Modify(_)
            | EventKind::Remove(_)
            | EventKind::Other
            | EventKind::Any
    )
}

/// Spawn the [`notify_debouncer_full`] watcher. Returns the live
/// debouncer guard; dropping it tears the OS watch down.
fn spawn_watcher(
    roots: &[PathBuf],
    tx: UnboundedSender<DebouncedEvent>,
) -> Result<Debouncer<RecommendedWatcher, RecommendedCache>> {
    // `new_debouncer` wants a sync handler; bridge it to the async
    // channel by forwarding each batch through `tx.send`.
    let mut debouncer = new_debouncer(
        DEBOUNCE,
        None,
        move |result: Result<Vec<DebouncedEvent>, Vec<notify::Error>>| match result {
            Ok(events) => {
                for ev in events {
                    if tx.send(ev).is_err() {
                        // Receiver gone — likely Ctrl-C exited the
                        // outer loop. Stop forwarding silently.
                        return;
                    }
                }
            }
            Err(errors) => {
                for err in errors {
                    tracing::warn!(error = %err, "watcher error");
                }
            }
        },
    )
    .context("initialise filesystem watcher")?;

    for root in roots {
        debouncer
            .watch(root, RecursiveMode::Recursive)
            .with_context(|| format!("watch {}", root.display()))?;
    }
    Ok(debouncer)
}

/// Manual `Clone` for [`LintArgs`] — kept here instead of deriving on
/// the upstream struct so this PR's surface stays scoped to
/// `commands::watch`. Every field is `Clone`, so the body is
/// straightforward.
fn clone_lint_args(src: &LintArgs) -> LintArgs {
    LintArgs {
        url: src.url.clone(),
        config_path: src.config_path.clone(),
        executable_path: src.executable_path.clone(),
        format: src.format,
        output_path: src.output_path.clone(),
        viewports: src.viewports.clone(),
        selector: src.selector.clone(),
        wait_for: src.wait_for.clone(),
        wait_ms: src.wait_ms,
        cookies: src.cookies.clone(),
        headers: src.headers.clone(),
        auth_script: src.auth_script.clone(),
        storage_state: src.storage_state.clone(),
        disable_animations: src.disable_animations,
        hide_scrollbars: src.hide_scrollbars,
        dpr: src.dpr,
        auto_fetch_chromium: src.auto_fetch_chromium,
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_IGNORES, PlumbIgnore, is_actionable};
    use notify::EventKind;
    use notify::event::{AccessKind, CreateKind, ModifyKind, RemoveKind};
    use std::path::PathBuf;

    fn ignore_with_defaults() -> PlumbIgnore {
        PlumbIgnore {
            patterns: DEFAULT_IGNORES.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    #[test]
    fn plumbignore_skips_built_in_directories() {
        let ignore = ignore_with_defaults();
        assert!(ignore.matches(&PathBuf::from("/repo/target/debug/foo")));
        assert!(ignore.matches(&PathBuf::from("/repo/.git/index")));
        assert!(ignore.matches(&PathBuf::from("/repo/node_modules/lib/index.js")));
    }

    #[test]
    fn plumbignore_keeps_source_paths() {
        let ignore = ignore_with_defaults();
        assert!(!ignore.matches(&PathBuf::from("/repo/src/main.rs")));
    }

    #[test]
    fn actionable_events_include_modify_create_remove() {
        assert!(is_actionable(EventKind::Modify(ModifyKind::Any)));
        assert!(is_actionable(EventKind::Create(CreateKind::File)));
        assert!(is_actionable(EventKind::Remove(RemoveKind::File)));
    }

    #[test]
    fn access_events_are_ignored() {
        assert!(!is_actionable(EventKind::Access(AccessKind::Any)));
    }
}
