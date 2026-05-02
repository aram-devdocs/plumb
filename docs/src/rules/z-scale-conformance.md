# z/scale-conformance

**Status:** active

**Default severity:** `warning`

## What it checks

For every node in the snapshot, the rule reads the computed `z-index`
value and parses it as an `i32`. A violation fires when the parsed
value is not present in `z_index.scale`.

The rule MUST skip a node when:

- the node has no computed `z-index` value;
- the value is `auto` (case-insensitive);
- the value does not parse as an `i32`;
- or `z_index.scale` is empty (the rule is a no-op).

## Why it matters

Uncontrolled z-index values lead to stacking-context wars: one
component uses `z-index: 999`, another uses `9999`, and the
escalation never ends. A defined scale (e.g. 0, 10, 20, 50, 100)
keeps layering intentional and predictable.

## Example violation

```json
{
  "rule_id": "z/scale-conformance",
  "severity": "warning",
  "message": "`html > body > div.modal` has off-scale z-index 15; expected a value from z_index.scale.",
  "selector": "html > body > div.modal",
  "viewport": "desktop",
  "dom_order": 3,
  "fix": {
    "kind": {
      "kind": "css_property_replace",
      "property": "z-index",
      "from": "15",
      "to": "10"
    },
    "description": "Snap `z-index` to the nearest scale value (10).",
    "confidence": "medium"
  },
  "doc_url": "https://plumb.aramhammoudeh.com/rules/z-scale-conformance"
}
```

## Configuration

`z_index.scale` is the list of allowed z-index values. Default is
empty (the rule is a no-op).

```toml
[z_index]
scale = [0, 10, 20, 30, 50, 100]
```

The fix suggests the nearest scale value. Ties resolve toward lower
absolute value, then toward zero.

## Suppression

Disable the rule for an entire run:

```toml
[rules."z/scale-conformance"]
enabled = false
```

Bump or lower the severity:

```toml
[rules."z/scale-conformance"]
severity = "info"
```

## See also

- [`opacity/scale-conformance`](./opacity-scale-conformance.md) —
  the sibling rule for opacity tokens.
- PRD §11 — rule catalog.
