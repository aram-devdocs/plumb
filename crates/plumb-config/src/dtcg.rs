//! DTCG 2025.10 token adapter.
//!
//! Imports a [Design Tokens Community Group][dtcg-spec] JSON document
//! into an existing [`plumb_core::Config`]. The adapter is a pure
//! function of `(input bytes, into.snapshot)` — no I/O, no global
//! state — so callers own filesystem reads and tracing.
//!
//! # Mapped types
//!
//! | DTCG `$type`              | Plumb destination                                       |
//! |---------------------------|---------------------------------------------------------|
//! | `color`                   | [`ColorSpec::tokens`] — hex `#rrggbb` / `#rrggbbaa`     |
//! | `dimension` (spacing)     | [`SpacingSpec::tokens`] (parent group is `spacing`,     |
//! |                           | `space`, `gap`, `padding`, or `margin`)                 |
//! | `dimension` (typography)  | [`TypeScaleSpec::tokens`] (parent group is `typography`,|
//! |                           | `type`, `font-size`, `text`, `font`, or `size`)         |
//! | `fontFamily`              | [`TypeScaleSpec::families`] (deduped, insertion-order)  |
//! | `fontWeight`              | [`TypeScaleSpec::weights`]                              |
//! | `radius` / `borderRadius` | [`RadiusSpec::scale`]                                   |
//! | `shadow`                  | warning — no `Config` slot exists yet                   |
//! | other                     | warning — `DtcgWarningKind::UnsupportedType`            |
//!
//! Bare `dimension` tokens whose parent group does not match either
//! heuristic land in [`SpacingSpec::tokens`] (the conservative default,
//! since the spacing slot is the broader catch-all).
//!
//! # Alias resolution
//!
//! The adapter accepts both DTCG forms:
//!
//! * `"$value": "{path.to.token}"` — brace-shorthand.
//! * `"$value": { "$ref": "#/path/to/token" }` — JSON-Pointer object.
//!
//! Resolution is a single forward pass with cycle detection. For each
//! token, the adapter follows aliases until it lands on a literal
//! value, recording every visited path in a `visiting` set. Re-entering
//! a path is a [`ConfigError::DtcgAlias`] cycle error, with the cycle
//! reported in visit order so the failing edge is human-readable.
//! Unresolved references (target missing) raise the same error variant
//! with a single-element cycle naming the dangling path.
//!
//! # Untrusted input
//!
//! Inputs come from user-supplied design-token files, which are
//! frequently auto-generated. The adapter:
//!
//! * Caps tree depth at [`MAX_NESTING`] (256 levels) before parsing
//!   anything user-visible.
//! * Returns a typed [`ConfigError::DtcgParse`] (with a miette
//!   [`NamedSource`] for span-aware diagnostics) on malformed JSON or
//!   schema violations — never panics.
//! * Validates hex colors with the same helper as the canonical config
//!   loader, so a DTCG file that round-trips through Plumb can never
//!   smuggle in a non-hex string.
//!
//! [dtcg-spec]: https://design-tokens.github.io/community-group/format/
//! [`ColorSpec::tokens`]: plumb_core::config::ColorSpec::tokens
//! [`SpacingSpec::tokens`]: plumb_core::config::SpacingSpec::tokens
//! [`TypeScaleSpec::tokens`]: plumb_core::config::TypeScaleSpec::tokens
//! [`TypeScaleSpec::families`]: plumb_core::config::TypeScaleSpec::families
//! [`TypeScaleSpec::weights`]: plumb_core::config::TypeScaleSpec::weights
//! [`RadiusSpec::scale`]: plumb_core::config::RadiusSpec::scale

use std::collections::HashSet;
use std::path::PathBuf;

use indexmap::IndexMap;
use miette::NamedSource;
use plumb_core::Config;
use serde_json::Value;

use crate::ConfigError;
use crate::validate::is_valid_hex_color;

/// Maximum tolerated nesting depth in a DTCG document.
///
/// Picked to comfortably accommodate hand-authored token files (rarely
/// past a dozen levels) while bounding stack use on adversarial input.
pub const MAX_NESTING: usize = 256;

