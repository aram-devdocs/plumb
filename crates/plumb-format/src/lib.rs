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
//! - [`mcp_compact`] / [`mcp_compact_capped`] — token-efficient output
//!   for the MCP server; returns a `(text, structured)` pair matching
//!   PRD §14.2. Duplicate violations are aggregated server-side into
//!   capped findings under a hard 10 KB `structuredContent` budget.
//!
//! [`pretty_with_suggested_ignores`] and [`json_with_suggested_ignores`]
//! extend the `pretty` and `json` shapes with a `.plumbignore` proposal
//! derived from the current violations. `plumb lint --suggest-ignores`
//! routes through them.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
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
    pretty_with_ignored(violations, 0)
}

/// Like [`pretty`] but appends a `N violation(s) suppressed by config`
/// footer line when `ignored_count > 0`.
///
/// Used by `plumb lint` when the loaded `plumb.toml` has `[[ignore]]`
/// entries that filtered some violations out of the reported set. The
/// footer documents that the loaded config silenced violations rather
/// than the lint pass missing them.
///
/// `ignored_count == 0` produces the same output as [`pretty`].
#[must_use]
pub fn pretty_with_ignored(violations: &[Violation], ignored_count: usize) -> String {
    pretty_capped(violations, None, ignored_count)
}

