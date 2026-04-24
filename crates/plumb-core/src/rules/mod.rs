//! Rule engine — the [`Rule`] trait and the built-in registry.
//!
//! To add a new rule, see `.agents/rules/rule-engine-patterns.md`. The
//! short version:
//!
//! 1. Add a module under `src/rules/` with a type implementing [`Rule`].
//! 2. Register it in [`register_builtin`].
//! 3. Add a golden snapshot test under `tests/`.
//! 4. Document it at `docs/src/rules/<rule-id>.md`.

pub mod placeholder;
pub mod spacing;
pub mod type_;

mod util;

use crate::config::Config;
use crate::report::{Severity, ViolationSink};
use crate::snapshot::SnapshotCtx;

/// A rule — the fundamental unit of work in the engine.
///
/// Rules are `Send + Sync` so the engine can evaluate built-in rules in
/// parallel against one shared snapshot context. Implementations must be
/// **pure**: given the same `ctx` and `config`, they must push the same
/// sequence of violations into their local sink every time. Do not rely on
/// shared mutable state, I/O, clocks, environment variables, randomness, or
/// cross-rule ordering; each rule must be safe to run concurrently with any
/// other rule.
pub trait Rule: Send + Sync {
    /// Stable identifier, `<category>/<id>` (e.g. `spacing/hard-coded-gap`).
    fn id(&self) -> &'static str;

    /// Default severity if the user's config doesn't override it.
    fn default_severity(&self) -> Severity;

    /// One-line human-readable summary. Shown in `plumb list-rules`.
    fn summary(&self) -> &'static str;

    /// Evaluate the rule against a snapshot.
    fn check(&self, ctx: &SnapshotCtx<'_>, config: &Config, sink: &mut ViolationSink<'_>);
}

/// Return every built-in rule in registration order. Registration order
/// is **not** part of the public contract — the engine sorts the resulting
/// violations by `(rule_id, viewport, selector, dom_order)` before return.
#[must_use]
pub fn register_builtin() -> Vec<Box<dyn Rule>> {
    // The placeholder is deprecated-on-purpose so it's visible in the
    // compiler output until every walking-skeleton consumer migrates.
    // The registration site is the one place we allow the deprecation.
    #[allow(deprecated)]
    {
        vec![
            Box::new(placeholder::HelloWorld),
            Box::new(spacing::grid_conformance::GridConformance),
            Box::new(spacing::scale_conformance::ScaleConformance),
            Box::new(type_::scale_conformance::ScaleConformance),
        ]
    }
}
