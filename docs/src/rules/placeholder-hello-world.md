# placeholder/hello-world

**Status:** walking-skeleton placeholder. Removed when the first real rule ships.

**Default severity:** `warning`

## What it checks

A single deterministic condition: any `<body>` element with a computed
`padding` value of `13px` in the snapshot.

## Why it exists

This rule is the end-to-end wiring test for Plumb's rule engine,
formatters, CLI, and MCP server. It proves the pipeline carries one
violation from `plumb-core::engine::run` all the way out to stdout, JSON,
SARIF, and the MCP-compact structured block.

It does **not** represent a real design-system concern. Real rules
replace it over PRs #3–#10.

## Example violation

```json
{
  "rule_id": "placeholder/hello-world",
  "severity": "warning",
  "message": "`body` has off-scale padding 13px; expected a value from the spacing token set.",
  "selector": "html > body",
  "viewport": "desktop",
  "fix": {
    "kind": { "kind": "css_property_replace", "property": "padding", "from": "13px", "to": "16px" },
    "description": "Snap `body` padding to the nearest spacing token (16px).",
    "confidence": "medium"
  }
}
```

## Suppression

This rule is removed in a subsequent PR — no suppression guidance applies.

## See also

- [`docs/local/prd.md` §11.6](../../local/prd.md) — the "how to add a rule" walkthrough.
