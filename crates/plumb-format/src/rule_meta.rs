//! SARIF rule metadata builder.
//!
//! Builds the `tool.driver.rules` array and a rule-index lookup table
//! from [`plumb_core::register_builtin`] and compile-time-embedded
//! documentation markdown. The arrays are sorted by rule id for
//! deterministic output.
//!
//! Each `reportingDescriptor` carries:
//!
//! - `id` and `name` — both equal to [`plumb_core::Rule::id`].
//! - `shortDescription.text` — [`plumb_core::Rule::summary`] (one-liner).
//! - `fullDescription.text` — the body of the `## What it checks`
//!   section pulled from the rule's markdown doc, falling back to the
//!   summary when the section is missing or empty.
//! - `helpUri` — `https://plumb.aramhammoudeh.com/rules/<slug>`, where
//!   `<slug>` is the rule id with `/` replaced by `-`.
//! - `help.text` and `help.markdown` — the full markdown body, so
//!   GitHub Code Scanning's "Help" panel renders the rule page.
//! - `defaultConfiguration.level` — SARIF severity derived from
//!   [`plumb_core::Rule::default_severity`].

// Items are `pub(crate)` because this module is private but needs to be
// visible to `lib.rs`.
#![allow(clippy::redundant_pub_crate)]

use plumb_core::{Severity, register_builtin};
use serde_json::{Value, json};

/// Compile-time rule documentation entry.
struct RuleDoc {
    /// Stable rule id — matches [`plumb_core::Rule::id`].
    rule_id: &'static str,
    /// Markdown body from `docs/src/rules/<slug>.md`.
    markdown: &'static str,
}

/// Table of every built-in rule's documentation, sorted by `rule_id`.
const RULE_DOCS: &[RuleDoc] = &[
    RuleDoc {
        rule_id: "a11y/touch-target",
        markdown: include_str!("../../../docs/src/rules/a11y-touch-target.md"),
    },
    RuleDoc {
        rule_id: "color/palette-conformance",
        markdown: include_str!("../../../docs/src/rules/color-palette-conformance.md"),
    },
    RuleDoc {
        rule_id: "edge/near-alignment",
        markdown: include_str!("../../../docs/src/rules/edge-near-alignment.md"),
    },
    RuleDoc {
        rule_id: "radius/scale-conformance",
        markdown: include_str!("../../../docs/src/rules/radius-scale-conformance.md"),
    },
    RuleDoc {
        rule_id: "sibling/height-consistency",
        markdown: include_str!("../../../docs/src/rules/sibling-height-consistency.md"),
    },
    RuleDoc {
        rule_id: "spacing/grid-conformance",
        markdown: include_str!("../../../docs/src/rules/spacing-grid-conformance.md"),
    },
    RuleDoc {
        rule_id: "spacing/scale-conformance",
        markdown: include_str!("../../../docs/src/rules/spacing-scale-conformance.md"),
    },
    RuleDoc {
        rule_id: "type/scale-conformance",
        markdown: include_str!("../../../docs/src/rules/type-scale-conformance.md"),
    },
];

/// Look up the markdown body for a rule id.
fn lookup_markdown(rule_id: &str) -> Option<&'static str> {
    RULE_DOCS
        .iter()
        .find(|entry| entry.rule_id == rule_id)
        .map(|entry| entry.markdown)
}

/// Header line that opens the section we care about. The check is
/// strict: a leading `## ` followed by the exact heading text.
const WHAT_IT_CHECKS_HEADING: &str = "## What it checks";

/// Extract the body of the `## What it checks` section from a markdown
/// document, trimmed of surrounding whitespace.
///
/// Returns `None` when the heading is absent or the body before the
/// next `## …` (or end of file) is empty after trimming.
fn extract_what_it_checks(markdown: &str) -> Option<String> {
    // Find the heading line. We match `## What it checks` at the start
    // of a line followed by an end-of-line, allowing optional trailing
    // whitespace. The split-once-on-newline approach below handles the
    // line boundaries explicitly without pulling in a regex dep.
    let mut rest: &str = markdown;
    loop {
        let idx = rest.find(WHAT_IT_CHECKS_HEADING)?;
        let after_heading_byte = idx + WHAT_IT_CHECKS_HEADING.len();
        // The heading must start at byte 0 or right after a newline.
        let line_start_ok = idx == 0 || rest.as_bytes().get(idx - 1) == Some(&b'\n');
        // The heading must be followed by end-of-line or end-of-file —
        // no trailing characters that would make it a different
        // heading like `## What it checks for`.
        let after = rest.get(after_heading_byte..).unwrap_or("");
        let line_end_ok = after.is_empty() || after.starts_with('\n') || after.starts_with('\r');
        if line_start_ok && line_end_ok {
            // Slice from immediately after the heading.
            let body_start = after_heading_byte;
            let after_body = rest.get(body_start..).unwrap_or("");
            // The body runs until the next `## ` heading at the start
            // of a line, or end of file.
            let body_raw = next_section_slice(after_body);
            let trimmed = body_raw.trim();
            return if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            };
        }
        // Not a match — advance past this occurrence and keep scanning.
        rest = rest.get(after_heading_byte..)?;
    }
}

