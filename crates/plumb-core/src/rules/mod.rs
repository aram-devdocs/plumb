//! Rule engine — the [`Rule`] trait and the built-in registry.
//!
//! To add a new rule, see `.agents/rules/rule-engine-patterns.md`. The
//! short version:
//!
//! 1. Add a module under `src/rules/` with a type implementing [`Rule`].
//! 2. Register it in [`register_builtin`].
//! 3. Add a golden snapshot test under `tests/`.
//! 4. Document it at `docs/src/rules/<rule-id>.md`.

pub mod a11y;
pub mod baseline;
pub mod color;
pub mod edge;
pub mod opacity;
pub mod radius;
pub mod shadow;
pub mod sibling;
pub mod spacing;
pub mod type_;
pub mod z;

mod util;

use crate::config::Config;
use crate::report::{Severity, ViolationSink};
use crate::snapshot::SnapshotCtx;

/// Static metadata needed by output formats and rule listings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleMetadata {
    /// Stable identifier, `<category>/<id>` (e.g. `spacing/grid-conformance`).
    pub id: String,
    /// One-line human-readable summary.
    pub summary: String,
    /// Canonical documentation URL for this rule.
    pub doc_url: String,
    /// Default severity if the user's config doesn't override it.
    pub default_severity: Severity,
}

impl RuleMetadata {
    /// Build metadata from a registered rule.
    #[must_use]
    pub fn from_rule(rule: &dyn Rule) -> Self {
        Self {
            id: rule.id().to_owned(),
            summary: rule.summary().to_owned(),
            doc_url: rule.doc_url(),
            default_severity: rule.default_severity(),
        }
    }
}

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

    /// Canonical documentation URL for this rule.
    fn doc_url(&self) -> String {
        let slug = self.id().replace('/', "-");
        format!("https://plumb.aramhammoudeh.com/rules/{slug}")
    }

    /// Evaluate the rule against a snapshot.
    fn check(&self, ctx: &SnapshotCtx<'_>, config: &Config, sink: &mut ViolationSink<'_>);
}

/// Return every built-in rule in registration order. Registration order
/// is **not** part of the public contract — the engine sorts the resulting
/// violations by `(rule_id, viewport, selector, dom_order)` before return.
#[must_use]
pub fn register_builtin() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(a11y::touch_target::TouchTarget),
        Box::new(baseline::rhythm::Rhythm),
        Box::new(color::contrast_aa::ContrastAa),
        Box::new(color::palette_conformance::PaletteConformance),
        Box::new(edge::near_alignment::NearAlignment),
        Box::new(opacity::scale_conformance::ScaleConformance),
        Box::new(radius::scale_conformance::ScaleConformance),
        Box::new(shadow::scale_conformance::ScaleConformance),
        Box::new(sibling::height_consistency::HeightConsistency),
        Box::new(sibling::padding_consistency::PaddingConsistency),
        Box::new(spacing::grid_conformance::GridConformance),
        Box::new(spacing::scale_conformance::ScaleConformance),
        Box::new(type_::family_conformance::FamilyConformance),
        Box::new(type_::scale_conformance::ScaleConformance),
        Box::new(type_::weight_conformance::WeightConformance),
        Box::new(z::scale_conformance::ScaleConformance),
    ]
}

/// Return metadata for every built-in rule, sorted by rule id.
#[must_use]
pub fn builtin_rule_metadata() -> Vec<RuleMetadata> {
    let mut metadata: Vec<RuleMetadata> = register_builtin()
        .iter()
        .map(|rule| RuleMetadata::from_rule(rule.as_ref()))
        .collect();
    metadata.sort_by(|a, b| a.id.cmp(&b.id));
    metadata
}
