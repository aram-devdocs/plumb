//! `plumb explain <rule>` — print the long-form documentation for a rule.
//!
//! Reads from `docs/src/rules/<slug>.md` relative to the binary or CWD.
//! The rule id `spacing/grid-conformance` maps to
//! `docs/src/rules/spacing-grid-conformance.md`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};

pub fn run(rule: &str) -> Result<ExitCode> {
    let slug = rule.replace('/', "-");
    let relative = PathBuf::from(format!("docs/src/rules/{slug}.md"));

    // Try CWD first — normal dev workflow. Then fall back to the
    // sibling-of-binary layout that cargo-dist installs produce.
    let candidates = [relative.clone(), binary_relative(&relative)?];
    let Some(candidate) = candidates.iter().find(|p| p.exists()) else {
        // Don't leak install paths in the error message. A
        // user-facing "no documentation found" plus a pointer at
        // `list_rules` and the docs site is everything callers need;
        // the internal search-path list (`/Users/<name>/.nvm/...`)
        // belongs at debug verbosity, where the candidate paths are
        // already in scope via tracing below.
        tracing::debug!(
            rule = %rule,
            candidates = ?candidates,
            "explain: no documentation file matched any candidate"
        );
        return Err(unknown_rule_error(rule));
    };
    let content = std::fs::read_to_string(candidate)
        .with_context(|| format!("read {}", candidate.display()))?;
    #[allow(clippy::print_stdout)]
    {
        print!("{content}");
    }
    Ok(ExitCode::SUCCESS)
}

/// Build the user-facing error returned when no documentation file
/// matches the supplied rule id.
///
/// The wording deliberately:
///   - names the bad rule id back to the user;
///   - points at `list_rules` (MCP) and the docs site so the next step
///     is obvious;
///   - omits every absolute path so npm-shim install layouts
///     (`/Users/<name>/.nvm/...`) stay private.
fn unknown_rule_error(rule: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "no documentation found for rule `{rule}`.\n       \
         Run `plumb mcp` and call `list_rules` to see all valid rule ids,\n       \
         or browse https://plumb.aramhammoudeh.com/rules/overview.html.\n       \
         (Pass `-v` to log the candidate paths the lookup walked.)"
    )
}

fn binary_relative(relative: &Path) -> Result<PathBuf> {
    let exe = std::env::current_exe().context("current_exe")?;
    let install_dir = exe.parent().context("exe has no parent")?;
    Ok(install_dir.join(relative))
}

#[cfg(test)]
mod tests {
    use super::unknown_rule_error;

    /// The user-facing error for an unknown rule MUST NOT leak any
    /// absolute filesystem path. Specifically: the npm-shim install
    /// layout produces `/Users/<name>/.nvm/...` paths, and an earlier
    /// implementation surfaced them via `bail!("...{candidates:?}")`.
    /// The fix returns a friendly message; this test pins that
    /// contract so a future refactor can't regress it.
    #[test]
    fn unknown_rule_error_does_not_leak_absolute_paths() {
        let err = unknown_rule_error("not/a-real-rule");
        let rendered = format!("{err}");
        assert!(
            !rendered.contains("/Users/"),
            "unknown-rule error must not leak absolute home paths: {rendered}"
        );
        assert!(
            !rendered.contains("/.nvm/"),
            "unknown-rule error must not leak npm install paths: {rendered}"
        );
        assert!(
            !rendered.contains("docs/src/rules"),
            "unknown-rule error must not leak the internal docs layout: {rendered}"
        );
    }

    /// The error MUST direct the user at the `list_rules` MCP tool and
    /// the docs site so they know what to do next.
    #[test]
    fn unknown_rule_error_points_at_list_rules_and_docs_site() {
        let err = unknown_rule_error("not/a-real-rule");
        let rendered = format!("{err}");
        assert!(
            rendered.contains("list_rules"),
            "error must mention `list_rules`: {rendered}"
        );
        assert!(
            rendered.contains("https://plumb.aramhammoudeh.com/rules/"),
            "error must point at the docs site: {rendered}"
        );
        assert!(
            rendered.contains("not/a-real-rule"),
            "error must echo the bad rule id back: {rendered}"
        );
    }
}
