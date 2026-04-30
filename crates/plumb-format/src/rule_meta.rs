//! SARIF rule metadata builder.
//!
//! Builds the `tool.driver.rules` array and a rule-index lookup table
//! from [`plumb_core::register_builtin`]. The array is sorted by rule id
//! for deterministic output.

use plumb_core::{Severity, register_builtin};
use serde_json::{Value, json};

/// Map a [`Severity`] to the SARIF `defaultConfiguration.level` string.
fn severity_to_sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

/// Build the canonical help URL for a rule id.
fn help_uri(rule_id: &str) -> String {
    let slug = rule_id.replace('/', "-");
    format!("https://plumb.aramhammoudeh.com/rules/{slug}")
}

/// Build the SARIF `tool.driver.rules` array.
///
/// Each entry is a SARIF `reportingDescriptor` with `id`, `name`,
/// `shortDescription`, `fullDescription`, `helpUri`, and
/// `defaultConfiguration`. The array is sorted by rule id.
pub(crate) fn driver_rules() -> Vec<Value> {
    let mut rules = register_builtin();
    rules.sort_by(|a, b| a.id().cmp(b.id()));

    rules
        .iter()
        .map(|rule| {
            let rule_id = rule.id();
            let summary = rule.summary();

            json!({
                "id": rule_id,
                "name": rule_id,
                "shortDescription": { "text": summary },
                "fullDescription": { "text": summary },
                "helpUri": help_uri(rule_id),
                "defaultConfiguration": {
                    "level": severity_to_sarif_level(rule.default_severity()),
                },
            })
        })
        .collect()
}

/// Build a mapping from rule id to its index in the [`driver_rules`]
/// array.
///
/// Returns `(rule_id, index)` pairs sorted by rule id — the same order
/// as [`driver_rules`].
pub(crate) fn rule_index_map() -> Vec<(&'static str, usize)> {
    let mut rules = register_builtin();
    rules.sort_by(|a, b| a.id().cmp(b.id()));
    rules
        .iter()
        .enumerate()
        .map(|(i, rule)| (rule.id(), i))
        .collect()
}
