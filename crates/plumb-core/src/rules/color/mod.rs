//! Color rules.
//!
//! Currently:
//!
//! - [`contrast_aa`] — enforce WCAG 2.1 AA text contrast from
//!   existing computed styles.
//! - [`palette_conformance`] — flag computed colors that aren't on the
//!   configured palette, measured by CIEDE2000 (ΔE00) in CIE Lab space.

pub mod contrast_aa;
pub mod palette_conformance;

/// Computed-style properties this category inspects.
///
/// Order is the deterministic emission order: a single offending node
/// can produce one violation per property, sorted alphabetically by
/// property name within the rule's loop. The engine's outer
/// `(rule_id, viewport, selector, dom_order)` sort then re-orders
/// across nodes and rules — within a `(rule_id, selector)` pair the
/// emission order is preserved by the stable sort, so two violations
/// on the same node read in property order.
pub(crate) const COLOR_PROPERTIES: &[&str] = &[
    "background-color",
    "border-bottom-color",
    "border-left-color",
    "border-right-color",
    "border-top-color",
    "color",
    "outline-color",
];
