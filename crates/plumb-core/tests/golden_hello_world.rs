//! Golden snapshot of the walking-skeleton engine run.
//!
//! Proves `engine::run` produces deterministic, sorted output given the
//! canned snapshot + default config.

use plumb_core::{Config, PlumbSnapshot, run};

#[test]
fn hello_world_golden() -> Result<(), serde_json::Error> {
    let snapshot = PlumbSnapshot::canned();
    let config = Config::default();
    let violations = run(&snapshot, &config);

    let json = serde_json::to_string_pretty(&violations)?;
    insta::assert_snapshot!("hello_world", json);
    Ok(())
}

#[test]
fn engine_run_is_deterministic() -> Result<(), serde_json::Error> {
    let snapshot = PlumbSnapshot::canned();
    let config = Config::default();
    let a = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let b = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    let c = serde_json::to_string_pretty(&run(&snapshot, &config))?;
    assert_eq!(a, b);
    assert_eq!(b, c);
    Ok(())
}