/// Render a pretty block whose stats reflect the FULL `violations` slice
/// while the findings body shows only the first `display_cap` of them.
///
/// `plumb lint --max-findings N` routes through here. The stats block —
/// `run_id`, the violation count line, `viewport_count`, and `rule_count`
/// — is always computed from every violation in `violations`, so it
/// matches the JSON envelope's `stats` for the same filtered set no matter
/// how small `display_cap` is. `run_id` is the SHA-256 of the canonically
/// sorted full set, identical to [`json()`]'s `run_id`, so the pretty and
/// JSON renders of one run agree on it byte for byte.
///
/// `display_cap == None`, or a cap at or above `violations.len()`, renders
/// every finding. A cap of `Some(0)` renders no findings but still prints
/// the full stats. `ignored_count` appends the
/// `N violation(s) suppressed by config` footer when non-zero.
#[must_use]
pub fn pretty_capped(
    violations: &[Violation],
    display_cap: Option<usize>,
    ignored_count: usize,
) -> String {
    // `run_id` hashes the canonically sorted FULL set — the same input
    // the JSON envelope hashes — so the two formats never disagree on it.
    let canonical = canonical_sorted(violations);
    let run_id = match run_id_for_sorted(&canonical) {
        Ok(run_id) => run_id,
        // `Violation` is JSON-serializable by construction, so this
        // branch should be unreachable in practice. Keep the pretty
        // formatter infallible and deterministic anyway.
        Err(_) => String::from("sha256:unavailable"),
    };

    // The body renders in pretty display order; the cap keeps the first
    // `display_cap` findings as they appear top to bottom.
    let sorted = pretty_sorted(violations);
    let shown: &[&Violation] = match display_cap {
        Some(n) if n < sorted.len() => &sorted[..n],
        _ => &sorted,
    };

    let mut out = String::new();

    if violations.is_empty() {
        out.push_str("No violations.\n");
    } else {
        let mut current_viewport: Option<&str> = None;
        let mut current_rule: Option<&str> = None;
        let mut current_selector: Option<&str> = None;

        for violation in shown {
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
    if ignored_count > 0 {
        let _ = writeln!(
            out,
            "  {ignored_count} violation{plural} suppressed by config",
            plural = if ignored_count == 1 { "" } else { "s" }
        );
    }
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
    json_with_ignored(violations, 0)
}

/// Like [`json()`] but extends the envelope with `"ignored": N`
/// counting violations suppressed by `[[ignore]]` config entries.
///
/// `ignored_count == 0` produces the same output as [`json()`] —
/// existing consumers parse the envelope by key and the new key is
/// always present.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn json_with_ignored(
    violations: &[Violation],
    ignored_count: usize,
) -> Result<String, serde_json::Error> {
    let sorted = canonical_sorted(violations);

    let run_id = run_id_for_sorted(&sorted)?;
    let stats = stats_json(&sorted, &run_id);

    // Build the envelope with alphabetically ordered keys so the
    // output is stable regardless of `serde_json`'s `preserve_order`
    // feature being enabled in the workspace.
    let mut envelope = serde_json::Map::new();
    envelope.insert(
        "ignored".to_owned(),
        Value::Number(serde_json::Number::from(ignored_count)),
    );
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

/// Compute the deterministic list of `(rule_id, selector)` ignore
/// suggestions for a violations slice.
///
/// Each unique pair surfaces exactly once. The list is sorted by
/// `(rule_id, selector)` so two runs with the same violations produce
/// byte-identical output. Used by [`pretty_with_suggested_ignores`] and
/// [`json_with_suggested_ignores`] and re-exported for callers that
/// need the raw pairs.
#[must_use]
pub fn suggested_ignores(violations: &[Violation]) -> Vec<(String, String)> {
    let mut pairs: BTreeSet<(String, String)> = BTreeSet::new();
    for v in violations {
        pairs.insert((v.rule_id.clone(), v.selector.clone()));
    }
    pairs.into_iter().collect()
}

/// Render a pretty block followed by a suggested `.plumbignore` footer.
///
/// The footer contains one line per `(rule_id, selector)` tuple that
/// would suppress a current violation, sorted by `(rule_id, selector)`.
/// Two header lines describe the format. When `violations` is empty the
/// footer reads `(no violations)` and lists no entries.
#[must_use]
pub fn pretty_with_suggested_ignores(violations: &[Violation]) -> String {
    pretty_with_suggested_ignores_and_ignored(violations, 0)
}

/// Combines [`pretty_with_suggested_ignores`] with the
/// `N violation(s) suppressed by config` footer from
/// [`pretty_with_ignored`]. The "suppressed by config" line goes inside
/// the stats block; the suggested-ignores list comes after.
#[must_use]
pub fn pretty_with_suggested_ignores_and_ignored(
    violations: &[Violation],
    ignored_count: usize,
) -> String {
    pretty_capped_with_suggested_ignores(violations, None, ignored_count)
}

/// Like [`pretty_capped`] but appends the suggested `.plumbignore` block.
///
/// The block — and its `would suppress N violation(s)` count — reflects
/// the FULL `violations` slice, matching the JSON `suggested_ignores`
/// array, even when `display_cap` hides some findings from the body.
#[must_use]
pub fn pretty_capped_with_suggested_ignores(
    violations: &[Violation],
    display_cap: Option<usize>,
    ignored_count: usize,
) -> String {
    let mut out = pretty_capped(violations, display_cap, ignored_count);
    append_suggested_ignores_block(&mut out, violations);
    out
}

/// Same as [`json()`] but extends the envelope with a
/// `suggested_ignores` array. Each element is `{ "rule_id": …,
/// "selector": … }`, sorted by `(rule_id, selector)`.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn json_with_suggested_ignores(violations: &[Violation]) -> Result<String, serde_json::Error> {
    json_with_suggested_ignores_and_ignored(violations, 0)
}

/// Combines [`json_with_suggested_ignores`] (the `suggested_ignores`
/// array) with [`json_with_ignored`] (the `ignored` count) so callers
/// that pass both `--suggest-ignores` and a config with `[[ignore]]`
/// entries get both pieces in the envelope.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn json_with_suggested_ignores_and_ignored(
    violations: &[Violation],
    ignored_count: usize,
) -> Result<String, serde_json::Error> {
    let sorted = canonical_sorted(violations);

    let run_id = run_id_for_sorted(&sorted)?;
    let stats = stats_json(&sorted, &run_id);
    let suggestions = suggested_ignores_json(violations);

    let mut envelope = serde_json::Map::new();
    envelope.insert(
        "ignored".to_owned(),
        Value::Number(serde_json::Number::from(ignored_count)),
    );
    envelope.insert(
        "plumb_version".to_owned(),
        Value::String(PLUMB_VERSION.to_owned()),
    );
    envelope.insert("run_id".to_owned(), Value::String(run_id));
    envelope.insert("stats".to_owned(), stats.clone());
    envelope.insert("suggested_ignores".to_owned(), suggestions);
    envelope.insert("summary".to_owned(), stats["counts"].clone());
    envelope.insert("violations".to_owned(), serde_json::to_value(&sorted)?);
    serde_json::to_string_pretty(&Value::Object(envelope))
}

fn suggested_ignores_json(violations: &[Violation]) -> Value {
    let pairs = suggested_ignores(violations);
    let entries: Vec<Value> = pairs
        .into_iter()
        .map(|(rule_id, selector)| {
            // Build each entry with alphabetically ordered keys so the
            // output is stable regardless of `serde_json`'s
            // `preserve_order` feature toggle.
            let mut map = serde_json::Map::new();
            map.insert("rule_id".to_owned(), Value::String(rule_id));
            map.insert("selector".to_owned(), Value::String(selector));
            Value::Object(map)
        })
        .collect();
    Value::Array(entries)
}

