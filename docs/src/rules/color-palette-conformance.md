# color/palette-conformance

**Status:** active

**Default severity:** `warning`

## What it checks

For every node in the snapshot, the rule reads each of these computed
styles and parses the value as a CSS color:

- `color`
- `background-color`
- `border-top-color`, `border-right-color`, `border-bottom-color`, `border-left-color`
- `outline-color`

Each parsed color is converted to CIE Lab (D65) and compared against
every entry in `color.tokens` via CIEDE2000 (ΔE00). A property fires
a violation when the smallest distance to any token exceeds
`color.delta_e_tolerance` (default `2.0`).

The rule MUST skip a property when:

- the value parses as `transparent` or has alpha `0`;
- the value is not one of the supported shapes — `rgb(...)`,
  `rgba(...)`, `#rgb`, `#rrggbb`, `#rgba`, `#rrggbbaa` (HSL, named
  colors other than `transparent`, and `color()` resolve through
  Chromium to one of these in real snapshots);
- or `color.tokens` is empty (the rule is a no-op in that case
  rather than flagging every color as off-palette).

For colors with `0 < alpha < 1`, the rule walks up the DOM ancestor
chain looking for the closest `background-color` with `alpha == 1.0`
and composites the foreground over it (Porter–Duff "source over" in
linear-light sRGB). When no fully-opaque ancestor declares a
`background-color`, the rule defaults to `#ffffff` — the User Agent
default. The composited result is the value used for the ΔE00
measurement, so a translucent overlay is judged against what the
user actually sees.

At most one violation is emitted per `(node, property)` pair.

## Why it matters

A palette is the design system's vocabulary for color. Off-palette
values introduce vocabulary the system did not sanction — a slightly
warmer red here, a slightly grayer text color there — and the
cumulative drift erodes the system's identity. CIEDE2000 is the
standard perceptual color-difference metric: a tolerance of `2.0` is
the "just noticeable difference" threshold for trained observers, so
a violation reads as "a designer would see this is not the right
color."

The rule's blended-background semantics matter for translucent UI
chrome — a half-opaque "muted" foreground that lands on a dark
background renders very differently from the same color on white,
and the rule judges it where it actually lives in the rendered tree.

## Example violation

```json
{
  "rule_id": "color/palette-conformance",
  "severity": "warning",
  "message": "`html > body > div:nth-child(3)` has off-palette color rgb(255, 0, 153); nearest token is `white` (#ffffff).",
  "selector": "html > body > div:nth-child(3)",
  "viewport": "desktop",
  "dom_order": 4,
  "fix": {
    "kind": {
      "kind": "css_property_replace",
      "property": "color",
      "from": "rgb(255, 0, 153)",
      "to": "#ffffff"
    },
    "description": "Snap `color` to the nearest palette token `white` (#ffffff).",
    "confidence": "medium"
  },
  "doc_url": "https://plumb.aramhammoudeh.com/rules/color-palette-conformance",
  "metadata": {
    "color": "rgb(255, 0, 153)",
    "nearest_token": "white",
    "nearest_token_hex": "#ffffff",
    "delta_e": 43.071,
    "delta_e_tolerance": 2.0
  }
}
```

The `metadata` block carries the ΔE00 value, the active tolerance,
and the nearest token's name and hex so downstream tooling can
render a richer suggestion than the bare `Fix` payload.

## Configuration

`color.tokens` is the list of allowed colors as `name → hex` pairs.
Slash-delimited names (`"bg/canvas"`) act as informal namespaces.
Default is empty (the rule is a no-op).

`color.delta_e_tolerance` controls how strict the match is. Default
is `2.0`. Lower values are stricter; values above `5.0` admit colors
that most designers would call "different."

```toml
[color]
delta_e_tolerance = 2.0

[color.tokens]
"bg/canvas" = "#ffffff"
"fg/primary" = "#0b7285"
"fg/muted" = "#495057"
```

The rule converts every token to CIE Lab once per `check` call
(never per node) and picks the nearest token by smallest CIEDE2000
distance when emitting a fix. Ties resolve to the first-declared
token (deterministic given `IndexMap` insertion order).

## Suppression

Disable the rule for an entire run:

```toml
[rules."color/palette-conformance"]
enabled = false
```

Bump or lower the severity:

```toml
[rules."color/palette-conformance"]
severity = "info"
```

`RuleOverride` accepts both `enabled` (default `true`) and an
optional `severity` of `info`, `warning`, or `error`. Severity
remapping is applied at the formatter layer.

## See also

- [`spacing/scale-conformance`](./spacing-scale-conformance.md) — the
  same allow-list shape applied to the spacing scale.
- [`type/scale-conformance`](./type-scale-conformance.md) — the same
  shape for `font-size`.
- PRD §11.3 — color rules and the token model.
