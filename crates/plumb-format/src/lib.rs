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
//! - [`json()`] — canonical machine-readable format.
//! - [`sarif`] — SARIF 2.1.0 for GitHub code-scanning and IDEs.
//! - [`mcp_compact`] — token-efficient output for the MCP server; returns
//!   a `(text, structured)` pair matching PRD §14.2.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::fmt::Write as _;

use plumb_core::{Severity, Violation};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

mod rule_meta;

/// Plumb version string embedded in the JSON envelope.
///
/// Pinned to `plumb-format`'s `CARGO_PKG_VERSION` because the envelope
/// shape is owned by this crate. The workspace version-bumps in
/// lockstep, so this resolves to the same value as the `plumb` binary
/// in practice; sourcing it from this crate keeps the formatter
/// self-contained and avoids a needless dependency cycle through
/// `plumb-cli`.
const PLUMB_VERSION: &str = env!("CARGO_PKG_VERSION");

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
        let _ = writeln!(
            out,
            "{level:>7} {rule}\n         at {selector} [{viewport}]\n         {msg}",
            level = v.severity.label(),
            rule = v.rule_id,
            selector = v.selector,
            viewport = v.viewport.as_str(),
            msg = v.message,
        );
        if let Some(fix) = &v.fix {
            let _ = writeln!(out, "         fix: {}", fix.description);
        }
        let _ = writeln!(out, "         docs: {}\n", v.doc_url);
    }
    out.push_str(&summary_line(violations));
    out.push('\n');
    out
}

/// Render a slice of violations as canonical, pretty-printed JSON.
///
/// # Envelope
///
/// The output is an object with these top-level fields, written in
/// alphabetical key order:
///
/// - `plumb_version` — the `plumb-format` crate version at compile
///   time. The workspace ships every crate with the same version, so
///   this matches the `plumb` binary version too.
/// - `run_id` — a content-derived identifier of the violations payload
///   (see below).
/// - `summary` — `{ "error": N, "info": N, "total": N, "warning": N }`,
///   keys also in alphabetical order.
/// - `violations` — the violations array, sorted by
///   [`plumb_core::Violation::sort_key`].
///
/// The workspace enables `serde_json/preserve_order` via `schemars`, so
/// `serde_json::Map` is `IndexMap`-backed and preserves insertion
/// order. The envelope inserts keys alphabetically to keep the output
/// independent of that crate-feature toggle.
///
/// # `run_id` derivation
///
/// `run_id = "sha256:" + hex(Sha256(serde_json::to_vec(&sorted_violations)))`
///
/// The hash input is the **compact** (non-pretty) JSON serialization of
/// the sorted violations array — not the pretty-printed envelope —
/// which means whitespace tweaks in the output never shift the hash,
/// and a `plumb_version` bump never shifts it either. Two runs with
/// the same violations always produce the same `run_id`; any
/// observable change in a violation flips the digest.
///
/// The formatter re-sorts violations defensively before hashing and
/// serializing. The engine already sorts on its way out, but the
/// formatter does not depend on caller invariants.
///
/// # Errors
///
/// Returns an error if serialization fails, which in practice only
/// happens when a `Violation::metadata` contains a non-JSON-representable
/// value.
pub fn json(violations: &[Violation]) -> Result<String, serde_json::Error> {
    let mut sorted: Vec<&Violation> = violations.iter().collect();
    sorted.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));

    let canonical = serde_json::to_vec(&sorted)?;
    let run_id = format!("sha256:{}", hex_digest(&canonical));

    // Build the envelope with alphabetically ordered keys so the
    // output is stable regardless of `serde_json`'s `preserve_order`
    // feature being enabled in the workspace.
    let mut envelope = serde_json::Map::new();
    envelope.insert(
        "plumb_version".to_owned(),
        Value::String(PLUMB_VERSION.to_owned()),
    );
    envelope.insert("run_id".to_owned(), Value::String(run_id));
    envelope.insert("summary".to_owned(), counts(violations));
    envelope.insert("violations".to_owned(), serde_json::to_value(&sorted)?);
    serde_json::to_string_pretty(&Value::Object(envelope))
}

/// Hex-alphabet table used by [`hex_digest`].
const HEX_TABLE: &[u8; 16] = b"0123456789abcdef";

/// Hex-encode a SHA-256 digest of `bytes` without an extra dependency.
fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let upper = HEX_TABLE[(byte >> 4) as usize];
        let lower = HEX_TABLE[(byte & 0x0f) as usize];
        hex.push(char::from(upper));
        hex.push(char::from(lower));
    }
    hex
}

/// Render a slice of violations as SARIF 2.1.0.
///
/// The output includes full rule metadata in `tool.driver.rules` (one
/// `reportingDescriptor` per built-in rule with `shortDescription`,
/// `fullDescription`, `helpUri`, and `defaultConfiguration`), and each
/// result carries a `ruleIndex` pointing back into that array.
///
/// Results are sorted defensively by violation sort key, matching the
/// JSON formatter's behavior.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn sarif(violations: &[Violation]) -> Result<String, serde_json::Error> {
    let rules = rule_meta::driver_rules();
    let index_map = rule_meta::rule_index_map();

    let mut sorted: Vec<&Violation> = violations.iter().collect();
    sorted.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));

    let results: Vec<Value> = sorted
        .iter()
        .map(|v| {
            let rule_index = index_map
                .iter()
                .find(|(id, _)| *id == v.rule_id)
                .map(|(_, idx)| *idx);

            let mut result = json!({
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
            });

            if let Some(idx) = rule_index {
                result
                    .as_object_mut()
                    .map(|obj| obj.insert("ruleIndex".to_owned(), json!(idx)));
            }

            result
        })
        .collect();

    let doc = json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "plumb",
                    "informationUri": "https://plumb.aramhammoudeh.com",
                    "rules": rules,
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
        let _ = writeln!(
            text,
            "{severity} {rule} @ {selector} [{viewport}]: {message}",
            severity = v.severity.label(),
            rule = v.rule_id,
            selector = v.selector,
            viewport = v.viewport.as_str(),
            message = v.message,
        );
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
    // Insert in alphabetical order so the output is stable regardless
    // of `serde_json`'s `preserve_order` feature toggle.
    let mut map = serde_json::Map::new();
    map.insert("error".to_owned(), json!(err));
    map.insert("info".to_owned(), json!(info));
    map.insert("total".to_owned(), json!(violations.len()));
    map.insert("warning".to_owned(), json!(warn));
    Value::Object(map)
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
