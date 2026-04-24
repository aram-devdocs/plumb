//! # plumb-format
//!
//! Output formatters for Plumb violations.
//!
//! Every formatter is pure: given the same slice of violations, it
//! produces byte-identical output. Formatters never read the environment,
//! the clock, or the filesystem.
//!
//! Four formats are supported, each matching a subsection of
//! `docs/local/prd.md` §13:
//!
//! - [`pretty`] — human-readable TTY output.
//! - [`json`] — canonical machine-readable format.
//! - [`sarif`] — SARIF 2.1.0 for GitHub code-scanning and IDEs.
//! - [`mcp_compact`] — token-efficient output for the MCP server; returns
//!   a `(text, structured)` pair matching PRD §14.2.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use plumb_core::{Severity, Violation};
use serde_json::{Value, json};

/// Render a slice of violations as a pretty, human-readable block.
///
/// No ANSI escapes — coloring is a CLI concern, not a library concern.
#[must_use]
pub fn pretty(violations: &[Violation]) -> String {
    if violations.is_empty() {
        return String::from("No violations.\n");
    }
    let mut out = String::new();
    for v in violations {
        out.push_str(&format!(
            "{level:>7} {rule}\n         at {selector} [{viewport}]\n         {msg}\n",
            level = v.severity.label(),
            rule = v.rule_id,
            selector = v.selector,
            viewport = v.viewport.as_str(),
            msg = v.message,
        ));
        if let Some(fix) = &v.fix {
            out.push_str(&format!("         fix: {}\n", fix.description));
        }
        out.push_str(&format!("         docs: {}\n\n", v.doc_url));
    }
    out.push_str(&summary_line(violations));
    out.push('\n');
    out
}

/// Render a slice of violations as canonical, pretty-printed JSON.
///
/// # Errors
///
/// Returns an error if serialization fails, which in practice only
/// happens when a `Violation::metadata` contains a non-JSON-representable
/// value.
pub fn json(violations: &[Violation]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(violations)
}

/// Render a slice of violations as SARIF 2.1.0.
///
/// This is a minimal conformant document — the real rule metadata is
/// attached as a placeholder. Downstream PRs enrich it with `helpUri`,
/// `defaultConfiguration`, etc. per the SARIF spec.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn sarif(violations: &[Violation]) -> Result<String, serde_json::Error> {
    let results: Vec<Value> = violations
        .iter()
        .map(|v| {
            json!({
                "ruleId": v.rule_id,
                "level": match v.severity {
                    Severity::Error => "error",
                    Severity::Warning => "warning",
                    Severity::Info => "note",
                },
                "message": { "text": v.message },
                "locations": [{
                    "logicalLocations": [{
                        "fullyQualifiedName": v.selector,
                        "kind": "element",
                    }],
                    "properties": {
                        "viewport": v.viewport.as_str(),
                        "domOrder": v.dom_order,
                    }
                }],
                "properties": {
                    "docUrl": v.doc_url,
                }
            })
        })
        .collect();

    let doc = json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "plumb",
                    "informationUri": "https://plumb.dev",
                    "rules": [],
                }
            },
            "results": results,
        }]
    });
    serde_json::to_string_pretty(&doc)
}

/// Render a slice of violations as MCP-compact output — a token-efficient
/// text block plus a structured JSON sidecar.
///
/// This matches PRD §14.2. AI coding agents consume the structured block
/// for machine-readable decisions and surface the text block to the user.
#[must_use]
pub fn mcp_compact(violations: &[Violation]) -> (String, Value) {
    let mut text = String::new();
    for v in violations {
        text.push_str(&format!(
            "{severity} {rule} @ {selector} [{viewport}]: {message}\n",
            severity = v.severity.label(),
            rule = v.rule_id,
            selector = v.selector,
            viewport = v.viewport.as_str(),
            message = v.message,
        ));
    }
    if violations.is_empty() {
        text.push_str("ok: 0 violations\n");
    } else {
        text.push_str(&summary_line(violations));
        text.push('\n');
    }

    let structured = json!({
        "violations": violations,
        "counts": counts(violations),
    });

    (text, structured)
}

fn counts(violations: &[Violation]) -> Value {
    let (mut err, mut warn, mut info) = (0usize, 0usize, 0usize);
    for v in violations {
        match v.severity {
            Severity::Error => err += 1,
            Severity::Warning => warn += 1,
            Severity::Info => info += 1,
        }
    }
    json!({
        "error": err,
        "warning": warn,
        "info": info,
        "total": violations.len(),
    })
}

fn summary_line(violations: &[Violation]) -> String {
    let c = counts(violations);
    format!(
        "{total} violations ({errors} error, {warnings} warning, {infos} info)",
        total = c["total"],
        errors = c["error"],
        warnings = c["warning"],
        infos = c["info"],
    )
}
