//! `expected.json` schema and loader.
//!
//! Every fixture under `e2e-sites/<name>/expected.json` declares the
//! intentional violations it introduces. The harness asserts:
//!
//! - The set of `rule_id`s that match `target_rules` has counts
//!   identical to `by_rule_id`.
//! - The total target-rule violation count equals
//!   `total_target_violations`.
//!
//! Non-target rule violations are tolerated. This narrows the
//! assertion surface to the design-system invariants the matrix is
//! built to validate, while staying robust to incidental Chromium-side
//! rendering differences.

use std::path::Path;

use anyhow::Context as _;
use indexmap::IndexMap;
use serde::Deserialize;

/// Parsed `expected.json` payload.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    /// Optional comment field — round-tripped, never asserted on. Lets
    /// fixture authors annotate the JSON without a separate sidecar.
    #[serde(rename = "$comment", default)]
    pub comment: Option<String>,
    /// The rule ids the fixture intentionally exercises.
    pub target_rules: Vec<String>,
    /// Expected violation count per target rule. Keys MUST be a subset
    /// of `target_rules`.
    pub by_rule_id: IndexMap<String, usize>,
    /// Sum of all target-rule violations. MUST equal the sum of
    /// `by_rule_id` values.
    pub total_target_violations: usize,
}

impl Expected {
    /// Load and validate from a path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read, fails to parse,
    /// or the internal invariants (`by_rule_id` keys subset of
    /// `target_rules`, sum equals `total_target_violations`) do not
    /// hold.
    pub fn load(path: &Path) -> Result<Self, anyhow::Error> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("read expected.json at {}", path.display()))?;
        let parsed: Self = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse expected.json at {}", path.display()))?;
        parsed.validate()?;
        Ok(parsed)
    }

    /// Sanity-check the parsed payload. The harness calls this after
    /// `serde_json::from_*`.
    ///
    /// # Errors
    ///
    /// Returns an error if `by_rule_id` references a rule id not in
    /// `target_rules`, or if the sum of `by_rule_id` values does not
    /// equal `total_target_violations`.
    pub fn validate(&self) -> Result<(), anyhow::Error> {
        for key in self.by_rule_id.keys() {
            if !self.target_rules.iter().any(|r| r == key) {
                return Err(anyhow::anyhow!(
                    "by_rule_id contains rule `{key}` not present in target_rules"
                ));
            }
        }
        let sum: usize = self.by_rule_id.values().sum();
        if sum != self.total_target_violations {
            return Err(anyhow::anyhow!(
                "by_rule_id sum {sum} disagrees with total_target_violations {total}",
                total = self.total_target_violations,
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Expected;

    fn parse(payload: &str) -> Result<Expected, anyhow::Error> {
        let parsed: Expected = serde_json::from_str(payload)?;
        parsed.validate()?;
        Ok(parsed)
    }

    #[test]
    fn happy_path_validates() {
        let payload = r#"{
            "target_rules": ["a/b", "c/d"],
            "by_rule_id": { "a/b": 2, "c/d": 3 },
            "total_target_violations": 5
        }"#;
        let parsed = parse(payload).expect("happy path");
        assert_eq!(parsed.total_target_violations, 5);
    }

    #[test]
    fn rejects_orphan_rule_in_by_rule_id() {
        let payload = r#"{
            "target_rules": ["a/b"],
            "by_rule_id": { "a/b": 1, "x/y": 1 },
            "total_target_violations": 2
        }"#;
        let err = parse(payload).expect_err("orphan rule must error");
        assert!(err.to_string().contains("not present in target_rules"));
    }

    #[test]
    fn rejects_sum_mismatch() {
        let payload = r#"{
            "target_rules": ["a/b"],
            "by_rule_id": { "a/b": 2 },
            "total_target_violations": 99
        }"#;
        let err = parse(payload).expect_err("sum mismatch must error");
        assert!(err.to_string().contains("disagrees"));
    }

    #[test]
    fn comment_field_is_accepted() {
        let payload = r#"{
            "$comment": "free-form text",
            "target_rules": [],
            "by_rule_id": {},
            "total_target_violations": 0
        }"#;
        let parsed = parse(payload).expect("comment ok");
        assert_eq!(parsed.comment.as_deref(), Some("free-form text"));
    }
}
