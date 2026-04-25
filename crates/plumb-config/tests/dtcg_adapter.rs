//! DTCG 2025.10 adapter integration tests.
//!
//! Each fixture under `tests/fixtures/dtcg/` exercises one of the
//! contract surfaces documented in `crates/plumb-config/src/dtcg.rs`:
//!
//! * `flat-palette.json` — round-trip a flat color palette into
//!   [`plumb_core::ColorSpec::tokens`].
//! * `nested-aliases.json` — nested groups with `{path.to.token}`
//!   aliases across colors, spacing, typography, and radius.
//! * `multi-mode.json` — DTCG `$extensions.modes` payload; default
//!   mode maps; additional modes surface as warnings.

#![allow(clippy::expect_used)]

use std::path::PathBuf;

use plumb_config::{ConfigError, DtcgSource, DtcgWarningKind, merge_dtcg};
use plumb_core::Config;

fn fixture(name: &str) -> DtcgSource {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "tests",
        "fixtures",
        "dtcg",
        name,
    ]
    .iter()
    .collect();
    let contents = std::fs::read_to_string(&path).expect("read fixture");
    DtcgSource { path, contents }
}

#[test]
fn flat_palette_round_trip() {
    let mut cfg = Config::default();
    let source = fixture("flat-palette.json");

    let report = merge_dtcg(&mut cfg, &source).expect("merge flat palette");

    assert_eq!(report.color_added, 4);
    assert_eq!(cfg.color.tokens.len(), 4);
    assert_eq!(cfg.color.tokens["brand-primary"], "#0b7285");
    assert_eq!(cfg.color.tokens["brand-secondary"], "#1971c2");
    assert_eq!(cfg.color.tokens["neutral-bg"], "#ffffff");
    assert_eq!(cfg.color.tokens["neutral-fg"], "#0b0b0b");

    // No unmapped types in this fixture → no warnings.
    assert!(
        report.warnings.is_empty(),
        "flat palette should not warn: {:?}",
        report.warnings
    );
}

#[test]
fn flat_palette_round_trip_preserves_insertion_order() {
    let mut cfg = Config::default();
    let source = fixture("flat-palette.json");

    merge_dtcg(&mut cfg, &source).expect("merge flat palette");

    let keys: Vec<&str> = cfg.color.tokens.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        vec![
            "brand-primary",
            "brand-secondary",
            "neutral-bg",
            "neutral-fg"
        ]
    );
}

#[test]
fn nested_group_with_aliases_resolves() {
    let mut cfg = Config::default();
    let source = fixture("nested-aliases.json");

    let report = merge_dtcg(&mut cfg, &source).expect("merge nested fixture");

    // 3 primitives + 3 semantic aliases = 6 colors.
    assert_eq!(report.color_added, 6);
    assert_eq!(cfg.color.tokens["color/primitive/blue-500"], "#1971c2");
    assert_eq!(cfg.color.tokens["color/primitive/gray-50"], "#f8f9fa");
    assert_eq!(cfg.color.tokens["color/primitive/gray-900"], "#0b0b0b");
    // Aliased semantic colors resolve to the primitive hex.
    assert_eq!(cfg.color.tokens["color/semantic/bg/canvas"], "#f8f9fa");
    assert_eq!(cfg.color.tokens["color/semantic/fg/primary"], "#0b0b0b");
    assert_eq!(cfg.color.tokens["color/semantic/accent/brand"], "#1971c2");

    // Spacing dimensions: unit, sm, md, lg (lg aliases md → 16).
    assert_eq!(report.spacing_added, 4);
    assert_eq!(cfg.spacing.tokens["spacing/unit"], 4);
    assert_eq!(cfg.spacing.tokens["spacing/sm"], 8);
    assert_eq!(cfg.spacing.tokens["spacing/md"], 16);
    assert_eq!(cfg.spacing.tokens["spacing/lg"], 16);

    // Typography sizes go to TypeScaleSpec.tokens via the namespace heuristic.
    assert_eq!(report.type_size_added, 2);
    assert_eq!(cfg.type_scale.tokens["typography/size/body"], 16);
    assert_eq!(cfg.type_scale.tokens["typography/size/heading"], 24);

    // Families and weights.
    assert_eq!(report.type_family_added, 3);
    assert!(cfg.type_scale.families.contains(&"Inter".to_owned()));
    assert!(
        cfg.type_scale
            .families
            .contains(&"JetBrains Mono".to_owned())
    );
    assert!(cfg.type_scale.families.contains(&"ui-monospace".to_owned()));

    assert_eq!(report.type_weight_added, 2);
    assert!(cfg.type_scale.weights.contains(&400));
    assert!(cfg.type_scale.weights.contains(&700));

    // Radius — both `borderRadius` and `radius` $type values land here.
    assert_eq!(report.radius_added, 3);
    assert!(cfg.radius.scale.contains(&4));
    assert!(cfg.radius.scale.contains(&8));
    assert!(cfg.radius.scale.contains(&16));

    // Shadow tokens are unmapped → one warning, not a hard error.
    assert!(
        report
            .warnings
            .iter()
            .any(|w| matches!(w.kind, DtcgWarningKind::UnsupportedType { .. })),
        "shadow should surface as an unsupported-type warning"
    );
}

