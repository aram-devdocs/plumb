//! The deterministic rule engine.
//!
//! Given a snapshot and a config, [`run`] evaluates every built-in rule
//! and returns a sorted, deduplicated `Vec<Violation>`. The sort key is
//! `(rule_id, viewport, selector, dom_order)` — see `docs/local/prd.md` §9.

use crate::config::{Config, IgnoreRule};
use crate::report::{ViewportKey, Violation, ViolationSink};
use crate::rules::{Rule, register_builtin};
use crate::snapshot::{PlumbSnapshot, SnapshotCtx};
use rayon::prelude::*;

/// A partitioned engine result: reported and ignored violations split
/// according to the active `[[ignore]]` config entries.
///
/// Both vectors are sorted by [`Violation::sort_key`] in ascending
/// order. `ignored` is empty when the config has no `[[ignore]]`
/// entries or none of the active entries match the snapshot's
/// violations.
///
/// This is what the CLI and MCP server consume when they need to
/// display "N violations suppressed by config" alongside the rendered
/// list.
#[derive(Debug, Clone, PartialEq)]
pub struct RunReport {
    /// Violations that survived the ignore filter and SHOULD be
    /// reported to the user. Sorted by [`Violation::sort_key`].
    pub reported: Vec<Violation>,
    /// Violations that an `[[ignore]]` entry matched. Sorted by
    /// [`Violation::sort_key`]. Excluded from the standard output;
    /// surfaced only in the count footer / JSON envelope so users can
    /// audit what their config silenced.
    pub ignored: Vec<Violation>,
}

impl RunReport {
    /// Empty report — no violations reported, no violations ignored.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            reported: Vec::new(),
            ignored: Vec::new(),
        }
    }

    /// Total raw violation count (reported + ignored). Useful for
    /// the "N violations, M suppressed" status line.
    #[must_use]
    pub fn total(&self) -> usize {
        self.reported.len() + self.ignored.len()
    }
}

/// Run every built-in rule against the snapshot. Output is sorted and
/// deduplicated before return.
///
/// # Determinism
///
/// This function is pure — no wall-clock, no RNG, no environment access.
/// Running it twice with the same inputs yields byte-identical output.
///
/// This is a thin wrapper over [`run_many`] for the single-snapshot case.
#[must_use]
pub fn run(snapshot: &PlumbSnapshot, config: &Config) -> Vec<Violation> {
    run_many([snapshot], config)
}

/// Run every built-in rule against each snapshot in `snapshots` and
/// return their merged, sorted, deduplicated violation list.
///
/// # Determinism
///
/// Output is byte-identical regardless of input order. The merge is
/// re-sorted by [`Violation::sort_key`] —
/// `(rule_id, viewport, selector, dom_order)`, the same key the
/// single-snapshot path uses — so a `desktop`-first config and a
/// `mobile`-first config yield
/// the same `Vec<Violation>`. Like [`run`], this function performs no
/// I/O, no RNG, and no clock reads.
///
/// `[[ignore]]` entries in `config` partition the post-rule output;
/// the returned `Vec` is the reported subset only. Use
/// [`run_report`] when the caller needs the ignored count or the
/// ignored violation list.
#[must_use]
pub fn run_many<'a, I>(snapshots: I, config: &Config) -> Vec<Violation>
where
    I: IntoIterator<Item = &'a PlumbSnapshot>,
{
    run_report(snapshots, config).reported
}

/// Like [`run_many`] but returns a [`RunReport`] partitioning the
/// violation set into reported vs. ignored according to
/// `config.ignore`.
///
/// # Determinism
///
/// Same invariants as [`run_many`]. Both vectors in the returned
/// report are sorted by [`Violation::sort_key`]; iteration over
/// `config.ignore` is in declaration order (it's a `Vec`).
#[must_use]
pub fn run_report<'a, I>(snapshots: I, config: &Config) -> RunReport
where
    I: IntoIterator<Item = &'a PlumbSnapshot>,
{
    let rules = register_builtin();
    let mut buffer: Vec<Violation> = snapshots
        .into_iter()
        .flat_map(|snapshot| run_rules(snapshot, config, &rules))
        .collect();

    // Re-sort across snapshots; `run_rules` already sorts within one
    // snapshot, but the cross-snapshot merge still needs an outer pass.
    buffer.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    buffer.dedup();

    apply_ignores(buffer, &config.ignore)
}