/// Return the slice of `body` up to (but not including) the next `##`
/// heading at the start of a line. If no further heading exists, the
/// whole slice is returned.
fn next_section_slice(body: &str) -> &str {
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // We're looking for a newline followed by `## ` (level-2
        // heading). A `### ` is a sub-heading and stays inside the
        // section.
        if bytes[i] == b'\n' {
            let after = i + 1;
            if bytes.get(after..after + 3) == Some(b"## ")
                || bytes.get(after..after + 3) == Some(b"##\n")
                || bytes.get(after..after + 3) == Some(b"##\r")
            {
                // Confirm this is exactly two hashes — not three.
                if bytes.get(after + 2).copied() != Some(b'#') {
                    return body.get(..after).unwrap_or(body);
                }
            }
        }
        i += 1;
    }
    body
}

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
/// `shortDescription`, `fullDescription`, `helpUri`, `help`, and
/// `defaultConfiguration`. The array is sorted by rule id.
pub(crate) fn driver_rules() -> Vec<Value> {
    let mut rules = register_builtin();
    rules.sort_by(|a, b| a.id().cmp(b.id()));

    rules
        .iter()
        .map(|rule| {
            let rule_id = rule.id();
            let summary = rule.summary();
            let markdown = lookup_markdown(rule_id);

            // fullDescription is the section body, falling back to the
            // summary when the section is missing or empty. It is
            // never the entire markdown file.
            let full_description = markdown
                .and_then(extract_what_it_checks)
                .unwrap_or_else(|| summary.to_owned());

            let mut descriptor = serde_json::Map::new();
            descriptor.insert("id".to_owned(), Value::String(rule_id.to_owned()));
            descriptor.insert("name".to_owned(), Value::String(rule_id.to_owned()));
            descriptor.insert("shortDescription".to_owned(), json!({ "text": summary }));
            descriptor.insert(
                "fullDescription".to_owned(),
                json!({ "text": full_description }),
            );
            descriptor.insert("helpUri".to_owned(), Value::String(help_uri(rule_id)));
            if let Some(md) = markdown {
                descriptor.insert(
                    "help".to_owned(),
                    json!({
                        "text": md,
                        "markdown": md,
                    }),
                );
            }
            descriptor.insert(
                "defaultConfiguration".to_owned(),
                json!({ "level": severity_to_sarif_level(rule.default_severity()) }),
            );

            Value::Object(descriptor)
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

#[cfg(test)]
mod tests {
    use super::{
        WHAT_IT_CHECKS_HEADING, extract_what_it_checks, lookup_markdown, severity_to_sarif_level,
    };
    use plumb_core::{Severity, register_builtin};

    #[test]
    fn extracts_what_it_checks_section() {
        let md = "# rule\n\n**Status:** active\n\n## What it checks\n\nFirst paragraph.\n\nSecond paragraph.\n\n## Why it matters\n\nIgnored.\n";
        let body = extract_what_it_checks(md).expect("section present");
        assert_eq!(body, "First paragraph.\n\nSecond paragraph.");
    }

    #[test]
    fn returns_none_when_section_missing() {
        let md = "# rule\n\n**Status:** active\n\n## Why it matters\n\nNo what-it-checks here.\n";
        assert!(extract_what_it_checks(md).is_none());
    }

    #[test]
    fn returns_none_when_section_empty() {
        // Header is present but the body trims to empty before the next `##`.
        let md = "# rule\n\n## What it checks\n\n   \n\n## Why it matters\n\nIgnored.\n";
        assert!(extract_what_it_checks(md).is_none());
    }

    #[test]
    fn returns_none_when_section_empty_at_eof() {
        let md = "# rule\n\n## What it checks\n\n";
        assert!(extract_what_it_checks(md).is_none());
    }

    #[test]
    fn extracts_section_when_at_end_of_file() {
        let md = "# rule\n\n## What it checks\n\nThe last section's body.\n";
        let body = extract_what_it_checks(md).expect("section present");
        assert_eq!(body, "The last section's body.");
    }

    #[test]
    fn ignores_subheadings_inside_section() {
        // A `### Worked example` should stay inside the section; only a
        // following level-2 heading closes it.
        let md = "## What it checks\n\nIntro.\n\n### Worked example\n\nDetails.\n\n## Why it matters\n\nIgnored.\n";
        let body = extract_what_it_checks(md).expect("section present");
        assert!(body.starts_with("Intro."));
        assert!(body.contains("### Worked example"));
        assert!(body.contains("Details."));
        assert!(!body.contains("Why it matters"));
    }

    #[test]
    fn does_not_match_a_heading_with_a_suffix() {
        // `## What it checks for` is a different heading and should not
        // count as the section we're looking for.
        let md = "## What it checks for tests\n\nNot the right one.\n";
        assert!(extract_what_it_checks(md).is_none());
    }

    #[test]
    fn every_builtin_rule_has_doc_entry() {
        for rule in register_builtin() {
            let id = rule.id();
            assert!(
                lookup_markdown(id).is_some(),
                "rule {id} is missing a markdown doc entry in RULE_DOCS"
            );
        }
    }

    #[test]
    fn every_builtin_rule_has_extractable_what_it_checks() {
        for rule in register_builtin() {
            let id = rule.id();
            let md = lookup_markdown(id).unwrap_or("");
            let extracted = extract_what_it_checks(md);
            assert!(
                extracted.is_some(),
                "rule {id} doc must have a non-empty `## What it checks` section"
            );
        }
    }

    #[test]
    fn severity_mapping_matches_sarif_levels() {
        assert_eq!(severity_to_sarif_level(Severity::Error), "error");
        assert_eq!(severity_to_sarif_level(Severity::Warning), "warning");
        assert_eq!(severity_to_sarif_level(Severity::Info), "note");
    }

    #[test]
    fn what_it_checks_heading_constant_is_canonical() {
        // Sanity check the literal — protects against accidental edits.
        assert_eq!(WHAT_IT_CHECKS_HEADING, "## What it checks");
    }
}
