# sibling/height-consistency

**Status:** active

**Default severity:** `info`

## What it checks

The rule looks for sibling elements that share a visual row but
disagree about how tall they are. The check has four phases:

1. **Group by parent.** Every node carries a `parent` `dom_order`.
   The rule groups nodes by that key. Nodes without a `Rect` are
   skipped — height clustering needs geometry.

2. **Cluster into visual rows.** Within each parent group, siblings
   walk in DOM order. A sibling joins the first existing row whose
   first member shares its top edge (within ±2 CSS px) AND overlaps
   it horizontally by at least 50% of the smaller width. Otherwise a
   new row opens. The 50% overlap rule keeps two stacked siblings
   that happen to share a top from being treated as row mates.

3. **Fall back when row clustering fails.** If every sibling lands
   in its own row (e.g. a vertical stack, an absolute-positioned
   layout, transforms that confuse the geometry), the rule treats
   the whole DOM-sibling group as one logical row. The size-≥-2
   gate at the next step still keeps singletons quiet.

4. **Median + deviation.** For each row of size ≥ 2, take the median
   height. Even-count rows pick the lower of the two middle values
   — an integer-only choice that avoids floating-point math. Any
   element whose height differs from the median by more than 4 CSS
   px fires a violation.

The rule emits at most one violation per offending node per
viewport. Sibling iteration uses `parent` `dom_order` only — nested
descendants are picked up when the engine walks their own parent
group.

### Worked example

Three cards sit in a row at `top = 0` with widths 200, 200, 200 and
heights 100, 100, 130. They cluster into one row — every pair
satisfies the top-tolerance and overlap tests. The median height is
100; the third card's 30 px drift exceeds the 4 px threshold and
triggers a violation on the third card only.

A second container holds three buttons stacked vertically. No two
buttons share a `top`, so the row clusterer produces three
singletons. The fallback kicks in: all three buttons become one
fallback row. Their heights are 32, 32, 48; the median is 32; the
third button's 16 px drift triggers a violation on the third button.

## Why it matters

Card grids and toolbar rows that are *almost* the same height look
sloppier than rows that are clearly different. The 4 px threshold
is loose enough to swallow subpixel rounding and tight enough to
catch a `padding: 12px` vs `padding: 16px` mismatch on otherwise
matching cards.

The fix is emitted at `confidence: low` — Plumb cannot know whether
to bump `min-height`, change the inner padding, or accept the drift
as design intent. The description names the row's median; a human
picks the change.

## Example violation

```json
{
  "rule_id": "sibling/height-consistency",
  "severity": "info",
  "message": "`html > body > div.row > div:nth-child(3)` is 130px tall; its row median is 100px (30px drift).",
  "selector": "html > body > div.row > div:nth-child(3)",
  "viewport": "desktop",
  "rect": { "x": 440, "y": 1, "width": 200, "height": 130 },
  "dom_order": 5,
  "fix": {
    "kind": {
      "kind": "description",
      "text": "Match the row's height (100px) by adjusting `height` / `min-height` or aligning the inner content. Drift: 30px."
    },
    "description": "Bring `html > body > div.row > div:nth-child(3)` in line with its row's height (100px).",
    "confidence": "low"
  },
  "doc_url": "https://plumb.aramhammoudeh.com/rules/sibling-height-consistency",
  "metadata": {
    "rendered_height_px": 130,
    "row_median_height_px": 100,
    "row_size": 3,
    "deviation_px": 30
  }
}
```

## Configuration

The rule has no `plumb.toml` knobs today. The thresholds are baked
into the rule and pinned by a unit test:

- Top-edge tolerance: 2 CSS px.
- Horizontal overlap: 50% of the smaller width.
- Height-deviation threshold: 4 CSS px.

Future revisions MAY expose these under a `sibling.height` section
of the config — see PRD §11.6.

## Suppression

Disable the rule for an entire run:

```toml
[rules."sibling/height-consistency"]
enabled = false
```

Bump or lower the severity:

```toml
[rules."sibling/height-consistency"]
severity = "warning"
```

For a one-off card that is meant to be taller (a hero, a featured
tile), suppression at the `[rules]` level is the right tool today.
Per-element suppression follows the standard `RuleOverride` model.

## See also

- [`edge/near-alignment`](./edge-near-alignment.md) — the rule that
  catches sibling edges that almost-but-not-quite line up.
- PRD §11.6 — the sibling-relationship rule family.
