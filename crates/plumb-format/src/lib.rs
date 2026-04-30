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
//! - [`sarif_with_rules`] — SARIF 2.1.0 for GitHub code-scanning and IDEs.
//! - [`mcp_compact`] — token-efficient output for the MCP server; returns
//!   a `(text, structured)` pair matching PRD §14.2.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::fmt::Write as _;

use plumb_core::{RuleMetadata, Severity, Violation};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

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
    let sorted = pretty_sorted(violations);
    let run_id = match run_id_for_sorted(&sorted) {
        Ok(run_id) => run_id,
        // `Violation` is JSON-serializable by construction, so this
        // branch should be unreachable in practice. Keep the pretty
        // formatter infallible and deterministic anyway.
        Err(_) => String::from("sha256:unavailable"),
    };

    let mut out = String::new();

    if sorted.is_empty() {
        out.push_str("No violations.\n");
    } else {
        let mut current_viewport: Option<&str> = None;
        let mut current_rule: Option<&str> = None;
        let mut current_selector: Option<&str> = None;

        for violation in &sorted {
            let viewport = violation.viewport.as_str();
            if current_viewport != Some(viewport) {
                if current_viewport.is_some() {
                    out.push('\n');
                }
                let _ = writeln!(out, "{viewport}");
                current_viewport = Some(viewport);
                current_rule = None;
                current_selector = None;
            }

            if current_rule != Some(violation.rule_id.as_str()) {
                let _ = writeln!(out, "  {}", violation.rule_id);
                current_rule = Some(violation.rule_id.as_str());
                current_selector = None;
            }

            if current_selector != Some(violation.selector.as_str()) {
                let _ = writeln!(out, "    {}", violation.selector);
                current_selector = Some(violation.selector.as_str());
            }

            let _ = writeln!(
                out,
                "      {}: {}",
                violation.severity.label(),
                violation.message
            );
            if let Some(fix) = &violation.fix {
                let _ = writeln!(out, "      fix: {}", fix.description);
            }
            let _ = writeln!(out, "      docs: {}", violation.doc_url);
        }
    }

    append_pretty_stats(&mut out, violations, &run_id);
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
    let sorted = canonical_sorted(violations);

    let run_id = run_id_for_sorted(&sorted)?;
    let stats = stats_json(&sorted, &run_id);

    // Build the envelope with alphabetically ordered keys so the
    // output is stable regardless of `serde_json`'s `preserve_order`
    // feature being enabled in the workspace.
    let mut envelope = serde_json::Map::new();
    envelope.insert(
        "plumb_version".to_owned(),
        Value::String(PLUMB_VERSION.to_owned()),
    );
    envelope.insert("run_id".to_owned(), Value::String(run_id));
    envelope.insert("stats".to_owned(), stats.clone());
    envelope.insert("summary".to_owned(), stats["counts"].clone());
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

/// Stable synthetic artifact URI used for every SARIF result.
///
/// GitHub Code Scanning's `locationFromSarifResult` rejects any result
/// whose first location does not carry a `physicalLocation`. Plumb
/// lints rendered URLs, not source files, so there is no real source artifact
/// to point at. The formatter emits this deterministic placeholder
/// instead. Viewport, DOM order, and the original CSS selector live on
/// the result's `logicalLocations` and location-level `properties`.
const SARIF_ARTIFACT_URI: &str = "plumb-lint-target";

/// Map a [`Severity`] to the SARIF `defaultConfiguration.level` string.
fn severity_to_sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

/// Build the SARIF `tool.driver.rules` array plus rule-id index map.
fn sarif_driver_rules_and_index(metadata: &[RuleMetadata]) -> (Vec<Value>, Vec<(String, usize)>) {
    let mut sorted: Vec<&RuleMetadata> = metadata.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));

    let mut index = Vec::with_capacity(sorted.len());
    let mut descriptors = Vec::with_capacity(sorted.len());

    for (i, rule) in sorted.iter().enumerate() {
        index.push((rule.id.clone(), i));
        let rule_name = rule.id.replace('/', "-");
        descriptors.push(json!({
            "id": rule.id,
            "name": rule_name,
            "shortDescription": { "text": rule.summary },
            "fullDescription": { "text": rule.summary },
            "helpUri": rule.doc_url,
            "defaultConfiguration": {
                "level": severity_to_sarif_level(rule.default_severity),
            },
        }));
    }

    (descriptors, index)
}

