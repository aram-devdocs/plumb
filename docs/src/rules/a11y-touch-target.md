# a11y/touch-target

**Status:** active

**Default severity:** `warning`

## What it checks

For every interactive node in the snapshot, the rule reads the
rendered bounding rect (`Rect`) and compares it to the configured
minimum target size:

- `width  ≥ a11y.touch_target.min_width_px`
- `height ≥ a11y.touch_target.min_height_px`

A node fires a violation when *either* axis is below its threshold.
Defaults are 24×24 CSS pixels — the minimum required by
[WCAG 2.5.8 *Target Size (Minimum)*](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html).

A node is treated as interactive when:

- `tag` is `button`, `select`, or `textarea`; or
- `tag` is `a` and the node has an `href` attribute (per the HTML
  spec, a bare `<a>` with no `href` is non-interactive); or
- `tag` is `input` with a button-shaped `type` (`button`, `submit`,
  `reset`, `image`, `checkbox`, `radio`); or
- the node carries `role="button"`.

The rule MUST skip a node when:

- it is not interactive by the rules above;
- its `Rect` is `None` (off-screen, hidden, or not yet laid out);
- both `min_width_px` and `min_height_px` are `0` (the rule is a
  no-op in that case).

At most one violation is emitted per offending node per viewport.
The violation's `metadata` records the rendered and minimum sizes for
formatter use.

## Why it matters

Tiny tap targets are unreachable for users with motor impairments and
miserable on touchscreens. WCAG 2.5.8 sets 24×24 CSS pixels as the
floor. Plumb checks rendered geometry — the visible hit area — rather
than the CSS the author wrote, because a `padding: 12px` button can
end up smaller than expected once flex squeeze or text-shrink kicks
in.

The fix is emitted at `confidence: low` — Plumb can't know whether to
adjust `min-width`, `padding`, or the surrounding layout. The
description names the target dimensions; a human picks the change.

## Example violation

```json
{
  "rule_id": "a11y/touch-target",
  "severity": "warning",
  "message": "`html > body > button:nth-child(2)` is 16×16px; WCAG 2.5.8 wants at least 24×24px for interactive targets.",
  "selector": "html > body > button:nth-child(2)",
  "viewport": "desktop",
  "rect": { "x": 0, "y": 40, "width": 16, "height": 16 },
  "dom_order": 3,
  "fix": {
    "kind": {
      "kind": "description",
      "text": "Enlarge the hit area to at least 24×24px (CSS pixels). Padding or `min-width` / `min-height` typically does the trick without changing the visual size."
    },
    "description": "Bring `html > body > button:nth-child(2)` up to the minimum touch-target size (24×24px).",
    "confidence": "low"
  },
  "doc_url": "https://plumb.aramhammoudeh.com/rules/a11y-touch-target",
  "metadata": {
    "rendered_width_px": 16,
    "rendered_height_px": 16,
    "min_width_px": 24,
    "min_height_px": 24
  }
}
```

## Configuration

`a11y.touch_target` carries the two thresholds. Both default to 24.

```toml
[a11y.touch_target]
min_width_px  = 24
min_height_px = 24
```

Bump the thresholds for an iOS-aligned 44×44 target:

```toml
[a11y.touch_target]
min_width_px  = 44
min_height_px = 44
```

Either knob set to `0` disables that axis. Setting both to `0`
disables the rule.

## Suppression

Disable the rule for an entire run:

```toml
[rules."a11y/touch-target"]
enabled = false
```

Bump or lower the severity:

```toml
[rules."a11y/touch-target"]
severity = "error"
```

Per-element suppression follows the standard `RuleOverride` model.

## See also

- [WCAG 2.5.8 *Target Size (Minimum)*](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html).
- PRD §11.7 — accessibility rules.