fn append_suggested_ignores_block(out: &mut String, violations: &[Violation]) {
    let pairs = suggested_ignores(violations);

    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
    let _ = writeln!(
        out,
        "Suggested .plumbignore (would suppress {count} {noun}):",
        count = violations.len(),
        noun = if violations.len() == 1 {
            "violation"
        } else {
            "violations"
        },
    );
    out.push_str("# Format: <rule_id> <selector_path>\n");
    if pairs.is_empty() {
        out.push_str("# (no violations)\n");
    } else {
        for (rule_id, selector) in pairs {
            let _ = writeln!(out, "{rule_id} {selector}");
        }
    }
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

/// Default cap on the number of aggregated findings [`mcp_compact`]
/// returns. The hard 10 KB byte budget ([`MCP_COMPACT_RESPONSE_CAP_BYTES`])
/// may drop additional groups beyond this when messages are unusually
/// long.
const MCP_COMPACT_DEFAULT_MAX_FINDINGS: usize = 20;

/// Hard byte cap on the serialized `structuredContent` payload, per
/// `.agents/rules/mcp-tool-patterns.md` and PRD §14.2. The aggregation
/// pass guarantees [`mcp_compact_capped`] never returns a payload whose
/// compact JSON serialization exceeds this size.
const MCP_COMPACT_RESPONSE_CAP_BYTES: usize = 10 * 1024;

/// Maximum example selectors carried per aggregated finding. The full
/// per-element list is intentionally never echoed back — it burns the
/// agent's token budget for no decision value.
const MCP_COMPACT_EXAMPLE_CAP: usize = 3;

/// Render a slice of violations as MCP-compact output — a token-efficient
/// text block plus a structured JSON sidecar.
///
/// Aggregates with [`mcp_compact_capped`] at the default finding cap of
/// 20. This matches PRD §14.2: AI coding agents consume the structured
/// block for machine-readable decisions and surface the text block to
/// the user.
#[must_use]
pub fn mcp_compact(violations: &[Violation]) -> (String, Value) {
    mcp_compact_capped(violations, MCP_COMPACT_DEFAULT_MAX_FINDINGS)
}

/// Aggregate violations into capped findings and render the MCP-compact
/// `(text, structured)` pair under a hard 10 KB `structuredContent`
/// budget.
///
/// # Aggregation
///
/// Violations are grouped by `(rule_id, normalized_message)`, where the
/// normalized message erases the element-specific parts of the text —
/// the node's own selector, numeric literals, and hex color codes — so
/// that the same defect across many elements collapses into one finding.
/// Each finding carries the representative message, an `instances`
/// count, up to `MCP_COMPACT_EXAMPLE_CAP` example selectors, the
/// representative fix description, and the doc URL.
///
/// # Structured payload
///
/// The returned `structuredContent` object has four keys, in alphabetical
/// order:
///
/// - `by_rule` — `rule_id` → instance count across every violation.
/// - `counts` — `{ error, info, total, warning }` across every violation.
/// - `findings` — at most `max_findings` aggregated groups, sorted by
///   `(severity desc, rule_id, representative selector)`.
/// - `truncated` — `true` when groups were dropped to fit either the
///   finding cap or the 10 KB byte budget.
///
/// `counts` and `by_rule` always reflect **all** input violations, even
/// when findings are dropped. The byte budget is enforced by first
/// dropping example selectors, then dropping the lowest-severity groups,
/// until the payload fits — the function never returns more than 10 KB.
///
/// # Determinism
///
/// Output is a pure function of the input slice. Violations are sorted
/// defensively before grouping, groups are sorted by a total order, and
/// every map is emitted in sorted-key order.
#[must_use]
pub fn mcp_compact_capped(violations: &[Violation], max_findings: usize) -> (String, Value) {
    let sorted = canonical_sorted(violations);

    // Group by (rule_id, normalized message). A BTreeMap keeps the build
    // deterministic; the representative fields are seeded on first
    // insertion, and because `sorted` is in sort-key order the
    // representative is always the lowest-sort-key violation of the group.
    let mut groups: BTreeMap<(String, String), FindingGroup> = BTreeMap::new();
    for v in &sorted {
        let key = (
            v.rule_id.clone(),
            normalize_message(&v.message, &v.selector),
        );
        let group = groups.entry(key).or_insert_with(|| FindingGroup {
            rule_id: v.rule_id.clone(),
            severity: v.severity,
            message: v.message.clone(),
            selector: v.selector.clone(),
            fix: v.fix.as_ref().map(|f| f.description.clone()),
            doc_url: v.doc_url.clone(),
            instances: 0,
            examples: Vec::new(),
        });
        group.instances += 1;
        if group.examples.len() < MCP_COMPACT_EXAMPLE_CAP
            && !group.examples.iter().any(|s| s == &v.selector)
        {
            group.examples.push(v.selector.clone());
        }
    }

    let mut findings: Vec<FindingGroup> = groups.into_values().collect();
    findings.sort_by(|a, b| {
        severity_rank(a.severity)
            .cmp(&severity_rank(b.severity))
            .then_with(|| a.rule_id.cmp(&b.rule_id))
            .then_with(|| a.selector.cmp(&b.selector))
    });

    let total_groups = findings.len();
    if findings.len() > max_findings {
        findings.truncate(max_findings);
    }

    let counts = counts_json(violations);
    let by_rule = by_rule_json(&sorted);

    let (structured, kept) = fit_structured_payload(&findings, total_groups, &counts, &by_rule);
    let text = compact_text(&findings[..kept], violations, total_groups);
    (text, structured)
}

/// One aggregated finding: a representative violation plus the counts and
/// examples that collapse a group of element-specific duplicates.
struct FindingGroup {
    rule_id: String,
    severity: Severity,
    message: String,
    /// Representative selector — the lowest-sort-key member of the group.
    /// Used both as the final tie-breaker in the group sort and as the
    /// first example.
    selector: String,
    fix: Option<String>,
    doc_url: String,
    instances: usize,
    examples: Vec<String>,
}

/// Severity sort rank: errors first, then warnings, then info. Used to
/// order findings (most severe first) and to decide drop order when the
/// byte budget is exceeded (least severe dropped first).
fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    }
}

