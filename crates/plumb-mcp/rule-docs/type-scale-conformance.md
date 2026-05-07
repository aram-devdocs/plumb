# type/scale-conformance

**Status:** active

**Default severity:** `warning`

## What it checks

For every node in the snapshot, the rule reads the computed `font-size`
value and parses it as a CSS pixel length. A violation fires when the
parsed pixel value is not within `0.5px` of any element of
`type.scale`. Subpixel rounding from `getComputedStyle` (e.g.
`16.4px`) matches the intended scale value (`16`) without admitting
truly off-scale values.

The rule MUST skip a node when:

- the computed `font-size` value parses as `auto`, `normal`, the empty
  string, `calc(...)`, `<n>em`, `<n>rem`, `<n>%`, or any other
  non-`px` shape;
- or `type.scale` is empty (the rule is a no-op in that case rather
  than flagging every value as out-of-scale).

The rule reads only `font-size`. It does not look at `font-family`,
`font-weight`, or `line-height` — those are covered by sibling rules
that will land in subsequent commits.

## Why it matters

A type scale is the design system's vocabulary for text size. Off-scale
font-size values introduce visual jitter — body copy at 15px next to
button text at 14px next to caption text at 13.5px reads as drift,
not hierarchy. This rule is the deterministic check that protects the
vocabulary.

## Example violation

```json
{
  "rule_id": "type/scale-conformance",
  "severity": "warning",
  "message": "`html > body > div:nth-child(2)` has off-scale font-size 15px; expected a value from type.scale.",
  "selector": "html > body > div:nth-child(2)",
  "viewport": "desktop",
  "dom_order": 3,
  "fix": {
    "kind": {
      "kind": "css_property_replace",
      "property": "font-size",
      "from": "15px",
      "to": "14px"
    },
    "description": "Snap `font-size` to the nearest type-scale value (14px).",
    "confidence": "medium"
  },
  "doc_url": "https://plumb.aramhammoudeh.com/rules/type-scale-conformance"
}
```

## Configuration

`type.scale` is the list of allowed font-size values in pixels. Default
is empty (the rule is a no-op).

```toml
[type]
scale = [12, 14, 16, 18, 20, 24, 30, 36, 48]
```

The rule reads `config.type_scale.scale` once per run and picks the
nearest member by absolute delta when emitting a fix. Ties resolve
to the lower scale value.

## Suppression

Disable the rule for an entire run:

```toml
[rules."type/scale-conformance"]
enabled = false
```

Bump or lower the severity:

```toml
[rules."type/scale-conformance"]
severity = "info"
```

`RuleOverride` accepts both `enabled` (default `true`) and an optional
`severity` of `info`, `warning`, or `error`. Severity remapping is
applied at the formatter layer.

## See also

- [`spacing/scale-conformance`](./spacing-scale-conformance.md) — the
  sibling rule for spacing tokens.
- [`spacing/grid-conformance`](./spacing-grid-conformance.md) — the
  base-unit check for spacing values.
- PRD §11.3 — type rules.
