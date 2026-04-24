//! Golden snapshot of the walking-skeleton engine run.
//!
//! Proves `engine::run` produces deterministic, sorted output given the
//! canned snapshot + default config.

use plumb_core::{Config, PlumbSnapshot, run};

#[test]
fn hello_world_golden() {
    let snapshot = PlumbSnapshot::canned();
    let config = Config::default();
    let violations = run(&snapshot, &config);

    let json = serde_json::to_string_pretty(&violations).expect("serialize");
    insta::assert_snapshot!("hello_world", json);
}

#[test]
fn engine_run_is_deterministic() {
    let snapshot = PlumbSnapshot::canned();
    let config = Config::default();
    let a = run(&snapshot, &config);
    let b = run(&snapshot, &config);
    let c = run(&snapshot, &config);
    assert_eq!(a, b);
    assert_eq!(b, c);
}