/// Render a slice of violations as SARIF 2.1.0 with caller-supplied rule metadata.
///
/// The output includes full rule metadata in `tool.driver.rules` (one
/// `reportingDescriptor` per rule with `shortDescription`,
/// `fullDescription`, `helpUri`, and `defaultConfiguration`), and each
/// result carries a `ruleIndex` pointing back into that array. Callers that
/// want a complete built-in rule table should pass
/// `plumb_core::builtin_rule_metadata()`. Keeping the registry lookup at
/// the caller boundary preserves this crate's formatter contract: output is
/// a pure function of explicit inputs.
///
/// Each result's first location carries a `physicalLocation` of the shape:
///
/// ```json
/// "physicalLocation": {
///   "artifactLocation": { "uri": "plumb-lint-target" },
///   "region": { "startLine": 1 }
/// }
/// ```
///
/// The artifact URI is the stable synthetic placeholder
/// `plumb-lint-target`. GitHub Code Scanning's
/// `locationFromSarifResult` requires every result to have a
/// `physicalLocation`, but Plumb violations have no source file — they are
/// tied to a rendered URL. The original selector, viewport, and DOM order
/// remain on the location's `logicalLocations` and `properties` blocks.
///
/// Results are sorted defensively by violation sort key, matching the JSON
/// formatter's behavior.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn sarif_with_rules(
    violations: &[Violation],
    rule_metadata: &[RuleMetadata],
) -> Result<String, serde_json::Error> {
    let (rules, index_map) = sarif_driver_rules_and_index(rule_metadata);

    let mut sorted: Vec<&Violation> = violations.iter().collect();
    sorted.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));

    let results: Vec<Value> = sorted
        .iter()
        .map(|v| {
            let rule_index = index_map
                .iter()
                .find(|(id, _)| id == &v.rule_id)
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
                    "physicalLocation": {
                        "artifactLocation": { "uri": SARIF_ARTIFACT_URI },
                        "region": { "startLine": 1 },
                    },
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

            if let Some(idx) = rule_index
                && let Some(obj) = result.as_object_mut()
            {
                obj.insert("ruleIndex".to_owned(), json!(idx));
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
        "counts": counts_json(violations),
    });

    (text, structured)
}

#[derive(Clone, Copy)]
struct SeverityCounts {
    error: usize,
    info: usize,
    warning: usize,
    total: usize,
}

fn canonical_sorted(violations: &[Violation]) -> Vec<&Violation> {
    let mut sorted: Vec<&Violation> = violations.iter().collect();
    sorted.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    sorted
}

fn pretty_sorted(violations: &[Violation]) -> Vec<&Violation> {
    let mut sorted: Vec<&Violation> = violations.iter().collect();
    sorted.sort_by(|a, b| {
        (
            a.viewport.as_str(),
            a.rule_id.as_str(),
            a.selector.as_str(),
            a.dom_order,
        )
            .cmp(&(
                b.viewport.as_str(),
                b.rule_id.as_str(),
                b.selector.as_str(),
                b.dom_order,
            ))
    });
    sorted
}

fn run_id_for_sorted(sorted: &[&Violation]) -> Result<String, serde_json::Error> {
    let canonical = serde_json::to_vec(sorted)?;
    Ok(format!("sha256:{}", hex_digest(&canonical)))
}

fn counts(violations: &[Violation]) -> SeverityCounts {
    let (mut err, mut warn, mut info) = (0usize, 0usize, 0usize);
    for v in violations {
        match v.severity {
            Severity::Error => err += 1,
            Severity::Warning => warn += 1,
            Severity::Info => info += 1,
        }
    }
    SeverityCounts {
        error: err,
        info,
        warning: warn,
        total: violations.len(),
    }
}

fn counts_json(violations: &[Violation]) -> Value {
    let counts = counts(violations);
    // Insert in alphabetical order so the output is stable regardless
    // of `serde_json`'s `preserve_order` feature toggle.
    let mut map = serde_json::Map::new();
    map.insert("error".to_owned(), json!(counts.error));
    map.insert("info".to_owned(), json!(counts.info));
    map.insert("total".to_owned(), json!(counts.total));
    map.insert("warning".to_owned(), json!(counts.warning));
    Value::Object(map)
}

fn stats_json(sorted: &[&Violation], run_id: &str) -> Value {
    let owned: Vec<Violation> = sorted
        .iter()
        .map(|violation| (*violation).clone())
        .collect();
    let counts = counts_json(&owned);
    let viewport_count = sorted
        .iter()
        .map(|violation| violation.viewport.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let rule_count = sorted
        .iter()
        .map(|violation| violation.rule_id.as_str())
        .collect::<BTreeSet<_>>()
        .len();

    let mut map = serde_json::Map::new();
    map.insert("counts".to_owned(), counts);
    map.insert("rule_count".to_owned(), json!(rule_count));
    map.insert("run_id".to_owned(), Value::String(run_id.to_owned()));
    map.insert("viewport_count".to_owned(), json!(viewport_count));
    Value::Object(map)
}

fn summary_line(violations: &[Violation]) -> String {
    let c = counts(violations);
    format!(
        "{total} violations ({errors} error, {warnings} warning, {infos} info)",
        total = c.total,
        errors = c.error,
        warnings = c.warning,
        infos = c.info,
    )
}

fn append_pretty_stats(out: &mut String, violations: &[Violation], run_id: &str) {
    let sorted = canonical_sorted(violations);
    let viewport_count = sorted
        .iter()
        .map(|violation| violation.viewport.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let rule_count = sorted
        .iter()
        .map(|violation| violation.rule_id.as_str())
        .collect::<BTreeSet<_>>()
        .len();

    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
    out.push_str("stats\n");
    let _ = writeln!(out, "  run_id: {run_id}");
    let _ = writeln!(out, "  {}", summary_line(violations));
    let _ = writeln!(out, "  viewport_count: {viewport_count}");
    let _ = writeln!(out, "  rule_count: {rule_count}");
}