/// A DTCG document handed to [`merge_dtcg`].
///
/// Callers own I/O — `contents` should be the file bytes already read
/// from `path`. The path is used only to render diagnostics.
#[derive(Debug, Clone)]
pub struct DtcgSource {
    /// Filesystem path of the document, used for diagnostics.
    pub path: PathBuf,
    /// Document contents (UTF-8 JSON).
    pub contents: String,
}

/// Summary of what [`merge_dtcg`] inserted into the target [`Config`].
///
/// The fields below are counts of tokens that the call mutated into the
/// destination; tokens that were skipped (duplicates, unsupported
/// types, multi-mode siblings) are recorded in [`Self::warnings`].
#[derive(Debug, Default, Clone)]
pub struct DtcgImport {
    /// Number of color tokens added to [`plumb_core::config::ColorSpec::tokens`].
    pub color_added: usize,
    /// Number of spacing tokens added to [`plumb_core::config::SpacingSpec::tokens`].
    pub spacing_added: usize,
    /// Number of typography size tokens added to
    /// [`plumb_core::config::TypeScaleSpec::tokens`].
    pub type_size_added: usize,
    /// Number of font families added to
    /// [`plumb_core::config::TypeScaleSpec::families`].
    pub type_family_added: usize,
    /// Number of font weights added to
    /// [`plumb_core::config::TypeScaleSpec::weights`].
    pub type_weight_added: usize,
    /// Number of radius values added to
    /// [`plumb_core::config::RadiusSpec::scale`].
    pub radius_added: usize,
    /// Non-fatal issues discovered during the merge.
    pub warnings: Vec<DtcgWarning>,
}

/// A single non-fatal diagnostic raised during DTCG import.
#[derive(Debug, Clone)]
pub struct DtcgWarning {
    /// Slash-joined token path the warning concerns.
    pub path: String,
    /// What kind of issue triggered the warning.
    pub kind: DtcgWarningKind,
}

/// Reason a [`DtcgWarning`] was raised.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum DtcgWarningKind {
    /// `$type` value is recognized by DTCG but not currently mapped to
    /// any [`Config`] section (e.g. `shadow`, `duration`,
    /// `cubicBezier`). The token is dropped and the warning records
    /// the type so callers can surface it.
    UnsupportedType {
        /// The DTCG `$type` field, verbatim.
        ty: String,
    },
    /// A token already exists in [`Config`] under this name. The
    /// existing value is kept; the incoming value is dropped.
    DuplicateName {
        /// Token name as inserted (slash-joined path).
        name: String,
    },
    /// A `$extensions.modes` entry was found alongside the canonical
    /// `$value`. Plumb does not yet model design-token modes; the
    /// canonical `$value` is imported, the mode payloads are dropped.
    MultiMode {
        /// Mode name (e.g. `"dark"`, `"compact"`).
        mode: String,
    },
    /// The token's `$value` could not be coerced into the destination
    /// type (e.g. a `dimension` value that isn't expressible in pixels,
    /// a `fontFamily` that is neither a string nor an array of strings).
    /// The token is skipped and reported.
    Unconvertible {
        /// DTCG `$type` of the offending token.
        ty: String,
        /// Why the value could not be converted.
        reason: String,
    },
}