#[test]
fn cycle_in_aliases_returns_typed_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("cycle.json");
    let body = r#"
    {
      "a": { "$type": "color", "$value": "{b}" },
      "b": { "$type": "color", "$value": "{a}" }
    }
    "#;
    std::fs::write(&path, body).expect("write fixture");
    let source = DtcgSource {
        path,
        contents: body.to_owned(),
    };

    let mut cfg = Config::default();
    let err = merge_dtcg(&mut cfg, &source).expect_err("cycle should fail");

    match err {
        ConfigError::DtcgAlias { ref cycle, .. } => {
            assert!(
                cycle.iter().any(|s| s == "a") && cycle.iter().any(|s| s == "b"),
                "cycle should mention both nodes, got {cycle:?}"
            );
        }
        other => panic!("expected DtcgAlias, got {other:?}"),
    }
}

#[test]
fn unresolved_alias_returns_typed_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("dangling.json");
    let body = r#"
    {
      "fg": { "$type": "color", "$value": "{missing.token}" }
    }
    "#;
    std::fs::write(&path, body).expect("write fixture");
    let source = DtcgSource {
        path,
        contents: body.to_owned(),
    };

    let mut cfg = Config::default();
    let err = merge_dtcg(&mut cfg, &source).expect_err("dangling alias should fail");

    assert!(
        matches!(err, ConfigError::DtcgAlias { .. }),
        "expected DtcgAlias for dangling reference, got {err:?}"
    );
}

#[test]
fn multi_mode_export_uses_default_value_and_warns() {
    let mut cfg = Config::default();
    let source = fixture("multi-mode.json");

    let report = merge_dtcg(&mut cfg, &source).expect("merge multi-mode fixture");

    // Default `$value` is what gets imported; mode payloads are surfaced
    // as MultiMode warnings instead of overwriting the canonical value.
    assert_eq!(cfg.color.tokens["color/bg"], "#ffffff");
    assert_eq!(cfg.color.tokens["color/fg"], "#0b0b0b");
    assert_eq!(cfg.spacing.tokens["spacing/default"], 16);

    let multi_mode_warnings = report
        .warnings
        .iter()
        .filter(|w| matches!(w.kind, DtcgWarningKind::MultiMode { .. }))
        .count();
    assert!(
        multi_mode_warnings >= 3,
        "expected at least 3 multi-mode warnings, got {multi_mode_warnings}"
    );
}

#[test]
fn unsupported_type_surfaces_warning_not_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("unsupported.json");
    let body = r##"
    {
      "duration": {
        "$type": "duration",
        "$value": "200ms"
      },
      "ok": {
        "$type": "color",
        "$value": "#0b7285"
      }
    }
    "##;
    std::fs::write(&path, body).expect("write fixture");
    let source = DtcgSource {
        path,
        contents: body.to_owned(),
    };

    let mut cfg = Config::default();
    let report = merge_dtcg(&mut cfg, &source).expect("unsupported should warn, not fail");

    assert_eq!(report.color_added, 1);
    assert_eq!(cfg.color.tokens["ok"], "#0b7285");
    assert!(
        report.warnings.iter().any(|w| matches!(
            &w.kind,
            DtcgWarningKind::UnsupportedType { ty } if ty == "duration"
        )),
        "duration should produce an UnsupportedType warning"
    );
}

#[test]
fn malformed_json_returns_dtcg_parse_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("bad.json");
    let body = "{ not valid json";
    std::fs::write(&path, body).expect("write fixture");
    let source = DtcgSource {
        path,
        contents: body.to_owned(),
    };

    let mut cfg = Config::default();
    let err = merge_dtcg(&mut cfg, &source).expect_err("malformed json should fail");

    assert!(
        matches!(err, ConfigError::DtcgParse { .. }),
        "expected DtcgParse error, got {err:?}"
    );
}

#[test]
fn rejects_invalid_hex_color() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("badhex.json");
    let body = r#"
    {
      "bad": { "$type": "color", "$value": "purple" }
    }
    "#;
    std::fs::write(&path, body).expect("write fixture");
    let source = DtcgSource {
        path,
        contents: body.to_owned(),
    };

    let mut cfg = Config::default();
    let err = merge_dtcg(&mut cfg, &source).expect_err("non-hex color should fail");

    assert!(
        matches!(err, ConfigError::DtcgParse { .. }),
        "expected DtcgParse for invalid hex, got {err:?}"
    );
}

