# type/family-conformance

**Status:** active

**Default severity:** `warning`

## What it checks

For every node in the snapshot, the rule reads the computed
`font-family` value and splits it into a comma-separated list of
family names. Outer quotes are stripped from each entry. A violation
fires when none of the parsed families match any entry in
`type.families` (case-insensitive comparison).

The rule MUST skip a node when:

- the node has no computed `font-family` value;
- the computed value is empty or whitespace-only;
- or `type.families` is empty (the rule is a no-op).

## Why it matters

A type system defines which font families belong in a product. Using
an off-brand family (a system default fallback, a debug font, a
copy-pasted Google Fonts import that was never removed) undermines
visual consistency. This rule catches those before they ship.

## Example violation

```json
{
  "rule_id": "type/family-conformance",
  "severity": "warning",
  "message": "`html > body > p` uses font-family `\"Comic Sans MS\", cursive` which is not in type.families.",
  "selector": "html > body > p",
  "viewport": "desktop",
  "dom_order": 3,
  "fix": {
    "kind": {
      "kind": "description",
      "text": "Use one of the allowed font families: Inter, sans-serif."
    },
    "description": "Replace `font-family` with one of the allowed families (Inter, sans-serif).",
    "confidence": "medium"
  },
  "doc_url": "https://plumb.aramhammoudeh.com/rules/type-family-conformance"
}
```

## Configuration

`type.families` is the list of allowed font-family names. Default is
empty (the rule is a no-op).

```toml
[type]
families = ["Inter", "sans-serif"]
```

Matching is case-insensitive. If any family in the element's
`font-family` stack matches any entry in `type.families`, the element
passes.

## Suppression

Disable the rule for an entire run:

```toml
[rules."type/family-conformance"]
enabled = false
```

Bump or lower the severity:

```toml
[rules."type/family-conformance"]
severity = "info"
```

## See also

- [`type/scale-conformance`](./type-scale-conformance.md) — the
  sibling rule for font-size tokens.
- [`type/weight-conformance`](./type-weight-conformance.md) — the
  sibling rule for font-weight tokens.
- PRD §11.3 — type rules.
