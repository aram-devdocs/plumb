//! Static rule-doc table for the `explain_rule` MCP tool.
//!
//! Each entry pairs a built-in rule id (`<category>/<id>`) with the
//! markdown body of `docs/src/rules/<slug>.md`, where `slug` is the
//! rule id with `/` replaced by `-`. The markdown is embedded at
//! compile time via `include_str!`, keeping the binary self-contained
//! and the response a pure function of `rule_id`.
//!
//! The table MUST stay in lock-step with
//! [`plumb_core::rules::register_builtin`]. The
//! `every_builtin_rule_has_doc_entry` test in
//! `tests/mcp_protocol.rs` guards against drift.

// Items are `pub(crate)` because this module is private but needs to be
// visible to `lib.rs`. The clippy nursery lint flags `pub(crate)` in
// private modules as redundant; we keep it explicit for clarity since
// `rule_ids` below is re-exported with `pub`.
#![allow(clippy::redundant_pub_crate)]

/// A built-in rule's canonical documentation entry.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RuleDoc {
    /// Stable rule id, `<category>/<id>` — matches `Rule::id`.
    pub(crate) rule_id: &'static str,
    /// Markdown body of `docs/src/rules/<slug>.md`.
    pub(crate) markdown: &'static str,
}

/// Table of every built-in rule's documentation. Entries are sorted by
/// `rule_id` for deterministic iteration.
pub(crate) const RULE_DOCS: &[RuleDoc] = &[
    RuleDoc {
        rule_id: "a11y/touch-target",
        markdown: include_str!("../../../docs/src/rules/a11y-touch-target.md"),
    },
    RuleDoc {
        rule_id: "color/contrast-aa",
        markdown: include_str!("../../../docs/src/rules/color-contrast-aa.md"),
    },
    RuleDoc {
        rule_id: "color/palette-conformance",
        markdown: include_str!("../../../docs/src/rules/color-palette-conformance.md"),
    },
    RuleDoc {
        rule_id: "edge/near-alignment",
        markdown: include_str!("../../../docs/src/rules/edge-near-alignment.md"),
    },
    RuleDoc {
        rule_id: "opacity/scale-conformance",
        markdown: include_str!("../../../docs/src/rules/opacity-scale-conformance.md"),
    },
    RuleDoc {
        rule_id: "radius/scale-conformance",
        markdown: include_str!("../../../docs/src/rules/radius-scale-conformance.md"),
    },
    RuleDoc {
        rule_id: "shadow/scale-conformance",
        markdown: include_str!("../../../docs/src/rules/shadow-scale-conformance.md"),
    },
    RuleDoc {
        rule_id: "sibling/height-consistency",
        markdown: include_str!("../../../docs/src/rules/sibling-height-consistency.md"),
    },
    RuleDoc {
        rule_id: "sibling/padding-consistency",
        markdown: include_str!("../../../docs/src/rules/sibling-padding-consistency.md"),
    },
    RuleDoc {
        rule_id: "spacing/grid-conformance",
        markdown: include_str!("../../../docs/src/rules/spacing-grid-conformance.md"),
    },
    RuleDoc {
        rule_id: "spacing/scale-conformance",
        markdown: include_str!("../../../docs/src/rules/spacing-scale-conformance.md"),
    },
    RuleDoc {
        rule_id: "type/family-conformance",
        markdown: include_str!("../../../docs/src/rules/type-family-conformance.md"),
    },
    RuleDoc {
        rule_id: "type/scale-conformance",
        markdown: include_str!("../../../docs/src/rules/type-scale-conformance.md"),
    },
    RuleDoc {
        rule_id: "type/weight-conformance",
        markdown: include_str!("../../../docs/src/rules/type-weight-conformance.md"),
    },
    RuleDoc {
        rule_id: "z/scale-conformance",
        markdown: include_str!("../../../docs/src/rules/z-scale-conformance.md"),
    },
];

/// Look up the markdown body for a rule id. Returns `None` for unknown
/// ids — callers map this to a JSON-RPC `-32602` error.
pub(crate) fn lookup(rule_id: &str) -> Option<&'static RuleDoc> {
    RULE_DOCS.iter().find(|entry| entry.rule_id == rule_id)
}

/// Every rule id with a documentation entry. Test-only — used by the
/// drift guard that asserts the table matches `register_builtin()`.
#[doc(hidden)]
#[must_use]
pub fn rule_ids() -> Vec<&'static str> {
    RULE_DOCS.iter().map(|entry| entry.rule_id).collect()
}

/// Build the canonical book URL for a rule id. Mirrors the doc-page
/// slug convention (`/` → `-`).
pub(crate) fn doc_url(rule_id: &str) -> String {
    let slug = rule_id.replace('/', "-");
    format!("https://plumb.aramhammoudeh.com/rules/{slug}")
}