#[test]
fn deeply_nested_input_is_rejected() {
    // 300 levels of nesting — well over both Plumb's 64 cap and
    // serde_json's default 128 recursion limit. Either layer of defense
    // is enough to surface the failure as a `DtcgParse`.
    let mut body = String::new();
    for _ in 0..300 {
        body.push_str("{\"g\":");
    }
    body.push_str("{\"$type\":\"color\",\"$value\":\"#000000\"}");
    for _ in 0..300 {
        body.push('}');
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("deep.json");
    std::fs::write(&path, &body).expect("write fixture");
    let source = DtcgSource {
        path,
        contents: body,
    };

    let mut cfg = Config::default();
    let err = merge_dtcg(&mut cfg, &source).expect_err("deep nesting should fail");
    assert!(
        matches!(err, ConfigError::DtcgParse { .. }),
        "expected DtcgParse for deep nesting, got {err:?}"
    );
}

#[test]
fn refs_style_alias_resolves() {
    // DTCG drafts also accept JSON-Pointer-style $ref alongside the brace
    // shorthand. Both forms must resolve.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("ref.json");
    let body = r##"
    {
      "primitives": {
        "blue": { "$type": "color", "$value": "#1971c2" }
      },
      "semantic": {
        "accent": {
          "$type": "color",
          "$value": { "$ref": "#/primitives/blue" }
        }
      }
    }
    "##;
    std::fs::write(&path, body).expect("write fixture");
    let source = DtcgSource {
        path,
        contents: body.to_owned(),
    };

    let mut cfg = Config::default();
    let report = merge_dtcg(&mut cfg, &source).expect("ref-style alias should resolve");

    assert_eq!(report.color_added, 2);
    assert_eq!(cfg.color.tokens["semantic/accent"], "#1971c2");
}

#[test]
fn duplicate_token_name_warns_and_keeps_first() {
    // Same flat key appears twice via different groups → second is a duplicate.
    let mut cfg = Config::default();
    cfg.color
        .tokens
        .insert("brand".to_owned(), "#abcdef".to_owned());

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("dup.json");
    let body = r##"
    {
      "brand": { "$type": "color", "$value": "#0b7285" }
    }
    "##;
    std::fs::write(&path, body).expect("write fixture");
    let source = DtcgSource {
        path,
        contents: body.to_owned(),
    };

    let report = merge_dtcg(&mut cfg, &source).expect("duplicate should warn, not fail");

    // First wins — the existing config value is preserved.
    assert_eq!(cfg.color.tokens["brand"], "#abcdef");
    assert!(
        report
            .warnings
            .iter()
            .any(|w| matches!(&w.kind, DtcgWarningKind::DuplicateName) && w.path == "brand"),
        "duplicate token should surface as a DuplicateName warning at path `brand`"
    );
}

#[test]
fn nesting_above_plumb_cap_is_rejected_by_dtcg_check() {
    // 100 levels of nesting — above Plumb's 64 cap, below serde_json's
    // default 128 recursion limit. This exercises the `exceeds_depth`
    // check directly (the deeper-nesting test relies on serde_json's
    // recursion guard firing first).
    let mut body = String::new();
    for _ in 0..100 {
        body.push_str("{\"g\":");
    }
    body.push_str("{\"$type\":\"color\",\"$value\":\"#000000\"}");
    for _ in 0..100 {
        body.push('}');
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("plumb-cap.json");
    std::fs::write(&path, &body).expect("write fixture");
    let source = DtcgSource {
        path,
        contents: body,
    };

    let mut cfg = Config::default();
    let err = merge_dtcg(&mut cfg, &source).expect_err("over-cap nesting should fail");
    match err {
        ConfigError::DtcgParse { reason, .. } => {
            assert!(
                reason.contains("exceeds maximum nesting depth (64)"),
                "expected Plumb cap message, got reason: {reason}"
            );
        }
        other => panic!("expected DtcgParse, got {other:?}"),
    }
}

#[test]
fn radius_unconvertible_warning_uses_actual_type() {
    // A `$type: "radius"` token whose `$value` cannot be coerced into
    // pixels should report `ty: "radius"` — not the previously-hardcoded
    // `"borderRadius"`.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("radius-unconvertible.json");
    let body = r#"
    {
      "rad": { "$type": "radius", "$value": "1.5em" }
    }
    "#;
    std::fs::write(&path, body).expect("write fixture");
    let source = DtcgSource {
        path,
        contents: body.to_owned(),
    };

    let mut cfg = Config::default();
    let report = merge_dtcg(&mut cfg, &source).expect("unconvertible should warn, not fail");

    assert_eq!(report.radius_added, 0);
    assert!(
        report.warnings.iter().any(|w| matches!(
            &w.kind,
            DtcgWarningKind::Unconvertible { ty, .. } if ty == "radius"
        )),
        "radius-typed unconvertible should report ty=`radius`, got {:?}",
        report.warnings
    );
}
