//! Violation reporting types — the public shape of Plumb's output.
//!
//! These types are serialized directly to JSON, SARIF, and the MCP-compact
//! structured block. Every field is `#[serde(deny_unknown_fields)]` at the
//! config boundary, but violations tolerate forward-compatible additions.

use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How severe a violation is. Maps to CLI exit-code thresholds and to the
/// SARIF `level` field.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Suggestion — ignored by default CI thresholds.
    Info,
    /// Warning — CI-configurable.
    Warning,
    /// Error — fails CI by default.
    Error,
}

impl Severity {
    /// Human-readable label used in the pretty formatter.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// How confident the rule engine is that a suggested fix is safe to apply.
/// Mirrors ESLint's suggestion/fix distinction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Automated fix would be safe.
    High,
    /// Fix is plausible but needs human review.
    Medium,
    /// Fix is speculative.
    Low,
}

/// The kind of fix a rule proposes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FixKind {
    /// Replace a CSS property value.
    CssPropertyReplace {
        /// Property name (e.g. `padding-inline`).
        property: String,
        /// Current value.
        from: String,
        /// Proposed value.
        to: String,
    },
    /// Remove a CSS property entirely.
    CssPropertyRemove {
        /// Property name.
        property: String,
    },
    /// Wrap the current element in a new element.
    WrapElement {
        /// The tag to wrap with.
        tag: String,
    },
    /// Insert an attribute on an element.
    AddAttribute {
        /// Attribute name.
        name: String,
        /// Attribute value.
        value: String,
    },
    /// Free-form suggestion with no structured patch.
    Description {
        /// Human-readable guidance.
        text: String,
    },
}

/// A single fix proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Fix {
    /// The kind and payload.
    pub kind: FixKind,
    /// Human-readable description.
    pub description: String,
    /// How confident the rule is in this fix.
    pub confidence: Confidence,
}

/// Integer pixel rectangle in the viewport's coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Rect {
    /// X offset in CSS pixels.
    pub x: i32,
    /// Y offset in CSS pixels.
    pub y: i32,
    /// Width in CSS pixels.
    pub width: u32,
    /// Height in CSS pixels.
    pub height: u32,
}

/// Named viewport the snapshot was taken at. Matches the config's `viewports`
/// map keys.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub struct ViewportKey(pub String);

impl ViewportKey {
    /// Construct from a string slice.
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// The underlying string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Deterministic run identifier. Derived from a content hash of the config
/// and snapshot inputs — never from a clock or random source.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct RunId(pub String);

impl RunId {
    /// Construct a `RunId` from an already-computed hash string.
    #[must_use]
    pub fn new(hash: impl Into<String>) -> Self {
        Self(hash.into())
    }
}

/// A single rule violation. This is the canonical unit of Plumb's output.
///
/// Sort key for deterministic output is `(rule_id, viewport, selector, dom_order)`.
///
/// `Eq` and `Hash` aren't derived: `metadata` carries `serde_json::Value`,
/// which is `PartialEq` only (floats). Engine dedup uses `PartialEq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Violation {
    /// Stable rule identifier, e.g. `spacing/hard-coded-gap`.
    pub rule_id: String,
    /// Severity — drives exit code.
    pub severity: Severity,
    /// Human-readable summary.
    pub message: String,
    /// CSS selector path to the offending node.
    pub selector: String,
    /// Which viewport the violation was detected in.
    pub viewport: ViewportKey,
    /// Bounding rect in viewport pixels, if applicable.
    pub rect: Option<Rect>,
    /// DOM document order — used as a stable tiebreaker.
    pub dom_order: u64,
    /// Proposed fix, if the rule has one.
    pub fix: Option<Fix>,
    /// Documentation URL — `plumb explain` uses this for a deep link.
    pub doc_url: String,
    /// Arbitrary rule-specific metadata. Must round-trip through JSON.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub metadata: IndexMap<String, serde_json::Value>,
}

impl Violation {
    /// The deterministic sort key tuple. Public so downstream tools can
    /// compose it into their own orderings.
    #[must_use]
    pub fn sort_key(&self) -> (&str, &str, &str, u64) {
        (
            self.rule_id.as_str(),
            self.viewport.as_str(),
            self.selector.as_str(),
            self.dom_order,
        )
    }
}

/// A bounded accumulator passed to rules during evaluation. Rules push
/// violations here instead of allocating their own `Vec` — this gives the
/// engine a single place to enforce per-rule budgets in the future.
#[derive(Debug)]
pub struct ViolationSink<'a> {
    buffer: &'a mut Vec<Violation>,
}

impl<'a> ViolationSink<'a> {
    /// Wrap a mutable `Vec`. The engine is the only caller.
    #[must_use]
    pub fn new(buffer: &'a mut Vec<Violation>) -> Self {
        Self { buffer }
    }

    /// Record a violation.
    pub fn push(&mut self, violation: Violation) {
        self.buffer.push(violation);
    }

    /// How many violations have been recorded so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Whether no violations have been recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}
