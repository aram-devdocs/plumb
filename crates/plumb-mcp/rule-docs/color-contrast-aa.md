# color/contrast-aa

**Status:** active

**Default severity:** `warning`

## What it checks

For every node in the snapshot, the rule reads these computed styles:

- `color`
- `font-size`
- `font-weight` (optional; only needed for the bold large-text cutoff)
- `background-color` on the node and its ancestors

The rule parses the foreground color, resolves the node's effective
background by compositing `background-color` layers up the DOM ancestor
chain, then computes the WCAG contrast ratio from relative luminance.
If the foreground itself has alpha, it is composited over the resolved
background before the ratio is measured.

WCAG 2.1 AA uses two floors:

- normal text: `4.5:1`
- large text: `3.0:1`

Plumb classifies a node as large text when its computed `font-size` is
at least `24px`, or at least `18.667px` with computed `font-weight >= 700`.
That matches WCAG's `18pt` regular / `14pt` bold thresholds in CSS pixels.

The rule MUST skip a node when:

- `color` is missing, transparent, or not parseable as `rgb(...)`,
  `rgba(...)`, `#rgb`, `#rrggbb`, `#rgba`, or `#rrggbbaa`;
- `font-size` is missing, not parseable as a pixel value, or not
  strictly positive;
- the background chain contains unsupported color syntax only, in which
  case the rule falls back to `#ffffff`, the User Agent default.

At most one violation is emitted per node per viewport.

## Why it matters

Contrast failures are hard to spot in a token audit because the problem
is relational: a text color can be valid on one surface and unreadable
on another. WCAG AA is the baseline accessibility contract for body copy
and large headings, and the large-text carveout matters because a ratio
that fails 16px body text may still be readable at 24px.

Using the nearest composited background keeps the rule grounded in what
the user actually sees. A muted foreground over a white card is a
different accessibility outcome from the same foreground over a dark
panel.

## Example violation

```json
{
  "rule_id": "color/contrast-aa",
  "severity": "warning",
  "message": "`html > body > div:nth-child(2)` has contrast ratio 4.478:1; WCAG 2.1 AA requires at least 4.500:1 for normal text.",
  "selector": "html > body > div:nth-child(2)",
  "viewport": "desktop",
  "dom_order": 3,
  "fix": {
    "kind": {
      "kind": "description",
      "text": "Increase the foreground/background contrast to at least 4.500:1 for this normal text."
    },
    "description": "Raise `html > body > div:nth-child(2)` to the WCAG 2.1 AA contrast floor.",
    "confidence": "low"
  },
  "doc_url": "https://plumb.aramhammoudeh.com/rules/color-contrast-aa",
  "metadata": {
    "contrast_ratio": 4.478,
    "required_ratio": 4.5,
    "font_size_px": 16.0,
    "large_text": false,
    "foreground_color": "rgb(119, 119, 119)"
  }
}
```

The `metadata` block carries the measured ratio, the active floor, and
the size-class inputs so downstream tools can explain why the node was
treated as normal or large text.

## Configuration

The rule has no required config. Its default behavior is fixed WCAG 2.1
AA: `4.5:1` for normal text and `3.0:1` for large text.

`a11y.min_contrast_ratio`, when set, acts as a stricter global floor.
It can raise the threshold above the AA defaults; it does not lower them.

```toml
[a11y]
min_contrast_ratio = 7.0
```

That example raises both normal and large text to `7.0:1`.

## Suppression

Disable the rule for an entire run:

```toml
[rules."color/contrast-aa"]
enabled = false
```

Bump or lower the severity:

```toml
[rules."color/contrast-aa"]
severity = "error"
```

`RuleOverride` accepts both `enabled` (default `true`) and an optional
`severity` of `info`, `warning`, or `error`. Severity remapping is
applied at the formatter layer.

## See also

- [`color/palette-conformance`](./color-palette-conformance.md) — checks
  whether the colors themselves are on the configured palette.
- [`type/scale-conformance`](./type-scale-conformance.md) — keeps text
  size on the system scale.
- [`a11y/touch-target`](./a11y-touch-target.md) — the other shipped
  accessibility rule.
- PRD §11.2 — built-in rule catalog.
