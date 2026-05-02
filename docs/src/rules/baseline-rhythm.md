# baseline/rhythm

**Status:** active

**Default severity:** `warning`

## What it checks

For every text-bearing element (`p`, `span`, `h1`--`h6`, `a`, `li`,
`label`, `button`, `input`, `textarea`, `select`, and others) with a
`font-size` computed style and a bounding rect, the rule approximates
the element's typographic baseline position:

1. Parses `font-size` from computed styles.
2. Computes cap-height: uses `rhythm.cap_height_fallback_px` when set,
   otherwise `font_size * 0.7` (typical Latin cap-height ratio).
3. Parses `line-height` (falls back to `font_size * 1.2` when missing
   or `normal`).
4. Calculates `baseline_y = rect.y + half_leading + cap_height`, where
   `half_leading = (line_height - font_size) / 2`.
5. Checks distance from `baseline_y` to the nearest multiple of
   `rhythm.base_line_px`. If distance exceeds `rhythm.tolerance_px`,
   a violation is emitted.

The rule is a no-op when `rhythm.base_line_px` is `0`.

## Why it matters

Vertical rhythm keeps text across a page visually aligned to a shared
grid, giving the layout a consistent cadence that readers perceive as
orderly even without consciously noticing. Off-rhythm baselines cause
adjacent columns of text to drift apart vertically, making the page
feel unpolished. Catching misalignments at lint time avoids slow visual
QA passes.

## Example violation

```json
{
  "rule_id": "baseline/rhythm",
  "severity": "warning",
  "message": "`html > body > p:nth-child(2)` baseline at 20.2px is 3.8px off the 24px rhythm grid.",
  "selector": "html > body > p:nth-child(2)",
  "viewport": "desktop",
  "dom_order": 3,
  "fix": {
    "kind": {
      "kind": "description",
      "text": "Adjust line-height or margin-top so the baseline aligns to the nearest 24px grid line (24px)."
    },
    "description": "Shift baseline from 20.2px to 24px to restore vertical rhythm.",
    "confidence": "low"
  },
  "doc_url": "https://plumb.aramhammoudeh.com/rules/baseline-rhythm",
  "metadata": {
    "baseline_y": 20.2,
    "nearest_grid_y": 24.0,
    "distance_px": 3.8
  }
}
```

## Configuration

Three knobs under `[rhythm]` in `plumb.toml`:

```toml
[rhythm]
base_line_px = 24          # grid interval; 0 disables the rule
tolerance_px = 2           # how far off-grid before firing
cap_height_fallback_px = 0 # explicit cap-height; 0 = estimate from font-size
```

Setting `base_line_px = 0` disables the rule entirely (no violations
emitted regardless of element positions).

## Suppression

Disable the rule for an entire run:

```toml
[rules."baseline/rhythm"]
enabled = false
```

Bump or lower the severity:

```toml
[rules."baseline/rhythm"]
severity = "error"
```

## See also

- [`spacing/grid-conformance`](./spacing-grid-conformance.md) — the
  horizontal spacing-grid sibling.
- PRD SS11.3 -- vertical rhythm and baseline alignment.
