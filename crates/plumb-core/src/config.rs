//! Config schema — the shape of `plumb.toml`.
//!
//! The real fields are spelled out in `docs/local/prd.md` §12.2. The
//! full shape is defined up front (so the JSON Schema emitted by
//! `plumb schema` is stable across PRs) even though most fields are
//! unused by the rules that have shipped so far.

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
    #[serde(default, rename = "type")]
    #[schemars(rename = "type")]
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

    /// Box-shadow spec.
    #[serde(default)]
    pub shadow: ShadowSpec,

    /// Z-index spec.
    #[serde(default)]
    pub z_index: ZIndexSpec,

    /// Opacity spec.
    #[serde(default)]
    pub opacity: OpacitySpec,

    /// Vertical rhythm spec.
    #[serde(default)]
    pub rhythm: RhythmSpec,

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpacingSpec {
    /// Base unit in pixels; discrete scale is multiples of this.
    #[serde(default = "default_base_unit")]
    pub base_unit: u32,
    /// Allowed spacing values in pixels.
    #[serde(default)]
    pub scale: Vec<u32>,
    /// Named tokens mapped to their pixel values.
    #[serde(default)]
    pub tokens: IndexMap<String, u32>,
}

fn default_base_unit() -> u32 {
    4
}

impl Default for SpacingSpec {
    fn default() -> Self {
        Self {
            base_unit: default_base_unit(),
            scale: Vec::new(),
            tokens: IndexMap::new(),
        }
    }
}

/// Type scale spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct TypeScaleSpec {
    /// Allowed font families.
    #[serde(default)]
    pub families: Vec<String>,
    /// Allowed font weights.
    #[serde(default)]
    pub weights: Vec<u16>,
    /// Allowed font sizes in pixels.
    #[serde(default)]
    pub scale: Vec<u32>,
    /// Named type tokens mapped to their pixel values.
    #[serde(default)]
    pub tokens: IndexMap<String, u32>,
}

/// Color spec.
///
/// Tokens are flat name → hex pairs. Slash-delimited names
/// (`"bg/canvas"`, `"fg/primary"`) namespace the palette without
/// requiring nested tables — TOML quotes the key, the rule engine
/// treats the slash as a hint for grouping in diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ColorSpec {
    /// Named tokens mapped to hex values (e.g. `#0b7285`). Slash-delimited
    /// keys (`"bg/canvas"`) act as informal namespaces.
    #[serde(default)]
    pub tokens: IndexMap<String, String>,
    /// CIEDE2000 Delta-E tolerance when matching off-palette colors.
    #[serde(default = "default_delta_e")]
    pub delta_e_tolerance: f32,
}

fn default_delta_e() -> f32 {
    2.0
}

impl Default for ColorSpec {
    fn default() -> Self {
        Self {
            tokens: IndexMap::new(),
            delta_e_tolerance: default_delta_e(),
        }
    }
}

/// Border-radius spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct RadiusSpec {
    /// Allowed border-radius values in pixels.
    ///
    /// Naming matches `spacing.scale` and `type.scale` for consistency.
    #[serde(default)]
    pub scale: Vec<u32>,
}

/// Alignment / layout spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AlignmentSpec {
    /// Grid column count, if the design uses a fixed grid.
    #[serde(default)]
    pub grid_columns: Option<u32>,
    /// Container gutter in pixels.
    #[serde(default)]
    pub gutter_px: Option<u32>,
    /// Edge-clustering tolerance in pixels for `edge/near-alignment`.
    /// Defaults to 3 px.
    #[serde(default = "default_alignment_tolerance_px")]
    pub tolerance_px: u32,
}

fn default_alignment_tolerance_px() -> u32 {
    3
}

impl Default for AlignmentSpec {
    fn default() -> Self {
        Self {
            grid_columns: None,
            gutter_px: None,
            tolerance_px: default_alignment_tolerance_px(),
        }
    }
}

/// Box-shadow spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct ShadowSpec {
    /// Allowed box-shadow values. Each entry is a complete shadow
    /// expression as returned by `getComputedStyle`.
    #[serde(default)]
    pub scale: Vec<String>,
}

/// Z-index spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct ZIndexSpec {
    /// Allowed z-index values.
    #[serde(default)]
    pub scale: Vec<i32>,
}

/// Opacity spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct OpacitySpec {
    /// Allowed opacity values in the range `[0.0, 1.0]`.
    #[serde(default)]
    pub scale: Vec<f32>,
}

/// Vertical-rhythm spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_field_names)]
pub struct RhythmSpec {
    /// Base line-height in pixels.
    #[serde(default)]
    pub base_line_px: u32,
    /// Tolerance in pixels for rhythm checks.
    #[serde(default = "default_rhythm_tolerance_px")]
    pub tolerance_px: u32,
    /// Cap-height fallback in pixels when font metrics are unavailable.
    #[serde(default)]
    pub cap_height_fallback_px: u32,
}

fn default_rhythm_tolerance_px() -> u32 {
    2
}

impl Default for RhythmSpec {
    fn default() -> Self {
        Self {
            base_line_px: 0,
            tolerance_px: default_rhythm_tolerance_px(),
            cap_height_fallback_px: 0,
        }
    }
}

/// Accessibility spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct A11ySpec {
    /// Minimum contrast ratio to enforce (e.g. `4.5` for WCAG AA body text).
    #[serde(default)]
    pub min_contrast_ratio: Option<f32>,
    /// Minimum interactive-element size for `a11y/touch-target`.
    #[serde(default)]
    pub touch_target: TouchTargetSpec,
}

/// Touch-target threshold per WCAG 2.5.8 (Target Size, Minimum).
///
/// Defaults to 24×24 CSS pixels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TouchTargetSpec {
    /// Minimum interactive width in CSS pixels.
    #[serde(default = "default_touch_target_px")]
    pub min_width_px: u32,
    /// Minimum interactive height in CSS pixels.
    #[serde(default = "default_touch_target_px")]
    pub min_height_px: u32,
}

fn default_touch_target_px() -> u32 {
    24
}

impl Default for TouchTargetSpec {
    fn default() -> Self {
        Self {
            min_width_px: default_touch_target_px(),
            min_height_px: default_touch_target_px(),
        }
    }
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
