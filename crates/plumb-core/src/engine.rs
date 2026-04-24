//! The deterministic rule engine.
//!
//! Given a snapshot and a config, [`run`] evaluates every built-in rule
//! and returns a sorted, deduplicated `Vec<Violation>`. The sort key is
//! `(rule_id, viewport, selector, dom_order)` — see `docs/local/prd.md` §9.

use crate::config::Config;
use crate::report::{Violation, ViolationSink};
use crate::rules::register_builtin;
use crate::snapshot::{PlumbSnapshot, SnapshotCtx};

/// Run every built-in rule against the snapshot. Output is sorted and
/// deduplicated before return.
///
/// # Determinism
///
/// This function is pure — no wall-clock, no RNG, no environment access.
/// Running it twice with the same inputs yields byte-identical output.
#[must_use]
pub fn run(snapshot: &PlumbSnapshot, config: &Config) -> Vec<Violation> {
    let ctx = SnapshotCtx::new(snapshot);
    let rules = register_builtin();
    let mut buffer: Vec<Violation> = Vec::new();
    {
        let mut sink = ViolationSink::new(&mut buffer);
        for rule in &rules {
            // Honor per-rule enable/disable. Severity overrides are not yet
            // applied at engine level — a rule still emits with its default
            // severity; the formatter layer remaps if the config asks.
            if let Some(over) = config.rules.get(rule.id()) {
                if !over.enabled {
                    continue;
                }
            }
            rule.check(&ctx, config, &mut sink);
        }
    }

    // Deterministic sort.
    buffer.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));

    // Dedup exact matches — different rules may independently flag the
    // same node; keep the first occurrence in sort order.
    buffer.dedup();

    buffer
}
