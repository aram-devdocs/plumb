//! Determinism guarantees for `plumb-format`.
//!
//! Each formatter is a pure function of its inputs; running it three
//! times on the same input must produce byte-identical output. The
//! suite mirrors the `just determinism-check` recipe at the formatter
//! level — i.e. before the CLI ever wraps it.

use plumb_core::{Config, PlumbSnapshot, Severity, builtin_rule_metadata, register_builtin, run};
use plumb_format::{json, mcp_compact, pretty, sarif_with_rules};

fn fixture() -> Vec<plumb_core::Violation> {
    let snapshot = PlumbSnapshot::canned();
    let config = Config::default();
    run(&snapshot, &config)
}

/// Mirror of the SARIF severity mapping owned by the formatter.
///
/// The test asserts the rule registry's default severities line up
/// with what the formatter emits, so the mapping is duplicated here
/// rather than reaching into a private module.
fn severity_to_sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

#[test]
fn json_is_byte_identical_across_runs() {
    let violations = fixture();
    let a = json(&violations).expect("json serialize a");
    let b = json(&violations).expect("json serialize b");
    let c = json(&violations).expect("json serialize c");
    assert_eq!(a, b);
    assert_eq!(b, c);
}

#[test]
fn json_envelope_has_required_fields() {
    let violations = fixture();
    let out = json(&violations).expect("json serialize");
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("parse json");

    let plumb_version = parsed
        .get("plumb_version")
        .and_then(serde_json::Value::as_str)
        .expect("plumb_version present");
    assert!(
        !plumb_version.is_empty(),
        "plumb_version must be a non-empty string"
    );

    let run_id = parsed
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .expect("run_id present");
    assert!(
        run_id.starts_with("sha256:"),
        "run_id must be prefixed with sha256:, got {run_id}"
    );
    let hex = run_id.trim_start_matches("sha256:");
    assert_eq!(hex.len(), 64, "sha256 hex digest is 64 chars");
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));

    let summary = parsed.get("summary").expect("summary present");
    for key in ["error", "warning", "info", "total"] {
        assert!(summary.get(key).is_some(), "summary.{key} must be present");
    }

    let violations_value = parsed
        .get("violations")
        .and_then(serde_json::Value::as_array)
        .expect("violations array present");
    assert_eq!(violations_value.len(), violations.len());
}

#[test]
fn json_run_id_changes_when_violations_change() {
    let v1 = fixture();
    let mut v2 = v1.clone();
    if let Some(first) = v2.first_mut() {
        first.message.push_str(" (mutated)");
    }
    let a = json(&v1).expect("json serialize v1");
    let b = json(&v2).expect("json serialize v2");
    let pa: serde_json::Value = serde_json::from_str(&a).expect("parse a");
    let pb: serde_json::Value = serde_json::from_str(&b).expect("parse b");
    assert_ne!(
        pa["run_id"], pb["run_id"],
        "run_id must change when violations change"
    );
}

#[test]
fn json_run_id_is_stable_under_input_reordering() {
    // The formatter re-sorts defensively before hashing, so a caller
    // that hands violations in a different order still produces the
    // same `run_id`. This is the determinism contract.
    let violations = fixture();
    if violations.len() < 2 {
        return; // canned fixture too small to reorder
    }
    let mut reversed = violations.clone();
    reversed.reverse();
    let a = json(&violations).expect("sorted");
    let b = json(&reversed).expect("reversed");
    let pa: serde_json::Value = serde_json::from_str(&a).expect("parse a");
    let pb: serde_json::Value = serde_json::from_str(&b).expect("parse b");
    assert_eq!(pa["run_id"], pb["run_id"]);
}

#[test]
fn pretty_is_byte_identical_across_runs() {
    let violations = fixture();
    let a = pretty(&violations);
    let b = pretty(&violations);
    let c = pretty(&violations);
    assert_eq!(a, b);
    assert_eq!(b, c);
}

#[test]
fn sarif_is_byte_identical_across_runs() {
    let violations = fixture();
    let a = sarif_with_rules(&violations, &builtin_rule_metadata()).expect("sarif a");
    let b = sarif_with_rules(&violations, &builtin_rule_metadata()).expect("sarif b");
    let c = sarif_with_rules(&violations, &builtin_rule_metadata()).expect("sarif c");
    assert_eq!(a, b);
    assert_eq!(b, c);
}