/// Collapse a violation message into a grouping key by erasing the
/// element-specific parts: the node's own selector (rule messages embed
/// it in backticks, e.g. `` `html > body` has off-grid padding… ``),
/// numeric literals, and hex color codes. Two violations that differ
/// only by which element they point at — or by a slightly different
/// measured value — normalize to the same key.
fn normalize_message(message: &str, selector: &str) -> String {
    // Drop the selector verbatim. Guard the empty case: `str::replace`
    // with an empty pattern splices the replacement between every char.
    let without_selector = if selector.is_empty() {
        std::borrow::Cow::Borrowed(message)
    } else {
        std::borrow::Cow::Owned(message.replace(selector, "<sel>"))
    };

    let mut out = String::with_capacity(without_selector.len());
    let mut chars = without_selector.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '#' {
            // Gather the following hex run. A run of 3/4/6/8 hex digits is
            // a CSS color and collapses to a single placeholder; anything
            // else (e.g. an id selector fragment) is left intact.
            let mut hex = String::new();
            while let Some(&next) = chars.peek() {
                if next.is_ascii_hexdigit() {
                    hex.push(next);
                    chars.next();
                } else {
                    break;
                }
            }
            if matches!(hex.len(), 3 | 4 | 6 | 8) {
                out.push_str("<#>");
            } else {
                out.push('#');
                out.push_str(&hex);
            }
        } else if c.is_ascii_digit() {
            // Consume the rest of the number (digits + decimal point) and
            // collapse it. Trailing units (`px`, `rem`, `:1`) survive and
            // keep distinct shapes apart.
            while let Some(&next) = chars.peek() {
                if next.is_ascii_digit() || next == '.' {
                    chars.next();
                } else {
                    break;
                }
            }
            out.push_str("<n>");
        } else {
            out.push(c);
        }
    }
    out
}

/// `rule_id` → instance count across every violation, emitted in sorted
/// `rule_id` order for byte-stable output.
fn by_rule_json(sorted: &[&Violation]) -> Value {
    let mut tally: BTreeMap<&str, usize> = BTreeMap::new();
    for v in sorted {
        *tally.entry(v.rule_id.as_str()).or_insert(0) += 1;
    }
    let mut map = serde_json::Map::new();
    for (rule_id, count) in tally {
        map.insert(rule_id.to_owned(), json!(count));
    }
    Value::Object(map)
}

/// Build the `structuredContent` object from a finding slice. Keys are
/// inserted alphabetically so the output is stable regardless of
/// `serde_json`'s `preserve_order` feature toggle.
fn build_structured(
    findings: &[FindingGroup],
    counts: &Value,
    by_rule: &Value,
    include_examples: bool,
    truncated: bool,
) -> Value {
    let findings_json: Vec<Value> = findings
        .iter()
        .map(|f| {
            let mut map = serde_json::Map::new();
            map.insert("doc_url".to_owned(), Value::String(f.doc_url.clone()));
            if include_examples {
                map.insert(
                    "examples".to_owned(),
                    Value::Array(f.examples.iter().cloned().map(Value::String).collect()),
                );
            }
            map.insert(
                "fix".to_owned(),
                f.fix.clone().map_or(Value::Null, Value::String),
            );
            map.insert("instances".to_owned(), json!(f.instances));
            map.insert("message".to_owned(), Value::String(f.message.clone()));
            map.insert("rule_id".to_owned(), Value::String(f.rule_id.clone()));
            map.insert(
                "severity".to_owned(),
                Value::String(f.severity.label().to_owned()),
            );
            Value::Object(map)
        })
        .collect();

    let mut map = serde_json::Map::new();
    map.insert("by_rule".to_owned(), by_rule.clone());
    map.insert("counts".to_owned(), counts.clone());
    map.insert("findings".to_owned(), Value::Array(findings_json));
    map.insert("truncated".to_owned(), Value::Bool(truncated));
    Value::Object(map)
}

