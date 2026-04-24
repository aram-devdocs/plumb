//! Config schema — the shape of `plumb.toml`.
//!
//! The real fields are spelled out in `docs/local/prd.md` §12.2. The
//! walking skeleton defines the full shape (so the JSON Schema emitted by
//! `plumb schema` is stable across PRs) even though most fields are unused
//! by the single placeholder rule.

use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::report::Severity;

/// Top-level Plumb configuration.
///
/// `Eq` is not derived — several sub-structs carry `f32` fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Named viewports to snapshot the page at.
    #[serde(default)]
    pub viewports: IndexMap<String, ViewportSpec>,

    /// Spacing spec — the allowed discrete values for `gap`, `margin`,
    /// `padding`, etc.
    #[serde(default)]
    pub spacing: SpacingSpec,

    /// Type scale spec.
    #[serde(default)]
    pub type_scale: TypeScaleSpec,

    /// Color palette spec.
    #[serde(default)]
    pub color: ColorSpec,

    /// Border-radius spec.
    #[serde(default)]
    pub radius: RadiusSpec,

    /// Alignment / layout spec.
    #[serde(default)]
    pub alignment: AlignmentSpec,

    /// Accessibility spec.
    #[serde(default)]
    pub a11y: A11ySpec,

    /// Per-rule overrides — severity bumps, enable/disable.
    #[serde(default)]
    pub rules: IndexMap<String, RuleOverride>,
}

/// Specification of a single named viewport.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewportSpec {
    /// Width in CSS pixels.
    pub width: u32,
    /// Height in CSS pixels.
    pub height: u32,
    /// Device pixel ratio. Defaults to 1.0.
    #[serde(default = "default_dpr")]
    pub device_pixel_ratio: f32,
}

fn default_dpr() -> f32 {
    1.0
}

/// Spacing spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct SpacingSpec {
    /// Base unit in pixels; discrete scale is multiples of this.
    #[serde(default)]
    pub base_px: Option<u32>,
    /// Named tokens mapped to their pixel values.
    #[serde(default)]
    pub tokens: IndexMap<String, u32>,
}

/// Type scale spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct TypeScaleSpec {
    /// Allowed font families.
    #[serde(default)]
    pub families: Vec<String>,
    /// Allowed font sizes in pixels.
    #[serde(default)]
    pub sizes_px: Vec<u32>,
    /// Allowed line heights (unitless ratios).
    #[serde(default)]
    pub line_heights: Vec<f32>,
    /// Allowed font weights.
    #[serde(default)]
    pub weights: Vec<u16>,
}

/// Color spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct ColorSpec {
    /// Named tokens mapped to hex values (e.g. `#0b7285`).
    #[serde(default)]
    pub tokens: IndexMap<String, String>,
    /// CIEDE2000 Delta-E tolerance when matching off-palette colors.
    #[serde(default = "default_delta_e")]
    pub delta_e_tolerance: f32,
}

fn default_delta_e() -> f32 {
    2.0
}

/// Border-radius spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct RadiusSpec {
    /// Allowed radii in pixels.
    #[serde(default)]
    pub allowed_px: Vec<u32>,
}

/// Alignment / layout spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct AlignmentSpec {
    /// Grid column count, if the design uses a fixed grid.
    #[serde(default)]
    pub grid_columns: Option<u32>,
    /// Container gutter in pixels.
    #[serde(default)]
    pub gutter_px: Option<u32>,
}

/// Accessibility spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct A11ySpec {
    /// Minimum contrast ratio to enforce (e.g. `4.5` for WCAG AA body text).
    #[serde(default)]
    pub min_contrast_ratio: Option<f32>,
}

/// Per-rule override.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuleOverride {
    /// Enable or disable the rule entirely.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Override the rule's default severity.
    #[serde(default)]
    pub severity: Option<Severity>,
}

fn default_enabled() -> bool {
    true
}

impl Default for RuleOverride {
    fn default() -> Self {
        Self {
            enabled: true,
            severity: None,
        }
    }
}
