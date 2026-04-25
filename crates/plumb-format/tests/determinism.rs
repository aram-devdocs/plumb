//! Determinism guarantees for `plumb-format`.
//!
//! Each formatter is a pure function of its inputs; running it three
//! times on the same input must produce byte-identical output. The
//! suite mirrors the `just determinism-check` recipe at the formatter
//! level — i.e. before the CLI ever wraps it.

use plumb_core::{Config, PlumbSnapshot, run};
use plumb_format::{json, mcp_compact, pretty, sarif};

fn fixture() -> Vec<plumb_core::Violation> {
    let snapshot = PlumbSnapshot::canned();
    let config = Config::default();
    run(&snapshot, &config)
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
    let a = sarif(&violations).expect("sarif a");
    let b = sarif(&violations).expect("sarif b");
    let c = sarif(&violations).expect("sarif c");
    assert_eq!(a, b);
    assert_eq!(b, c);
}

#[test]
fn mcp_compact_is_byte_identical_across_runs() {
    let violations = fixture();
    let (ta, sa) = mcp_compact(&violations);
    let (tb, sb) = mcp_compact(&violations);
    assert_eq!(ta, tb);
    assert_eq!(sa, sb);
}
