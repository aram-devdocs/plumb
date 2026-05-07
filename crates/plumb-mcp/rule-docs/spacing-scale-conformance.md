# spacing/scale-conformance

**Status:** active

**Default severity:** `warning`

## What it checks

For every node in the snapshot, the rule reads each of these computed
styles and parses the value as a CSS pixel length:

- `margin-top`, `margin-right`, `margin-bottom`, `margin-left`
- `padding-top`, `padding-right`, `padding-bottom`, `padding-left`
- `gap`, `row-gap`, `column-gap`

A property fires a violation when the parsed pixel value is not
within `0.5px` of any element of `spacing.scale`. The tolerance lets
subpixel rounding from `getComputedStyle` (e.g. `12.4px`) match the
intended scale value (`12`) without admitting truly off-scale values.

The rule MUST skip a property when:

- the computed value parses as `auto`, `normal`, the empty string,
  `calc(...)`, `<n>em`, `<n>rem`, `<n>%`, or any other non-`px` shape;
- or `spacing.scale` is empty (the rule is a no-op in that case rather
  than flagging every value as out-of-scale).

At most one violation is emitted per `(selector, property)` pair.

The `margin` and `padding` shorthands are deliberately excluded — the
Chromium driver returns longhands per PRD §10.3, and checking both
shapes would emit two violations for the same logical issue.

## Why it matters

A discrete spacing scale is the design system's vocabulary for
distance. Off-scale values introduce vocabulary the system did not
sanction — three pixels of margin here, twenty over there, "close
enough" everywhere — and the cumulative drift makes future template
work harder. This rule is the deterministic check that protects the
vocabulary.

It complements
[`spacing/grid-conformance`](./spacing-grid-conformance.md): a value
like `20px` is on-grid against `base_unit = 4` but off-scale against
`scale = [0, 4, 8, 12, 16, 24, 32, 48]`. Both rules can fire on the
same property; both fixes are emitted at `confidence: medium`.

## Example violation

```json
{
  "rule_id": "spacing/scale-conformance",
  "severity": "warning",
  "message": "`html > body > div:nth-child(2)` has off-scale margin-right 20px; expected a value from spacing.scale.",
  "selector": "html > body > div:nth-child(2)",
  "viewport": "desktop",
  "dom_order": 3,
  "fix": {
    "kind": {
      "kind": "css_property_replace",
      "property": "margin-right",
      "from": "20px",
      "to": "16px"
    },
    "description": "Snap `margin-right` to the nearest spacing-scale value (16px).",
    "confidence": "medium"
  },
  "doc_url": "https://plumb.aramhammoudeh.com/rules/spacing-scale-conformance"
}
```

## Configuration

`spacing.scale` is the list of allowed pixel values. Default is empty
(the rule is a no-op).

```toml
[spacing]
scale = [0, 4, 8, 12, 16, 24, 32, 48]
```

The rule reads `config.spacing.scale` once per run and picks the
nearest member by absolute delta when emitting a fix. Ties resolve
to the lower scale value.

## Suppression

Disable the rule for an entire run:

```toml
[rules."spacing/scale-conformance"]
enabled = false
```

Bump or lower the severity:

```toml
[rules."spacing/scale-conformance"]
severity = "info"
```

`RuleOverride` accepts both `enabled` (default `true`) and an optional
`severity` of `info`, `warning`, or `error`. Severity remapping is
applied at the formatter layer.

## See also

- [`spacing/grid-conformance`](./spacing-grid-conformance.md) — the
  base-unit sibling check.
- PRD §11.2 — spacing rules and the token model.
