# sibling/padding-consistency

**Status:** active

**Default severity:** `info`

## What it checks

The rule groups nodes by their parent `dom_order` and, within each
group of two or more siblings, checks the four padding longhands
(`padding-top`, `padding-right`, `padding-bottom`, `padding-left`)
independently. For each property, it computes the median value among
siblings that have a parseable pixel value for that property. Any
sibling whose value deviates from the median by more than 4 CSS
pixels fires a violation.

The rule MUST skip:

- sibling groups smaller than 2 (nothing to compare);
- siblings where the property value does not parse as a pixel length.

Even-count medians pick the lower of the two middle values, matching
the tie-break used by `sibling/height-consistency`.

## Why it matters

Card grids, nav bars, and list items that share a parent but disagree
on padding look sloppy. The 4px threshold catches real mismatches
(12px vs 24px) while ignoring subpixel rendering differences. The
fix is emitted at `confidence: low` because Plumb cannot know whether
to change the outlier or update the siblings.

## Example violation

```json
{
  "rule_id": "sibling/padding-consistency",
  "severity": "info",
  "message": "`html > body > div.cards > div:nth-child(3)` has padding-top 28px; sibling median is 16px (12px drift).",
  "selector": "html > body > div.cards > div:nth-child(3)",
  "viewport": "desktop",
  "dom_order": 5,
  "fix": {
    "kind": {
      "kind": "description",
      "text": "Match sibling padding-top (16px) to keep padding consistent. Drift: 12px."
    },
    "description": "Bring `html > body > div.cards > div:nth-child(3)` padding-top in line with its siblings (16px).",
    "confidence": "low"
  },
  "doc_url": "https://plumb.aramhammoudeh.com/rules/sibling-padding-consistency",
  "metadata": {
    "property": "padding-top",
    "rendered_padding_px": 28.0,
    "sibling_median_px": 16.0,
    "deviation_px": 12
  }
}
```

## Configuration

The rule has no `plumb.toml` knobs today. The deviation threshold
(4 CSS px) is baked into the rule and pinned by a unit test.

## Suppression

Disable the rule for an entire run:

```toml
[rules."sibling/padding-consistency"]
enabled = false
```

Bump or lower the severity:

```toml
[rules."sibling/padding-consistency"]
severity = "warning"
```

## See also

- [`sibling/height-consistency`](./sibling-height-consistency.md) —
  the sibling rule for row-height drift.
- PRD §11.6 — sibling-relationship rules.
