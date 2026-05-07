# opacity/scale-conformance

**Status:** active

**Default severity:** `warning`

## What it checks

For every node in the snapshot, the rule reads the computed `opacity`
value and parses it as an `f64`. A violation fires when no entry in
`opacity.scale` is within 0.005 of the parsed value.

The rule MUST skip a node when:

- the node has no computed `opacity` value;
- the value does not parse as an `f64`;
- or `opacity.scale` is empty (the rule is a no-op).

## Why it matters

Opacity values define the transparency vocabulary of a design system.
Arbitrary values like `0.35` or `0.87` create visual inconsistency
across hover states, disabled elements, and overlays. A defined scale
(e.g. 0, 0.25, 0.5, 0.75, 1.0) keeps transparency intentional.

## Example violation

```json
{
  "rule_id": "opacity/scale-conformance",
  "severity": "warning",
  "message": "`html > body > div.overlay` has off-scale opacity 0.35; expected a value from opacity.scale.",
  "selector": "html > body > div.overlay",
  "viewport": "desktop",
  "dom_order": 3,
  "fix": {
    "kind": {
      "kind": "css_property_replace",
      "property": "opacity",
      "from": "0.35",
      "to": "0.25"
    },
    "description": "Snap `opacity` to the nearest scale value (0.25).",
    "confidence": "medium"
  },
  "doc_url": "https://plumb.aramhammoudeh.com/rules/opacity-scale-conformance"
}
```

## Configuration

`opacity.scale` is the list of allowed opacity values in `[0.0, 1.0]`.
Default is empty (the rule is a no-op).

```toml
[opacity]
scale = [0.0, 0.25, 0.5, 0.75, 1.0]
```

The tolerance for matching is 0.005 — values within half a percent
of a scale entry pass. The fix suggests the nearest scale value by
absolute delta. Ties resolve to the lower value.

## Suppression

Disable the rule for an entire run:

```toml
[rules."opacity/scale-conformance"]
enabled = false
```

Bump or lower the severity:

```toml
[rules."opacity/scale-conformance"]
severity = "info"
```

## See also

- [`z/scale-conformance`](./z-scale-conformance.md) — the sibling
  rule for z-index tokens.
- [`shadow/scale-conformance`](./shadow-scale-conformance.md) — the
  sibling rule for box-shadow tokens.
- PRD §11 — rule catalog.