/// Merge a DTCG document into `into`.
///
/// Each token from the source replaces the corresponding `Config` slot
/// only when no conflicting entry exists. Skipped tokens, unsupported
/// types, and dropped multi-mode siblings are reported as
/// [`DtcgWarning`]s in [`DtcgImport::warnings`].
///
/// # Errors
///
/// * [`ConfigError::DtcgParse`] — JSON is malformed, exceeds
///   [`MAX_NESTING`], or fails type-specific validation (e.g. a color
///   `$value` that isn't a hex string).
/// * [`ConfigError::DtcgAlias`] — the document contains an alias cycle
///   or a dangling reference.
pub fn merge_dtcg(into: &mut Config, source: &DtcgSource) -> Result<DtcgImport, ConfigError> {
    let parsed: Value =
        serde_json::from_str(&source.contents).map_err(|e| ConfigError::DtcgParse {
            path: source.path.display().to_string(),
            source_code: Some(named_source(source)),
            span: None,
            reason: e.to_string(),
        })?;

    if !parsed.is_object() {
        return Err(parse_error(source, "root must be a JSON object"));
    }

    if exceeds_depth(&parsed, MAX_NESTING) {
        return Err(parse_error(
            source,
            &format!("token tree exceeds maximum nesting depth ({MAX_NESTING})"),
        ));
    }

    let mut import = DtcgImport::default();
    let mut tokens: IndexMap<String, RawToken> = IndexMap::new();
    collect_tokens(&parsed, &[], &mut tokens, &mut import.warnings);

    let mut resolved: IndexMap<String, ResolvedToken> = IndexMap::with_capacity(tokens.len());
    for (path, raw) in &tokens {
        let mut visiting: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let value = resolve_alias(path, &tokens, &mut visiting, &mut seen, source)?;
        resolved.insert(
            path.clone(),
            ResolvedToken {
                ty: raw.ty.clone(),
                value,
            },
        );
    }

    apply_resolved(into, &resolved, &mut import, source)?;
    Ok(import)
}

/// Internal — a parsed-but-unresolved token (its `$value` may still be
/// a `{path}` brace alias or a `{ "$ref": "#/..." }` pointer).
#[derive(Debug, Clone)]
struct RawToken {
    ty: String,
    value: Value,
}

#[derive(Debug, Clone)]
struct ResolvedToken {
    ty: String,
    value: Value,
}

/// Build a miette [`NamedSource`] from `source`. Centralized so future
/// span recovery only has to be wired up in one place.
fn named_source(source: &DtcgSource) -> NamedSource<String> {
    NamedSource::new(source.path.display().to_string(), source.contents.clone())
        .with_language("json")
}

fn parse_error(source: &DtcgSource, reason: &str) -> ConfigError {
    ConfigError::DtcgParse {
        path: source.path.display().to_string(),
        source_code: Some(named_source(source)),
        span: None,
        reason: reason.to_owned(),
    }
}

/// Conservative depth-bound check on the parsed JSON tree. We don't
/// rely on `serde_json`'s recursion limit because the public default
/// (128) is below our cap and not user-tunable per call.
fn exceeds_depth(value: &Value, limit: usize) -> bool {
    fn walk(value: &Value, depth: usize, limit: usize) -> bool {
        if depth > limit {
            return true;
        }
        match value {
            Value::Object(map) => map.values().any(|v| walk(v, depth + 1, limit)),
            Value::Array(items) => items.iter().any(|v| walk(v, depth + 1, limit)),
            _ => false,
        }
    }
    walk(value, 0, limit)
}