/// Partition `violations` into `(reported, ignored)` according to
/// `ignores`.
///
/// Matching is exact-string on `Violation::selector`. When an entry's
/// `rule_id` is `Some(id)`, the violation must also have
/// `Violation::rule_id == id`. When `rule_id` is `None`, every rule's
/// violation at the selector is suppressed.
///
/// Iteration over `ignores` follows declaration order; matching is
/// short-circuited on the first hit per violation. Both the reported
/// and ignored vectors preserve their input ordering, which is the
/// caller's pre-sorted [`Violation::sort_key`] order.
#[must_use]
pub fn apply_ignores(violations: Vec<Violation>, rules: &[IgnoreRule]) -> RunReport {
    if rules.is_empty() {
        return RunReport {
            reported: violations,
            ignored: Vec::new(),
        };
    }

    let mut reported = Vec::with_capacity(violations.len());
    let mut suppressed = Vec::new();

    for violation in violations {
        if ignore_matches(&violation, rules) {
            suppressed.push(violation);
        } else {
            reported.push(violation);
        }
    }

    RunReport {
        reported,
        ignored: suppressed,
    }
}

/// `true` when any entry in `rules` matches `violation`. Selector
/// equality is exact-string; `rule_id` is matched only when the entry
/// declares one.
fn ignore_matches(violation: &Violation, rules: &[IgnoreRule]) -> bool {
    rules.iter().any(|rule| {
        if rule.selector != violation.selector {
            return false;
        }
        match &rule.rule_id {
            Some(id) => id == &violation.rule_id,
            None => true,
        }
    })
}

fn run_rules(snapshot: &PlumbSnapshot, config: &Config, rules: &[Box<dyn Rule>]) -> Vec<Violation> {
    let ctx = if config.viewports.is_empty() {
        SnapshotCtx::new(snapshot)
    } else {
        SnapshotCtx::with_viewports(
            snapshot,
            config.viewports.keys().cloned().map(ViewportKey::new),
        )
    };
    let mut buffer: Vec<Violation> = rules
        .par_iter()
        .filter(|rule| {
            // Honor per-rule enable/disable. Severity overrides are
            // applied below, after each rule's local emissions are
            // collected — rules are pure and emit with their
            // `default_severity()`; the engine owns the remap so the
            // CLI's exit-code logic and every formatter see a single
            // post-override view.
            config.rules.get(rule.id()).is_none_or(|over| over.enabled)
        })
        .flat_map(|rule| {
            let mut local = Vec::new();
            let mut sink = ViolationSink::new(&mut local);
            rule.check(&ctx, config, &mut sink);
            // Apply [rules."<id>"].severity, if set. Lookup is a single
            // IndexMap probe per rule, regardless of how many
            // violations it emitted.
            if let Some(override_severity) =
                config.rules.get(rule.id()).and_then(|over| over.severity)
            {
                for violation in &mut local {
                    violation.severity = override_severity;
                }
            }
            local
        })
        .collect();

    // Deterministic sort.
    buffer.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));

    // Dedup exact matches — different rules may independently flag the
    // same node; keep the first occurrence in sort order.
    buffer.dedup();

    buffer
}

#[cfg(test)]
mod tests {
    use crate::config::{Config, IgnoreRule};
    use crate::report::{Severity, ViewportKey, Violation, ViolationSink};
    use crate::rules::Rule;
    use crate::snapshot::{PlumbSnapshot, SnapshotCtx};
    use indexmap::IndexMap;

    use super::{apply_ignores, run_report, run_rules};

    #[derive(Debug, Clone, Copy)]
    struct Emission {
        selector: &'static str,
        dom_order: u64,
    }

    #[derive(Debug)]
    struct OutOfOrderRule {
        id: &'static str,
        emissions: &'static [Emission],
    }

    impl Rule for OutOfOrderRule {
        fn id(&self) -> &'static str {
            self.id
        }

        fn default_severity(&self) -> Severity {
            Severity::Warning
        }

