# radius/scale-conformance

**Status:** active

**Default severity:** `warning`

## What it checks

For every node in the snapshot, the rule reads each of these computed
styles and parses the value as a CSS pixel length:

- `border-top-left-radius`
- `border-top-right-radius`
- `border-bottom-right-radius`
- `border-bottom-left-radius`

A property fires a violation when the parsed pixel value is not within
`0.5px` of any element of `radius.scale`. The tolerance lets subpixel
rounding from `getComputedStyle` (e.g. `4.4px`) match the intended
scale value (`4`) without admitting truly off-scale values.

The rule MUST skip a property when:

- the computed value parses as `auto`, the empty string, `calc(...)`,
  `<n>em`, `<n>rem`, `<n>%`, or any other non-`px` shape;
- or `radius.scale` is empty (the rule is a no-op in that case rather
  than flagging every value as out-of-scale).

At most one violation is emitted per `(selector, property)` pair.

The `border-radius` shorthand is deliberately excluded — the Chromium
driver returns longhands per PRD §10.3, and checking both shapes would
emit two violations for the same logical issue.

## Why it matters

A discrete radius scale is the design system's vocabulary for corner
softness. Ad-hoc radii — a stray `5px` here, a `13px` there — drift
the system's identity card by card. This rule is the deterministic
check that keeps the vocabulary tight.

It is symmetric with
[`spacing/scale-conformance`](./spacing-scale-conformance.md): the two
rules share the same scale-membership and tie-break rules, so the
nearest-in-scale fix lines up with what authors already see for
spacing. Both fixes are emitted at `confidence: medium`.

## Example violation

```json
{
  "rule_id": "radius/scale-conformance",
  "severity": "warning",
  "message": "`html > body > div:nth-child(2)` has off-scale border-top-left-radius 5px; expected a value from radius.scale.",
  "selector": "html > body > div:nth-child(2)",
  "viewport": "desktop",
  "dom_order": 3,
  "fix": {
    "kind": {
      "kind": "css_property_replace",
      "property": "border-top-left-radius",
      "from": "5px",
      "to": "4px"
    },
    "description": "Snap `border-top-left-radius` to the nearest radius-scale value (4px).",
    "confidence": "medium"
  },
  "doc_url": "https://plumb.aramhammoudeh.com/rules/radius-scale-conformance"
}
```

## Configuration

`radius.scale` is the list of allowed pixel values. Default is empty
(the rule is a no-op).

```toml
[radius]
scale = [0, 4, 8, 12, 16, 24]
```

The rule reads `config.radius.scale` once per run and picks the
nearest member by absolute delta when emitting a fix. Ties resolve to
the lower scale value.

## Suppression

Disable the rule for an entire run:

```toml
[rules."radius/scale-conformance"]
enabled = false
```

Bump or lower the severity:

```toml
[rules."radius/scale-conformance"]
severity = "info"
```

`RuleOverride` accepts both `enabled` (default `true`) and an optional
`severity` of `info`, `warning`, or `error`. Severity remapping is
applied at the formatter layer.

## See also

- [`spacing/scale-conformance`](./spacing-scale-conformance.md) — the
  symmetric rule for margin / padding / gap.
- [`type/scale-conformance`](./type-scale-conformance.md) — the
  symmetric rule for `font-size`.
- PRD §11.3 — the radius spec.