/// Walk the DTCG tree and emit one [`RawToken`] per token-bearing leaf
/// (anything with both `$type` and `$value` keys). Group metadata
/// (`$description`, `$extensions` at group level) is silently dropped;
/// per-token `$extensions.modes` is recorded as a warning.
fn collect_tokens(
    value: &Value,
    path: &[String],
    out: &mut IndexMap<String, RawToken>,
    warnings: &mut Vec<DtcgWarning>,
) {
    let Some(map) = value.as_object() else {
        return;
    };

    if map.contains_key("$type") && map.contains_key("$value") {
        let key = path.join("/");
        let ty = map
            .get("$type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let raw_value = map.get("$value").cloned().unwrap_or(Value::Null);

        // Emit one MultiMode warning per mode key. The canonical $value
        // still wins; this is a reporting hook only.
        if let Some(modes) = map
            .get("$extensions")
            .and_then(Value::as_object)
            .and_then(|ext| ext.get("modes"))
            .and_then(Value::as_object)
        {
            for mode_name in modes.keys() {
                warnings.push(DtcgWarning {
                    path: key.clone(),
                    kind: DtcgWarningKind::MultiMode {
                        mode: mode_name.clone(),
                    },
                });
            }
        }

        out.insert(
            key,
            RawToken {
                ty,
                value: raw_value,
            },
        );
        return;
    }

    for (k, v) in map {
        if k.starts_with('$') {
            continue;
        }
        let mut next = path.to_vec();
        next.push(k.clone());
        collect_tokens(v, &next, out, warnings);
    }
}

/// Resolve `$value` into a literal (non-alias) value.
///
/// Cycle detection: `seen` tracks paths currently on the resolution
/// stack; reentering a path raises a [`ConfigError::DtcgAlias`]
/// reporting the cycle in visit order. Single-element cycles encode
/// dangling references.
fn resolve_alias(
    path: &str,
    tokens: &IndexMap<String, RawToken>,
    visiting: &mut Vec<String>,
    seen: &mut HashSet<String>,
    source: &DtcgSource,
) -> Result<Value, ConfigError> {
    if seen.contains(path) {
        let mut cycle: Vec<String> = visiting.clone();
        cycle.push(path.to_owned());
        return Err(ConfigError::DtcgAlias {
            path: source.path.display().to_string(),
            source_code: Some(named_source(source)),
            cycle,
            reason: "alias cycle detected".to_owned(),
        });
    }

    let Some(token) = tokens.get(path) else {
        return Err(ConfigError::DtcgAlias {
            path: source.path.display().to_string(),
            source_code: Some(named_source(source)),
            cycle: vec![path.to_owned()],
            reason: format!("alias references unknown token `{path}`"),
        });
    };

    seen.insert(path.to_owned());
    visiting.push(path.to_owned());

    let resolved = if let Some(target) = parse_alias(&token.value) {
        resolve_alias(&target, tokens, visiting, seen, source)?
    } else if let Value::Object(map) = &token.value {
        // Object $value with embedded brace aliases anywhere inside
        // (e.g. composite shadow with `color: "{primitives.shadow}"`).
        // We resolve every string field shaped like `{x.y}` against the
        // token table so composites work too.
        let mut out = serde_json::Map::with_capacity(map.len());
        for (k, v) in map {
            out.insert(
                k.clone(),
                resolve_inline(v, tokens, visiting, seen, source)?,
            );
        }
        Value::Object(out)
    } else {
        token.value.clone()
    };

    visiting.pop();
    seen.remove(path);
    Ok(resolved)
}

/// Inline-resolve aliases inside an arbitrary `$value` payload. Used
/// for composite tokens (shadows, transitions). Returns the input
/// unchanged when no alias is present.
fn resolve_inline(
    value: &Value,
    tokens: &IndexMap<String, RawToken>,
    visiting: &mut Vec<String>,
    seen: &mut HashSet<String>,
    source: &DtcgSource,
) -> Result<Value, ConfigError> {
    if let Some(target) = parse_alias(value) {
        return resolve_alias(&target, tokens, visiting, seen, source);
    }
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                out.insert(
                    k.clone(),
                    resolve_inline(v, tokens, visiting, seen, source)?,
                );
            }
            Ok(Value::Object(out))
        }
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for v in items {
                out.push(resolve_inline(v, tokens, visiting, seen, source)?);
            }
            Ok(Value::Array(out))
        }
        other => Ok(other.clone()),
    }
}

/// Recognise a DTCG alias.
///
/// * `"{path.to.token}"` — brace shorthand. The dots in the source map
///   onto the slash-joined collection key.
/// * `{ "$ref": "#/path/to/token" }` — JSON-Pointer object form.
///
/// Returns the canonical slash-joined key, or `None` if `value` is a
/// literal.
fn parse_alias(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed.len() >= 2 {
                let inner = &trimmed[1..trimmed.len() - 1];
                // Reject empty, whitespace-only, or nested braces.
                if inner.is_empty() || inner.contains('{') || inner.contains('}') {
                    return None;
                }
                Some(inner.replace('.', "/"))
            } else {
                None
            }
        }
        Value::Object(map) => {
            let r = map.get("$ref").and_then(Value::as_str)?;
            // JSON Pointer must start with `#/`. Reject anything else.
            let pointer = r.strip_prefix("#/")?;
            if pointer.is_empty() {
                return None;
            }
            Some(pointer.to_owned())
        }
        _ => None,
    }
}