        fn summary(&self) -> &'static str {
            "Test-only rule that emits fixed violations."
        }

        fn check(&self, ctx: &SnapshotCtx<'_>, _config: &Config, sink: &mut ViolationSink<'_>) {
            for emission in self.emissions {
                sink.push(test_violation(
                    self.id(),
                    emission.selector,
                    ctx.snapshot().viewport.clone(),
                    emission.dom_order,
                ));
            }
        }
    }

    fn test_violation(
        rule_id: &str,
        selector: &str,
        viewport: ViewportKey,
        dom_order: u64,
    ) -> Violation {
        Violation {
            rule_id: rule_id.to_owned(),
            severity: Severity::Warning,
            message: "test violation".to_owned(),
            selector: selector.to_owned(),
            viewport,
            rect: None,
            dom_order,
            fix: None,
            doc_url: "https://plumb.aramhammoudeh.com/rules/test-only".to_owned(),
            metadata: IndexMap::new(),
        }
    }

    #[test]
    fn run_rules_sorts_parallel_rule_output() {
        const ALPHA_EMISSIONS: &[Emission] = &[
            Emission {
                selector: "html > zed",
                dom_order: 9,
            },
            Emission {
                selector: "html > alpha",
                dom_order: 1,
            },
        ];
        const ZED_EMISSIONS: &[Emission] = &[
            Emission {
                selector: "html > body",
                dom_order: 2,
            },
            Emission {
                selector: "html",
                dom_order: 0,
            },
        ];

        let snapshot = PlumbSnapshot::canned();
        let config = Config::default();
        let rules: Vec<Box<dyn Rule>> = vec![
            Box::new(OutOfOrderRule {
                id: "z/rule",
                emissions: ZED_EMISSIONS,
            }),
            Box::new(OutOfOrderRule {
                id: "a/rule",
                emissions: ALPHA_EMISSIONS,
            }),
        ];

        let first = run_rules(&snapshot, &config, &rules);
        let second = run_rules(&snapshot, &config, &rules);

        assert_eq!(first, second);
        assert_eq!(
            first.iter().map(Violation::sort_key).collect::<Vec<_>>(),
            vec![
                ("a/rule", "desktop", "html > alpha", 1),
                ("a/rule", "desktop", "html > zed", 9),
                ("z/rule", "desktop", "html", 0),
                ("z/rule", "desktop", "html > body", 2),
            ],
        );
    }

    fn fixture_violation(rule_id: &str, selector: &str, dom_order: u64) -> Violation {
        Violation {
            rule_id: rule_id.to_owned(),
            severity: Severity::Warning,
            message: "test".to_owned(),
            selector: selector.to_owned(),
            viewport: ViewportKey::new("desktop"),
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

    #[test]
    fn apply_ignores_passthrough_when_empty() {
        let v = vec![
            fixture_violation("spacing/grid-conformance", "html > body", 2),
            fixture_violation("color/palette-conformance", "main", 5),
        ];
        let report = apply_ignores(v.clone(), &[]);
        assert_eq!(report.reported, v);
        assert!(report.ignored.is_empty());
    }

    #[test]
    fn apply_ignores_selector_only_match_suppresses_all_rules() {
        let v = vec![
            fixture_violation("spacing/grid-conformance", "html > body", 2),
            fixture_violation("color/palette-conformance", "html > body", 2),
            fixture_violation("spacing/grid-conformance", "main", 5),
        ];
        let ignores = vec![IgnoreRule {
            selector: "html > body".to_owned(),
            rule_id: None,
            reason: "test".to_owned(),
        }];
        let report = apply_ignores(v, &ignores);
        assert_eq!(report.reported.len(), 1);
        assert_eq!(report.reported[0].selector, "main");
        assert_eq!(report.ignored.len(), 2);
    }

    #[test]
    fn apply_ignores_selector_plus_rule_id_filters_one_rule_only() {
        let v = vec![
            fixture_violation("spacing/grid-conformance", "html > body", 2),
            fixture_violation("color/palette-conformance", "html > body", 2),
        ];
        let ignores = vec![IgnoreRule {
            selector: "html > body".to_owned(),
            rule_id: Some("spacing/grid-conformance".to_owned()),
            reason: "test".to_owned(),
        }];
        let report = apply_ignores(v, &ignores);
        assert_eq!(report.reported.len(), 1);
        assert_eq!(report.reported[0].rule_id, "color/palette-conformance");
        assert_eq!(report.ignored.len(), 1);
        assert_eq!(report.ignored[0].rule_id, "spacing/grid-conformance");
    }

    #[test]
    fn apply_ignores_selector_mismatch_does_not_filter() {
        let v = vec![fixture_violation(
            "spacing/grid-conformance",
            "html > body",
            2,
        )];
        let ignores = vec![IgnoreRule {
            selector: "html > body > div".to_owned(),
            rule_id: None,
            reason: "test".to_owned(),
        }];
        let report = apply_ignores(v.clone(), &ignores);
        assert_eq!(report.reported, v);
        assert!(report.ignored.is_empty());
    }

    #[test]
    fn apply_ignores_is_deterministic_across_runs() {
        let v = vec![
            fixture_violation("a/rule", "html > body", 1),
            fixture_violation("a/rule", "html > body", 2),
            fixture_violation("b/rule", "main", 3),
        ];
        let ignores = vec![IgnoreRule {
            selector: "html > body".to_owned(),
            rule_id: None,
            reason: "x".to_owned(),
        }];
        let first = apply_ignores(v.clone(), &ignores);
        let second = apply_ignores(v, &ignores);
        assert_eq!(first, second);
    }

    #[test]
    fn run_report_applies_ignores_against_real_engine_output() {
        let snapshot = PlumbSnapshot::canned();
        // The canned snapshot has one violation: spacing/grid-conformance
        // on `html > body` (padding-top: 13px is off-grid against base
        // unit 4).
        let mut config = Config::default();
        config.ignore.push(IgnoreRule {
            selector: "html > body".to_owned(),
            rule_id: Some("spacing/grid-conformance".to_owned()),
            reason: "canned snapshot exemption".to_owned(),
        });
        let report = run_report([&snapshot], &config);
        assert!(report.reported.is_empty());
        assert_eq!(report.ignored.len(), 1);
        assert_eq!(report.ignored[0].rule_id, "spacing/grid-conformance");
        assert_eq!(report.ignored[0].selector, "html > body");
    }
}
