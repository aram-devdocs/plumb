# spacing/grid-conformance

**Status:** active

**Default severity:** `warning`

## What it checks

For every node in the snapshot, the rule reads each of these computed
styles and parses the value as a CSS pixel length:

- `margin-top`, `margin-right`, `margin-bottom`, `margin-left`
- `padding-top`, `padding-right`, `padding-bottom`, `padding-left`
- `gap`, `row-gap`, `column-gap`

A property fires a violation when the parsed value is not a multiple
of `spacing.base_unit`. The check tolerates a `1e-6` floating-point
residue so subpixel rounding from `getComputedStyle` does not produce
spurious noise.

The `margin` and `padding` shorthands are deliberately excluded — the
Chromium driver returns longhands per PRD §10.3, and checking both
shapes would emit two violations for the same logical issue.

The rule MUST skip a property when:

- the computed value parses as `auto`, `normal`, the empty string,
  `calc(...)`, `<n>em`, `<n>rem`, `<n>%`, or any other non-`px` shape;
- or `spacing.base_unit` is `0` (the rule is a no-op in that case).

At most one violation is emitted per `(selector, property)` pair.

## Why it matters

A spacing grid encodes a design decision about visual rhythm. Off-grid
values fragment that rhythm — buttons line up by chance, paddings drift
across templates, and visual review burns time on judgment calls a
deterministic check can answer in microseconds. Catching off-grid
values at lint time keeps the design system enforceable as the codebase
grows.

## Example violation

```json
{
  "rule_id": "spacing/grid-conformance",
  "severity": "warning",
  "message": "`html > body > div:nth-child(2)` has off-grid padding-top 5px; expected a multiple of 4px.",
  "selector": "html > body > div:nth-child(2)",
  "viewport": "desktop",
  "dom_order": 2,
  "fix": {
    "kind": {
      "kind": "css_property_replace",
      "property": "padding-top",
      "from": "5px",
      "to": "4px"
    },
    "description": "Snap `padding-top` to the nearest spacing-grid value (4px).",
    "confidence": "medium"
  },
  "doc_url": "https://plumb.aramhammoudeh.com/rules/spacing-grid-conformance"
}
```

## Configuration

`spacing.base_unit` is the only knob. Default is `4`.

```toml
[spacing]
base_unit = 4
```

The rule reads `config.spacing.base_unit` once per run. Setting it to
`0` disables the rule.

## Suppression

Disable the rule for an entire run:

```toml
[rules."spacing/grid-conformance"]
enabled = false
```

Bump or lower the severity:

```toml
[rules."spacing/grid-conformance"]
severity = "error"
```

`RuleOverride` accepts both `enabled` (default `true`) and an optional
`severity` of `info`, `warning`, or `error`. Severity remapping is
applied at the formatter layer.

## See also

- [`spacing/scale-conformance`](./spacing-scale-conformance.md) — the
  discrete-token sibling check.
- PRD §11.2 — spacing rules and the token model.