/// Apply the resolved tokens onto `into`. This is the only place that
/// reaches into [`Config`]'s sections; the rest of the module is type-
/// agnostic.
fn apply_resolved(
    into: &mut Config,
    resolved: &IndexMap<String, ResolvedToken>,
    import: &mut DtcgImport,
    source: &DtcgSource,
) -> Result<(), ConfigError> {
    for (path, token) in resolved {
        match token.ty.as_str() {
            "color" => apply_color(into, path, &token.value, import, source)?,
            "dimension" => apply_dimension(into, path, &token.value, import, source)?,
            "fontFamily" => apply_font_family(into, path, &token.value, import),
            "fontWeight" => apply_font_weight(into, path, &token.value, import),
            "radius" | "borderRadius" => apply_radius(into, path, &token.value, import, source)?,
            "" => {
                // No `$type` — DTCG allows this if a parent group
                // declares `$type`, but Plumb doesn't track group-level
                // defaults yet. Surface as a typed warning.
                import.warnings.push(DtcgWarning {
                    path: path.clone(),
                    kind: DtcgWarningKind::UnsupportedType {
                        ty: "<missing>".to_owned(),
                    },
                });
            }
            other => {
                import.warnings.push(DtcgWarning {
                    path: path.clone(),
                    kind: DtcgWarningKind::UnsupportedType {
                        ty: other.to_owned(),
                    },
                });
            }
        }
    }
    Ok(())
}

fn apply_color(
    into: &mut Config,
    path: &str,
    value: &Value,
    import: &mut DtcgImport,
    source: &DtcgSource,
) -> Result<(), ConfigError> {
    let Some(s) = value.as_str() else {
        return Err(ConfigError::DtcgParse {
            path: source.path.display().to_string(),
            source_code: Some(named_source(source)),
            span: None,
            reason: format!(
                "color token `{path}` $value must be a hex string, got {kind}",
                kind = value_kind(value)
            ),
        });
    };
    if !is_valid_hex_color(s) {
        return Err(ConfigError::DtcgParse {
            path: source.path.display().to_string(),
            source_code: Some(named_source(source)),
            span: None,
            reason: format!(
                "color token `{path}` $value `{s}` is not a valid hex (#rgb, #rgba, #rrggbb, or #rrggbbaa)"
            ),
        });
    }
    if into.color.tokens.contains_key(path) {
        import.warnings.push(DtcgWarning {
            path: path.to_owned(),
            kind: DtcgWarningKind::DuplicateName {
                name: path.to_owned(),
            },
        });
        return Ok(());
    }
    into.color.tokens.insert(path.to_owned(), s.to_owned());
    import.color_added += 1;
    Ok(())
}

fn apply_dimension(
    into: &mut Config,
    path: &str,
    value: &Value,
    import: &mut DtcgImport,
    source: &DtcgSource,
) -> Result<(), ConfigError> {
    let pixels = match dimension_to_pixels(value) {
        Ok(px) => px,
        Err(reason) => {
            import.warnings.push(DtcgWarning {
                path: path.to_owned(),
                kind: DtcgWarningKind::Unconvertible {
                    ty: "dimension".to_owned(),
                    reason,
                },
            });
            return Ok(());
        }
    };

    if pixels.is_sign_negative() || !pixels.is_finite() {
        return Err(ConfigError::DtcgParse {
            path: source.path.display().to_string(),
            source_code: Some(named_source(source)),
            span: None,
            reason: format!(
                "dimension token `{path}` resolves to a non-finite or negative pixel value"
            ),
        });
    }

    // We accept fractional inputs (sub-pixel typography is real) but
    // round to `u32` because every Plumb spec slot is integer-pixel.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let px = pixels.round() as u32;

    if dimension_is_typography(path) {
        if into.type_scale.tokens.contains_key(path) {
            import.warnings.push(DtcgWarning {
                path: path.to_owned(),
                kind: DtcgWarningKind::DuplicateName {
                    name: path.to_owned(),
                },
            });
            return Ok(());
        }
        into.type_scale.tokens.insert(path.to_owned(), px);
        import.type_size_added += 1;
    } else {
        if into.spacing.tokens.contains_key(path) {
            import.warnings.push(DtcgWarning {
                path: path.to_owned(),
                kind: DtcgWarningKind::DuplicateName {
                    name: path.to_owned(),
                },
            });
            return Ok(());
        }
        into.spacing.tokens.insert(path.to_owned(), px);
        import.spacing_added += 1;
    }
    Ok(())
}