/// Enforce the 10 KB `structuredContent` budget. Returns the final
/// payload plus the number of findings kept (so the text block matches).
///
/// The shrink order is: (1) drop example selectors, (2) drop the
/// lowest-severity groups one at a time. `counts` and `by_rule` are
/// small and fixed, so the loop always terminates with a payload under
/// the cap.
fn fit_structured_payload(
    findings: &[FindingGroup],
    total_groups: usize,
    counts: &Value,
    by_rule: &Value,
) -> (Value, usize) {
    let mut include_examples = true;
    let mut kept = findings.len();
    loop {
        let truncated = total_groups > kept;
        let structured = build_structured(
            &findings[..kept],
            counts,
            by_rule,
            include_examples,
            truncated,
        );
        let size = serde_json::to_string(&structured).map_or(usize::MAX, |s| s.len());
        if size <= MCP_COMPACT_RESPONSE_CAP_BYTES {
            return (structured, kept);
        }
        if include_examples {
            include_examples = false;
            continue;
        }
        if kept == 0 {
            // counts + by_rule alone exceed the cap — pathological, but
            // still return the smallest payload we can rather than panic.
            return (structured, 0);
        }
        kept -= 1;
    }
}

/// Render the compact text block: one line per kept finding, then a
/// summary line. Bounded by the finding cap, so it never grows with the
/// raw violation count.
fn compact_text(findings: &[FindingGroup], all: &[Violation], total_groups: usize) -> String {
    let mut text = String::new();
    if all.is_empty() {
        text.push_str("ok: 0 violations\n");
        return text;
    }
    for f in findings {
        let _ = writeln!(
            text,
            "{severity} {rule} \u{d7}{instances}: {message}",
            severity = f.severity.label(),
            rule = f.rule_id,
            instances = f.instances,
            message = f.message,
        );
    }
    let _ = writeln!(
        text,
        "{summary} in {shown}/{groups} group{plural}",
        summary = summary_line(all),
        shown = findings.len(),
        groups = total_groups,
        plural = if total_groups == 1 { "" } else { "s" },
    );
    text
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

#[cfg(test)]
mod tests {
    use super::{
        MCP_COMPACT_RESPONSE_CAP_BYTES, json, json_with_ignored, json_with_suggested_ignores,
        json_with_suggested_ignores_and_ignored, mcp_compact, mcp_compact_capped, pretty,
        pretty_capped, pretty_capped_with_suggested_ignores, pretty_with_ignored,
        pretty_with_suggested_ignores, pretty_with_suggested_ignores_and_ignored,
        suggested_ignores,
    };
    use indexmap::IndexMap;
    use plumb_core::{Severity, ViewportKey, Violation};

    fn violation(rule_id: &str, selector: &str, viewport: &str, dom_order: u64) -> Violation {
        Violation {
            rule_id: rule_id.to_owned(),
            severity: Severity::Warning,
            message: "test".to_owned(),
            selector: selector.to_owned(),
            viewport: ViewportKey::new(viewport),
            rect: None,
            dom_order,
            fix: None,
            doc_url: format!(
                "https://plumb.aramhammoudeh.com/rules/{}",
                rule_id.replace('/', "-")
            ),
            metadata: IndexMap::new(),
        }
    }

    fn contrast_violation(selector: &str, ratio: &str, dom_order: u64) -> Violation {
        Violation {
            rule_id: "color/contrast-aa".to_owned(),
            severity: Severity::Error,
            message: format!(
                "`{selector}` has contrast ratio {ratio}:1; WCAG 2.1 AA requires at least 4.5:1 for normal text."
            ),
            selector: selector.to_owned(),
            viewport: ViewportKey::new("desktop"),
            rect: None,
            dom_order,
            fix: None,
            doc_url: "https://plumb.aramhammoudeh.com/rules/color-contrast-aa".to_owned(),
            metadata: IndexMap::new(),
        }
    }

    fn structured_byte_len(structured: &serde_json::Value) -> usize {
        serde_json::to_string(structured)
            .expect("structured payload serializes")
            .len()
    }

    #[test]
    fn mcp_compact_collapses_element_specific_duplicates() {
        // 700 low-contrast rows on distinct elements with slightly
        // different measured ratios — the HN meta-text case. Aggregation
        // MUST collapse them into a single finding while the counts still
        // reflect all 700.
        let violations: Vec<Violation> = (0..700)
            .map(|i| {
                let ratio = format!("{}.{}", 2 + i % 3, i % 10);
                contrast_violation(&format!("tr.athing > td.subtext-{i} > span"), &ratio, i)
            })
            .collect();

        let (_text, structured) = mcp_compact(&violations);

        assert_eq!(structured["counts"]["total"].as_u64(), Some(700));
        assert_eq!(structured["counts"]["error"].as_u64(), Some(700));
        let findings = structured["findings"].as_array().expect("findings array");
        assert_eq!(
            findings.len(),
            1,
            "700 element-specific duplicates collapse to one finding"
        );
        assert_eq!(findings[0]["instances"].as_u64(), Some(700));
        assert_eq!(
            findings[0]["examples"].as_array().map(Vec::len),
            Some(3),
            "examples are capped at three selectors"
        );
        assert_eq!(structured["truncated"].as_bool(), Some(false));
        assert!(
            structured_byte_len(&structured) <= MCP_COMPACT_RESPONSE_CAP_BYTES,
            "aggregated payload must stay under the 10 KB budget"
        );
    }

    #[test]
    fn mcp_compact_caps_findings_and_stays_under_budget() {
        // 700 violations spread across 40 distinct rule shapes — more
        // groups than the default cap, so findings are truncated, the
        // flag flips, counts still total 700, and the payload stays under
        // 10 KB.
        let violations: Vec<Violation> = (0..700)
            .map(|i| {
                let rule = format!("spacing/rule-{:02}", i % 40);
                let mut v = violation(&rule, &format!("div.item-{i}"), "desktop", i);
                v.message = "spacing drifts off the eight-pixel grid".to_owned();
                v
            })
            .collect();

        let (_text, structured) = mcp_compact(&violations);

        assert_eq!(structured["counts"]["total"].as_u64(), Some(700));
        let findings = structured["findings"].as_array().expect("findings array");
        assert!(
            findings.len() <= 20,
            "findings must be capped at the default of 20, got {}",
            findings.len()
        );
        assert_eq!(
            structured["truncated"].as_bool(),
            Some(true),
            "40 groups exceed the cap of 20, so truncated must be true"
        );
        // `by_rule` still accounts for every rule shape.
        assert_eq!(
            structured["by_rule"].as_object().map(serde_json::Map::len),
            Some(40)
        );
        assert!(
            structured_byte_len(&structured) <= MCP_COMPACT_RESPONSE_CAP_BYTES,
            "capped payload must stay under the 10 KB budget"
        );
    }

    #[test]
    fn mcp_compact_drops_groups_to_honor_hard_budget() {
        // Each group carries a ~1 KB message, so even a handful of groups
        // blow past 10 KB. The fit loop must drop example selectors and
        // then whole groups until the payload fits — never returning more
        // than 10 KB.
        let big = "x".repeat(1024);
        let violations: Vec<Violation> = (0..30)
            .map(|i| {
                let mut v = violation(&format!("spacing/rule-{i:02}"), "div", "desktop", i);
                v.message = format!("{big}-{i}");
                v
            })
            .collect();

        let (_text, structured) = mcp_compact_capped(&violations, 30);

        assert!(
            structured_byte_len(&structured) <= MCP_COMPACT_RESPONSE_CAP_BYTES,
            "payload with oversized messages must still be capped at 10 KB"
        );
        assert_eq!(structured["truncated"].as_bool(), Some(true));
        assert_eq!(structured["counts"]["total"].as_u64(), Some(30));
    }

    #[test]
    fn mcp_compact_empty_is_clean() {
        let (text, structured) = mcp_compact(&[]);
        assert_eq!(text, "ok: 0 violations\n");
        assert_eq!(structured["counts"]["total"].as_u64(), Some(0));
        assert_eq!(
            structured["findings"].as_array().map(Vec::is_empty),
            Some(true)
        );
        assert_eq!(structured["truncated"].as_bool(), Some(false));
    }

    #[test]
    fn mcp_compact_is_byte_identical_regardless_of_input_order() {
        let forward: Vec<Violation> = (0..50)
            .map(|i| contrast_violation(&format!("span.row-{i}"), "3.1", i))
            .collect();
        let mut reversed = forward.clone();
        reversed.reverse();

        let (ta, sa) = mcp_compact(&forward);
        let (tb, sb) = mcp_compact(&reversed);
        assert_eq!(ta, tb, "text must not depend on input order");
        assert_eq!(sa, sb, "structured payload must not depend on input order");
    }

    #[test]
    fn suggested_ignores_dedupes_across_viewports() {
        // The same `(rule_id, selector)` pair fires on two viewports;
        // the suggested-ignores list collapses it to one entry.
        let v = vec![
            violation("spacing/grid-conformance", "body", "desktop", 1),
            violation("spacing/grid-conformance", "body", "mobile", 1),
        ];
        let pairs = suggested_ignores(&v);
        assert_eq!(
            pairs,
            vec![("spacing/grid-conformance".to_owned(), "body".to_owned())]
        );
    }

    #[test]
    fn suggested_ignores_sorts_by_rule_then_selector() {
        // Input is intentionally unsorted; output MUST be sorted by
        // `(rule_id, selector)` for byte-identical determinism.
        let v = vec![
            violation("spacing/grid-conformance", ".footer", "desktop", 3),
            violation("color/palette-conformance", "#cta", "desktop", 1),
            violation("spacing/grid-conformance", ".header", "desktop", 2),
        ];
        let pairs = suggested_ignores(&v);
        assert_eq!(
            pairs,
            vec![
                ("color/palette-conformance".to_owned(), "#cta".to_owned()),
                ("spacing/grid-conformance".to_owned(), ".footer".to_owned()),
                ("spacing/grid-conformance".to_owned(), ".header".to_owned()),
            ]
        );
    }

    #[test]
    fn suggested_ignores_empty_for_no_violations() {
        assert!(suggested_ignores(&[]).is_empty());
    }

    #[test]
    fn pretty_with_suggested_ignores_appends_block() {
        let v = vec![violation("spacing/grid-conformance", "body", "desktop", 1)];
        let out = pretty_with_suggested_ignores(&v);
        assert!(out.contains("Suggested .plumbignore (would suppress 1 violation):"));
        assert!(out.contains("# Format: <rule_id> <selector_path>"));
        assert!(out.contains("spacing/grid-conformance body"));
    }

    #[test]
    fn pretty_with_suggested_ignores_handles_empty() {
        let out = pretty_with_suggested_ignores(&[]);
        assert!(out.contains("Suggested .plumbignore (would suppress 0 violations):"));
        assert!(out.contains("(no violations)"));
    }

    #[test]
    fn json_with_suggested_ignores_emits_sorted_array() {
        let v = vec![
            violation("spacing/grid-conformance", ".footer", "desktop", 3),
            violation("color/palette-conformance", "#cta", "desktop", 1),
        ];
        let raw = json_with_suggested_ignores(&v).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("envelope is valid JSON");
        let arr = parsed
            .get("suggested_ignores")
            .and_then(|v| v.as_array())
            .expect("suggested_ignores array");
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["rule_id"], "color/palette-conformance");
        assert_eq!(arr[0]["selector"], "#cta");
        assert_eq!(arr[1]["rule_id"], "spacing/grid-conformance");
        assert_eq!(arr[1]["selector"], ".footer");
    }

    #[test]
    fn json_with_suggested_ignores_is_byte_deterministic() {
        let v = vec![
            violation("spacing/grid-conformance", ".header", "mobile", 1),
            violation("color/palette-conformance", "#cta", "desktop", 1),
        ];
        let a = json_with_suggested_ignores(&v).expect("serialize a");
        let b = json_with_suggested_ignores(&v).expect("serialize b");
        assert_eq!(a, b);
    }

    #[test]
    fn pretty_with_ignored_zero_matches_pretty() {
        let v = vec![violation("spacing/grid-conformance", "body", "desktop", 1)];
        assert_eq!(pretty(&v), pretty_with_ignored(&v, 0));
    }

    /// A three-violation set spanning two viewports and two rules. Each
    /// finding renders as a `warning: test` line, so counting that
    /// substring gives the number of findings in the body.
    fn capped_fixture() -> Vec<Violation> {
        vec![
            violation("a/one", "sel1", "desktop", 1),
            violation("b/two", "sel2", "desktop", 2),
            violation("a/one", "sel3", "mobile", 3),
        ]
    }

    #[test]
    fn pretty_capped_none_matches_pretty_with_ignored() {
        // `display_cap == None` is the uncapped path — identical output to
        // the existing entry point, so the refactor is behavior-preserving.
        let v = capped_fixture();
        assert_eq!(pretty_capped(&v, None, 0), pretty_with_ignored(&v, 0));
    }

    #[test]
    fn pretty_capped_stats_reflect_full_set_not_the_cap() {
        let v = capped_fixture();
        let out = pretty_capped(&v, Some(1), 0);

        // Only the first finding (pretty order: desktop a/one sel1) renders.
        assert_eq!(
            out.matches("      warning: test").count(),
            1,
            "cap of 1 renders exactly one finding:\n{out}"
        );

        // Stats count the full three-violation set regardless of the cap.
        assert!(
            out.contains("3 violations (0 error, 3 warning, 0 info)"),
            "stats must show the full count:\n{out}"
        );
        assert!(out.contains("viewport_count: 2"), "stats:\n{out}");
        assert!(out.contains("rule_count: 2"), "stats:\n{out}");
    }

    #[test]
    fn pretty_capped_run_id_matches_json_run_id() {
        // The pretty `run_id` is the SHA-256 of the canonically sorted full
        // set — the same input JSON hashes — so capping the rendered body
        // never shifts it and the two formats agree.
        let v = capped_fixture();
        let pretty_out = pretty_capped(&v, Some(1), 0);
        let json_out = json(&v).expect("json serialize");
        let envelope: serde_json::Value =
            serde_json::from_str(&json_out).expect("json envelope parses");
        let json_run_id = envelope["run_id"].as_str().expect("run_id present");

        assert!(
            pretty_out.contains(&format!("run_id: {json_run_id}")),
            "pretty run_id must equal json run_id ({json_run_id}):\n{pretty_out}"
        );
    }

    #[test]
    fn pretty_capped_zero_renders_no_findings_but_keeps_stats() {
        let v = capped_fixture();
        let out = pretty_capped(&v, Some(0), 0);
        assert_eq!(
            out.matches("      warning: test").count(),
            0,
            "cap of 0 renders no findings:\n{out}"
        );
        assert!(
            out.contains("3 violations (0 error, 3 warning, 0 info)"),
            "stats survive a zero cap:\n{out}"
        );
        assert!(
            !out.contains("No violations."),
            "a capped-out body is not the empty-set 'No violations.' message:\n{out}"
        );
    }

    #[test]
    fn pretty_capped_with_suggested_ignores_counts_full_set() {
        // The suggested-ignores block reflects the full set (three
        // violations across two rules), not the single rendered finding.
        let v = capped_fixture();
        let out = pretty_capped_with_suggested_ignores(&v, Some(1), 0);
        assert!(
            out.contains("Suggested .plumbignore (would suppress 3 violations):"),
            "suggested-ignores count must reflect the full set:\n{out}"
        );
        assert!(out.contains("a/one sel1"), "block lists full set:\n{out}");
        assert!(out.contains("a/one sel3"), "block lists full set:\n{out}");
        assert!(out.contains("b/two sel2"), "block lists full set:\n{out}");
    }

    #[test]
    fn pretty_with_ignored_appends_singular_footer() {
        let v = vec![violation("spacing/grid-conformance", "body", "desktop", 1)];
        let out = pretty_with_ignored(&v, 1);
        assert!(
            out.contains("1 violation suppressed by config"),
            "footer: {out}"
        );
    }

    #[test]
    fn pretty_with_ignored_appends_plural_footer() {
        let v = vec![violation("spacing/grid-conformance", "body", "desktop", 1)];
        let out = pretty_with_ignored(&v, 7);
        assert!(
            out.contains("7 violations suppressed by config"),
            "footer: {out}"
        );
    }

    #[test]
    fn json_with_ignored_zero_matches_json() {
        let v = vec![violation("spacing/grid-conformance", "body", "desktop", 1)];
        let a = json(&v).expect("serialize");
        let b = json_with_ignored(&v, 0).expect("serialize with zero");
        assert_eq!(a, b);
    }

    #[test]
    fn json_with_ignored_emits_count() {
        let v = vec![violation("spacing/grid-conformance", "body", "desktop", 1)];
        let raw = json_with_ignored(&v, 5).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("envelope");
        assert_eq!(
            parsed.get("ignored").and_then(serde_json::Value::as_u64),
            Some(5)
        );
    }

    #[test]
    fn pretty_with_suggested_ignores_and_ignored_includes_both_blocks() {
        let v = vec![violation("spacing/grid-conformance", "body", "desktop", 1)];
        let out = pretty_with_suggested_ignores_and_ignored(&v, 3);
        assert!(out.contains("3 violations suppressed by config"));
        assert!(out.contains("Suggested .plumbignore"));
    }

    #[test]
    fn json_with_suggested_ignores_and_ignored_includes_both_keys() {
        let v = vec![violation("spacing/grid-conformance", "body", "desktop", 1)];
        let raw = json_with_suggested_ignores_and_ignored(&v, 3).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("envelope");
        assert_eq!(
            parsed.get("ignored").and_then(serde_json::Value::as_u64),
            Some(3)
        );
        assert!(parsed.get("suggested_ignores").is_some());
    }
}
