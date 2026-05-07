# type/weight-conformance

**Status:** active

**Default severity:** `warning`

## What it checks

For every node in the snapshot, the rule reads the computed
`font-weight` value and parses it as a `u16`. A violation fires when
the parsed weight is not present in `type.weights`.

The rule MUST skip a node when:

- the node has no computed `font-weight` value;
- the value does not parse as a `u16` (e.g. `bold`, `normal`);
- or `type.weights` is empty (the rule is a no-op).

Chromium resolves keyword weights (`bold` → `700`, `normal` → `400`)
in `getComputedStyle`, so in practice the rule sees numeric values.

## Why it matters

Design systems restrict font weights to a small set (often 400 and
700, or a wider range for variable fonts). An off-scale weight like
`450` or `550` may render differently across browsers and fonts,
creating inconsistency that is hard to spot by eye.

## Example violation

```json
{
  "rule_id": "type/weight-conformance",
  "severity": "warning",
  "message": "`html > body > span` has off-scale font-weight 450; expected a value from type.weights.",
  "selector": "html > body > span",
  "viewport": "desktop",
  "dom_order": 3,
  "fix": {
    "kind": {
      "kind": "css_property_replace",
      "property": "font-weight",
      "from": "450",
      "to": "400"
    },
    "description": "Snap `font-weight` to the nearest type-scale weight (400).",
    "confidence": "medium"
  },
  "doc_url": "https://plumb.aramhammoudeh.com/rules/type-weight-conformance"
}
```

## Configuration

`type.weights` is the list of allowed font-weight values. Default is
empty (the rule is a no-op).

```toml
[type]
weights = [100, 300, 400, 500, 700, 900]
```

The fix suggests the nearest allowed weight by absolute delta. Ties
resolve to the lower weight.

## Suppression

Disable the rule for an entire run:

```toml
[rules."type/weight-conformance"]
enabled = false
```

Bump or lower the severity:

```toml
[rules."type/weight-conformance"]
severity = "info"
```

## See also

- [`type/family-conformance`](./type-family-conformance.md) — the
  sibling rule for font-family tokens.
- [`type/scale-conformance`](./type-scale-conformance.md) — the
  sibling rule for font-size tokens.
- PRD §11.3 — type rules.
