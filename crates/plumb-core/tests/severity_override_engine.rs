//! Per-rule severity overrides applied at the engine layer.
//!
//! `[rules."<id>"] severity = "..."` in `plumb.toml` MUST remap every
//! emitted violation for that rule id, before the engine returns. This
//! is a contract the CLI's `exit_code_for` and the formatter layer both
//! depend on — neither has its own remap path. See the audit blocker
//! "B2 — severity override silent no-op" and finding H32 for context.
//!
//! Test surface:
//!
//! 1. `warning → error` (the quickstart customisation users are told to
//!    perform first) actually flips severity.
//! 2. `warning → info` demotes severity (validates the remap is
//!    bidirectional, not just an upgrade path).
//! 3. No override → the rule's `default_severity()` survives untouched.
//! 4. Override on an unknown rule id → ignored cleanly; no panic, no
//!    spurious emission, default severities preserved on real rules.

use plumb_core::config::RuleOverride;
use plumb_core::{Config, PlumbSnapshot, Severity, run};

/// The `canned()` snapshot fires exactly one
/// `spacing/grid-conformance` violation against the default config
/// (`padding-top: 13px` is off the base-4 grid). Default severity for
/// that rule is `warning`.
const TARGET_RULE: &str = "spacing/grid-conformance";

fn config_with_override(rule_id: &str, severity: Severity) -> Config {
    let mut config = Config::default();
    config.rules.insert(
        rule_id.to_owned(),
        RuleOverride {
            enabled: true,
            severity: Some(severity),
        },
    );
    config
}

#[test]
fn severity_override_promotes_warning_to_error() {
    let snapshot = PlumbSnapshot::canned();
    let config = config_with_override(TARGET_RULE, Severity::Error);

    let violations = run(&snapshot, &config);
    let target: Vec<_> = violations
        .iter()
        .filter(|v| v.rule_id == TARGET_RULE)
        .collect();

    assert!(
        !target.is_empty(),
        "canned snapshot must fire {TARGET_RULE} so the override has something to remap",
    );
    for violation in target {
        assert_eq!(
            violation.severity,
            Severity::Error,
            "override must remap severity at the engine layer; got {:?} for {}",
            violation.severity,
            violation.selector,
        );
    }
}

#[test]
fn severity_override_demotes_warning_to_info() {
    let snapshot = PlumbSnapshot::canned();
    let config = config_with_override(TARGET_RULE, Severity::Info);

    let violations = run(&snapshot, &config);
    let target: Vec<_> = violations
        .iter()
        .filter(|v| v.rule_id == TARGET_RULE)
        .collect();

    assert!(!target.is_empty());
    for violation in target {
        assert_eq!(
            violation.severity,
            Severity::Info,
            "override must remap severity downward as well as upward",
        );
    }
}

#[test]
fn no_override_preserves_default_severity() {
    let snapshot = PlumbSnapshot::canned();
    let config = Config::default();

    let violations = run(&snapshot, &config);
    let target: Vec<_> = violations
        .iter()
        .filter(|v| v.rule_id == TARGET_RULE)
        .collect();

    assert!(!target.is_empty());
    for violation in target {
        assert_eq!(
            violation.severity,
            Severity::Warning,
            "without an override, the rule's default severity must survive",
        );
    }
}

#[test]
fn override_on_unknown_rule_id_is_ignored_cleanly() {
    let snapshot = PlumbSnapshot::canned();
    // Plausibly-shaped but nonexistent id — no rule registers under it.
    let config = config_with_override("does-not-exist/no-such-rule", Severity::Error);

    // Must not panic, must not produce a spurious emission for the
    // unknown id, and must not perturb the default severity of real
    // rules that fire against the canned snapshot.
    let violations = run(&snapshot, &config);

    assert!(
        violations
            .iter()
            .all(|v| v.rule_id != "does-not-exist/no-such-rule"),
        "unknown override must not synthesise a violation",
    );

    let target: Vec<_> = violations
        .iter()
        .filter(|v| v.rule_id == TARGET_RULE)
        .collect();
    assert!(!target.is_empty());
    for violation in target {
        assert_eq!(
            violation.severity,
            Severity::Warning,
            "unknown override must not bleed into other rules",
        );
    }
}
