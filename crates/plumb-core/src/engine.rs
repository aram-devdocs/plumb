//! The deterministic rule engine.
//!
//! Given a snapshot and a config, [`run`] evaluates every built-in rule
//! and returns a sorted, deduplicated `Vec<Violation>`. The sort key is
//! `(rule_id, viewport, selector, dom_order)` — see `docs/local/prd.md` §9.

use crate::config::Config;
use crate::report::{ViewportKey, Violation, ViolationSink};
use crate::rules::{Rule, register_builtin};
use crate::snapshot::{PlumbSnapshot, SnapshotCtx};
use rayon::prelude::*;

/// Run every built-in rule against the snapshot. Output is sorted and
/// deduplicated before return.
///
/// # Determinism
///
/// This function is pure — no wall-clock, no RNG, no environment access.
/// Running it twice with the same inputs yields byte-identical output.
#[must_use]
pub fn run(snapshot: &PlumbSnapshot, config: &Config) -> Vec<Violation> {
    let rules = register_builtin();
    run_rules(snapshot, config, &rules)
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
            // Honor per-rule enable/disable. Severity overrides are not yet
            // applied at engine level — a rule still emits with its default
            // severity; the formatter layer remaps if the config asks.
            config.rules.get(rule.id()).is_none_or(|over| over.enabled)
        })
        .flat_map(|rule| {
            let mut local = Vec::new();
            let mut sink = ViolationSink::new(&mut local);
            rule.check(&ctx, config, &mut sink);
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
    use crate::config::Config;
    use crate::report::{Severity, ViewportKey, Violation, ViolationSink};
    use crate::rules::Rule;
    use crate::snapshot::{PlumbSnapshot, SnapshotCtx};
    use indexmap::IndexMap;

    use super::run_rules;

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
}