#[test]
fn sarif_has_rule_metadata() {
    let violations = fixture();
    let out = sarif_with_rules(&violations, &builtin_rule_metadata()).expect("sarif serialize");
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("parse sarif");

    let rules = parsed["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("rules array present");

    assert!(!rules.is_empty(), "rules array must not be empty");

    for rule in rules {
        assert!(
            rule.get("id").and_then(serde_json::Value::as_str).is_some(),
            "each rule must have an id"
        );
        assert!(
            rule.get("shortDescription")
                .and_then(|sd| sd.get("text"))
                .and_then(serde_json::Value::as_str)
                .is_some(),
            "each rule must have shortDescription.text"
        );
        assert!(
            rule.get("fullDescription")
                .and_then(|fd| fd.get("text"))
                .and_then(serde_json::Value::as_str)
                .is_some(),
            "each rule must have fullDescription.text"
        );
        assert!(
            rule.get("helpUri")
                .and_then(serde_json::Value::as_str)
                .is_some(),
            "each rule must have helpUri"
        );
        assert!(
            rule.get("defaultConfiguration")
                .and_then(|dc| dc.get("level"))
                .and_then(serde_json::Value::as_str)
                .is_some(),
            "each rule must have defaultConfiguration.level"
        );
    }
}

#[test]
fn sarif_results_reference_rules() {
    let violations = fixture();
    let out = sarif_with_rules(&violations, &builtin_rule_metadata()).expect("sarif serialize");
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("parse sarif");

    let rules = parsed["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("rules array present");
    let results = parsed["runs"][0]["results"]
        .as_array()
        .expect("results array present");

    for result in results {
        let rule_id = result
            .get("ruleId")
            .and_then(serde_json::Value::as_str)
            .expect("result must have ruleId");
        let rule_index = result
            .get("ruleIndex")
            .and_then(serde_json::Value::as_u64)
            .expect("result must have ruleIndex");
        let rule_index: usize = usize::try_from(rule_index).expect("ruleIndex fits usize");

        assert!(
            rule_index < rules.len(),
            "ruleIndex {rule_index} out of bounds (rules len {})",
            rules.len()
        );
        let indexed_id = rules[rule_index]
            .get("id")
            .and_then(serde_json::Value::as_str)
            .expect("indexed rule must have id");
        assert_eq!(
            rule_id, indexed_id,
            "ruleId must match the rule at ruleIndex"
        );
    }
}

#[test]
fn sarif_rules_sorted_by_id() {
    let violations = fixture();
    let out = sarif_with_rules(&violations, &builtin_rule_metadata()).expect("sarif serialize");
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("parse sarif");

    let rules = parsed["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("rules array present");

    let ids: Vec<&str> = rules
        .iter()
        .filter_map(|r| r.get("id").and_then(serde_json::Value::as_str))
        .collect();

    let mut sorted_ids = ids.clone();
    sorted_ids.sort_unstable();
    assert_eq!(ids, sorted_ids, "rules must be sorted by id");
}

#[test]
fn sarif_default_severity_matches_rule_registry() {
    let violations = fixture();
    let out = sarif_with_rules(&violations, &builtin_rule_metadata()).expect("sarif serialize");
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("parse sarif");

    let rules = parsed["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("rules array present");

    // Build the registry-side mapping of rule_id to default severity.
    let registry: std::collections::HashMap<String, Severity> = register_builtin()
        .iter()
        .map(|r| (r.id().to_owned(), r.default_severity()))
        .collect();

    for rule in rules {
        let id = rule
            .get("id")
            .and_then(serde_json::Value::as_str)
            .expect("each rule must have an id");
        let level = rule
            .get("defaultConfiguration")
            .and_then(|dc| dc.get("level"))
            .and_then(serde_json::Value::as_str)
            .expect("each rule must have defaultConfiguration.level");
        assert!(
            matches!(level, "error" | "warning" | "note"),
            "level must be one of error|warning|note, got {level}"
        );

        let registry_severity = registry
            .get(id)
            .copied()
            .unwrap_or_else(|| panic!("rule {id} is not in register_builtin()"));
        assert_eq!(
            level,
            severity_to_sarif_level(registry_severity),
            "rule {id}: SARIF level must match register_builtin() default severity"
        );
    }
}

#[test]
fn sarif_rule_help_uri_uses_canonical_slug() {
    let violations = fixture();
    let out = sarif_with_rules(&violations, &builtin_rule_metadata()).expect("sarif serialize");
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("parse sarif");

    let rules = parsed["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("rules array present");

    for rule in rules {
        let id = rule
            .get("id")
            .and_then(serde_json::Value::as_str)
            .expect("each rule must have an id");
        let help_uri = rule
            .get("helpUri")
            .and_then(serde_json::Value::as_str)
            .expect("each rule must have helpUri");
        let slug = id.replace('/', "-");
        let expected = format!("https://plumb.aramhammoudeh.com/rules/{slug}");
        assert_eq!(
            help_uri, expected,
            "rule {id} helpUri must use canonical slug ({expected})"
        );
    }
}

#[test]
fn sarif_results_have_physical_location() {
    // GitHub Code Scanning's `locationFromSarifResult` rejects results
    // that don't carry a `physicalLocation`. Plumb's violations are
    // tied to rendered URLs, not source files, so the formatter emits
    // a stable synthetic placeholder. Lock the shape in.
    let violations = fixture();
    let out = sarif_with_rules(&violations, &builtin_rule_metadata()).expect("sarif serialize");
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("parse sarif");

    let results = parsed["runs"][0]["results"]
        .as_array()
        .expect("results array present");
    assert!(
        !results.is_empty(),
        "fixture must produce at least one result"
    );

    for (i, result) in results.iter().enumerate() {
        let physical = result["locations"][0]
            .get("physicalLocation")
            .unwrap_or_else(|| panic!("result {i} missing locations[0].physicalLocation"));

        let uri = physical
            .get("artifactLocation")
            .and_then(|al| al.get("uri"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("result {i} missing physicalLocation.artifactLocation.uri"));
        assert!(
            !uri.is_empty(),
            "result {i} physicalLocation.artifactLocation.uri must be non-empty"
        );

        assert!(
            physical
                .get("region")
                .and_then(|r| r.get("startLine"))
                .is_some(),
            "result {i} missing physicalLocation.region.startLine"
        );
    }
}

#[test]
fn sarif_document_passes_basic_schema_shape() {
    let violations = fixture();
    let out = sarif_with_rules(&violations, &builtin_rule_metadata()).expect("sarif serialize");
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("parse sarif");

    assert_eq!(
        parsed
            .get("version")
            .and_then(serde_json::Value::as_str)
            .expect("version present"),
        "2.1.0",
        "version must be 2.1.0"
    );
    assert!(
        parsed
            .get("$schema")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|s| !s.is_empty()),
        "$schema must be a non-empty string"
    );

    let runs = parsed
        .get("runs")
        .and_then(serde_json::Value::as_array)
        .expect("runs array present");
    assert!(!runs.is_empty(), "runs must be a non-empty array");

    let driver = runs[0]
        .get("tool")
        .and_then(|t| t.get("driver"))
        .expect("runs[0].tool.driver present");
    assert_eq!(
        driver
            .get("name")
            .and_then(serde_json::Value::as_str)
            .expect("driver.name present"),
        "plumb",
        "driver name must be plumb"
    );
    let rules = driver
        .get("rules")
        .and_then(serde_json::Value::as_array)
        .expect("driver.rules array present");
    assert!(!rules.is_empty(), "rules array must not be empty");

    let results = runs[0]
        .get("results")
        .and_then(serde_json::Value::as_array)
        .expect("runs[0].results array present");

    for result in results {
        assert!(
            result
                .get("ruleId")
                .and_then(serde_json::Value::as_str)
                .is_some(),
            "each result must have ruleId"
        );
        assert!(
            result
                .get("level")
                .and_then(serde_json::Value::as_str)
                .is_some(),
            "each result must have level"
        );
        assert!(
            result
                .get("message")
                .and_then(|m| m.get("text"))
                .and_then(serde_json::Value::as_str)
                .is_some(),
            "each result must have message.text"
        );
        let locations = result
            .get("locations")
            .and_then(serde_json::Value::as_array)
            .expect("each result must have a locations array");
        assert!(
            !locations.is_empty(),
            "each result must have at least one location"
        );
        assert!(
            locations[0].get("physicalLocation").is_some(),
            "each result must have locations[0].physicalLocation"
        );
    }
}

#[test]
fn mcp_compact_is_byte_identical_across_runs() {
    let violations = fixture();
    let (ta, sa) = mcp_compact(&violations);
    let (tb, sb) = mcp_compact(&violations);
    assert_eq!(ta, tb);
    assert_eq!(sa, sb);
}
