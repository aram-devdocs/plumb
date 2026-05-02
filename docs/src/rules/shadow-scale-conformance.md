# shadow/scale-conformance

**Status:** active

**Default severity:** `warning`

## What it checks

For every node in the snapshot, the rule reads the computed
`box-shadow` value and compares it against the entries in
`shadow.scale`. The comparison is an exact string match (after
trimming whitespace). A violation fires when the computed value does
not match any scale entry.

The rule MUST skip a node when:

- the node has no computed `box-shadow` value;
- the value is `none` (case-insensitive);
- or `shadow.scale` is empty (the rule is a no-op).

## Why it matters

Box shadows are one of the most visually inconsistent properties in
a design system. Different blur radii, spread values, and color
opacities create a muddy visual hierarchy. Restricting shadows to a
named set of tokens keeps elevation consistent across components.

## Example violation

```json
{
  "rule_id": "shadow/scale-conformance",
  "severity": "warning",
  "message": "`html > body > div.card` has off-scale box-shadow `0px 8px 24px rgba(0, 0, 0, 0.3)`; expected a value from shadow.scale.",
  "selector": "html > body > div.card",
  "viewport": "desktop",
  "dom_order": 3,
  "fix": {
    "kind": {
      "kind": "description",
      "text": "The box-shadow value `0px 8px 24px rgba(0, 0, 0, 0.3)` is not in the allowed shadow scale."
    },
    "description": "Replace `box-shadow` with one of the allowed shadow tokens.",
    "confidence": "medium"
  },
  "doc_url": "https://plumb.aramhammoudeh.com/rules/shadow-scale-conformance"
}
```

## Configuration

`shadow.scale` is the list of allowed `box-shadow` expressions as
returned by `getComputedStyle`. Default is empty (the rule is a
no-op).

```toml
[shadow]
scale = [
  "0px 1px 2px rgba(0, 0, 0, 0.05)",
  "0px 2px 4px rgba(0, 0, 0, 0.1)",
  "0px 4px 8px rgba(0, 0, 0, 0.15)",
]
```

Each entry should match the exact computed-style output from
Chromium. Use `getComputedStyle(el).boxShadow` in DevTools to get
the canonical form.

## Suppression

Disable the rule for an entire run:

```toml
[rules."shadow/scale-conformance"]
enabled = false
```

Bump or lower the severity:

```toml
[rules."shadow/scale-conformance"]
severity = "info"
```

## See also

- [`opacity/scale-conformance`](./opacity-scale-conformance.md) —
  the sibling rule for opacity tokens.
- PRD §11 — rule catalog.