/// Heuristic: a dimension token is treated as typography when any path
/// segment matches one of these typography-flavored names.
///
/// This mirrors the convention Tokens Studio and Style Dictionary use
/// out of the box. Tokens whose path doesn't match the typography list
/// fall through to spacing.
fn dimension_is_typography(path: &str) -> bool {
    const TYPE_KEYS: &[&str] = &[
        "typography",
        "type",
        "font-size",
        "fontsize",
        "font_size",
        "text",
        "font",
        "size",
    ];
    path.split('/').any(|seg| {
        let normalized = seg.to_ascii_lowercase();
        TYPE_KEYS.iter().any(|k| normalized == *k)
    })
}

fn dimension_to_pixels(value: &Value) -> Result<f64, String> {
    match value {
        Value::Number(n) => n.as_f64().ok_or_else(|| "non-finite number".to_owned()),
        Value::String(s) => parse_dimension_string(s),
        Value::Object(map) => {
            let v = map
                .get("value")
                .ok_or_else(|| "object dimension missing `value`".to_owned())?;
            let unit = map.get("unit").and_then(Value::as_str).unwrap_or("px");
            if !unit_is_px(unit) {
                return Err(format!("unsupported dimension unit `{unit}`"));
            }
            v.as_f64()
                .ok_or_else(|| "object dimension `value` must be a number".to_owned())
        }
        other => Err(format!(
            "unsupported dimension shape: {}",
            value_kind(other)
        )),
    }
}

fn parse_dimension_string(s: &str) -> Result<f64, String> {
    let trimmed = s.trim();
    let (num, unit) = if let Some(rest) = trimmed.strip_suffix("px") {
        (rest.trim(), "px")
    } else if let Some(rest) = trimmed.strip_suffix("rem") {
        (rest.trim(), "rem")
    } else if let Some(rest) = trimmed.strip_suffix("em") {
        (rest.trim(), "em")
    } else {
        (trimmed, "")
    };
    if !unit_is_px(unit) {
        return Err(format!("unsupported dimension unit `{unit}`"));
    }
    num.parse::<f64>()
        .map_err(|e| format!("dimension `{s}`: {e}"))
}

fn unit_is_px(unit: &str) -> bool {
    matches!(unit, "" | "px")
}

fn apply_font_family(into: &mut Config, path: &str, value: &Value, import: &mut DtcgImport) {
    let families = match value {
        Value::String(s) => vec![s.clone()],
        Value::Array(items) => items
            .iter()
            .filter_map(|v| v.as_str().map(ToOwned::to_owned))
            .collect(),
        _ => {
            import.warnings.push(DtcgWarning {
                path: path.to_owned(),
                kind: DtcgWarningKind::Unconvertible {
                    ty: "fontFamily".to_owned(),
                    reason: format!(
                        "fontFamily $value must be a string or array of strings, got {}",
                        value_kind(value)
                    ),
                },
            });
            return;
        }
    };
    for fam in families {
        if !into.type_scale.families.iter().any(|f| f == &fam) {
            into.type_scale.families.push(fam);
            import.type_family_added += 1;
        }
    }
}

fn apply_font_weight(into: &mut Config, path: &str, value: &Value, import: &mut DtcgImport) {
    let weight = match value {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => match s.trim() {
            // DTCG allows the named weights from CSS spec.
            "thin" | "hairline" => Some(100),
            "extra-light" | "extralight" | "ultralight" => Some(200),
            "light" => Some(300),
            "regular" | "normal" => Some(400),
            "medium" => Some(500),
            "semi-bold" | "semibold" | "demibold" => Some(600),
            "bold" => Some(700),
            "extra-bold" | "extrabold" | "ultrabold" => Some(800),
            "black" | "heavy" => Some(900),
            other => other.parse::<u64>().ok(),
        },
        _ => None,
    };
    let Some(w) = weight else {
        import.warnings.push(DtcgWarning {
            path: path.to_owned(),
            kind: DtcgWarningKind::Unconvertible {
                ty: "fontWeight".to_owned(),
                reason: format!(
                    "fontWeight $value must be a number or named weight, got {}",
                    value_kind(value)
                ),
            },
        });
        return;
    };
    let Ok(w16) = u16::try_from(w) else {
        import.warnings.push(DtcgWarning {
            path: path.to_owned(),
            kind: DtcgWarningKind::Unconvertible {
                ty: "fontWeight".to_owned(),
                reason: format!("fontWeight `{w}` does not fit in u16"),
            },
        });
        return;
    };
    if !into.type_scale.weights.contains(&w16) {
        into.type_scale.weights.push(w16);
        import.type_weight_added += 1;
    }
}

fn apply_radius(
    into: &mut Config,
    path: &str,
    value: &Value,
    import: &mut DtcgImport,
    source: &DtcgSource,
) -> Result<(), ConfigError> {
    let pixels = match dimension_to_pixels(value) {
        Ok(px) => px,
        Err(reason) => {
            import.warnings.push(DtcgWarning {
                path: path.to_owned(),
                kind: DtcgWarningKind::Unconvertible {
                    ty: "borderRadius".to_owned(),
                    reason,
                },
            });
            return Ok(());
        }
    };
    if pixels.is_sign_negative() || !pixels.is_finite() {
        return Err(ConfigError::DtcgParse {
            path: source.path.display().to_string(),
            source_code: Some(named_source(source)),
            span: None,
            reason: format!(
                "radius token `{path}` resolves to a non-finite or negative pixel value"
            ),
        });
    }
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let px = pixels.round() as u32;
    if !into.radius.scale.contains(&px) {
        into.radius.scale.push(px);
        import.radius_added += 1;
    }
    Ok(())
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_alias_brace_form() {
        assert_eq!(
            parse_alias(&Value::String("{a.b.c}".to_owned())),
            Some("a/b/c".to_owned())
        );
    }

    #[test]
    fn parse_alias_ref_form() {
        let v = serde_json::json!({ "$ref": "#/a/b" });
        assert_eq!(parse_alias(&v), Some("a/b".to_owned()));
    }

    #[test]
    fn parse_alias_rejects_garbage() {
        assert_eq!(parse_alias(&Value::String("plain".to_owned())), None);
        assert_eq!(parse_alias(&Value::String("{}".to_owned())), None);
        assert_eq!(parse_alias(&Value::String("{nested{x}}".to_owned())), None);
        let v = serde_json::json!({ "$ref": "../escape" });
        assert_eq!(parse_alias(&v), None);
    }

    #[test]
    fn dimension_pixels_object_form() {
        let v = serde_json::json!({ "value": 12, "unit": "px" });
        assert!((dimension_to_pixels(&v).expect("ok") - 12.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dimension_pixels_string_form() {
        assert!((parse_dimension_string("16px").expect("ok") - 16.0).abs() < f64::EPSILON);
        assert!((parse_dimension_string("8").expect("ok") - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dimension_rejects_non_px_units() {
        assert!(parse_dimension_string("1.5rem").is_err());
        assert!(parse_dimension_string("2em").is_err());
    }

    #[test]
    fn dimension_typography_heuristic() {
        assert!(dimension_is_typography("typography/size/body"));
        assert!(dimension_is_typography("type/heading"));
        assert!(dimension_is_typography("font-size/lg"));
        assert!(dimension_is_typography("text/body"));
        assert!(!dimension_is_typography("spacing/md"));
        assert!(!dimension_is_typography("gap/xl"));
        assert!(!dimension_is_typography("layout/gutter"));
    }

    #[test]
    fn depth_check_flags_overflow() {
        let mut v = Value::Null;
        for _ in 0..(MAX_NESTING + 5) {
            let mut m = serde_json::Map::new();
            m.insert("g".to_owned(), v);
            v = Value::Object(m);
        }
        assert!(exceeds_depth(&v, MAX_NESTING));
    }

    #[test]
    fn depth_check_passes_under_limit() {
        let mut v = Value::Null;
        for _ in 0..32 {
            let mut m = serde_json::Map::new();
            m.insert("g".to_owned(), v);
            v = Value::Object(m);
        }
        assert!(!exceeds_depth(&v, MAX_NESTING));
    }
}
