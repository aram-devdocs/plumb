# edge/near-alignment

**Status:** active

**Default severity:** `info`

## What it checks

The rule looks for sibling elements whose edges *almost* line up but
miss by one or two pixels. It runs the same clustering pass on each
of the four edge axes — `left`, `right`, `top`, `bottom` — and emits
one violation per `(node, axis)` near miss.

Per parent group of siblings (with rects):

1. Sort the group's edge values along the current axis.
2. Walk the sorted list. An edge joins the active cluster when it is
   within `alignment.tolerance_px` of the cluster's lowest member;
   otherwise it opens a new cluster.
3. For each cluster of ≥ 2 members, compute the integer centroid
   (the rounded mean).
4. For each cluster member, compute `delta = |edge - centroid|`.
   - `delta == 0` → pixel-perfect; the rule stays silent.
   - `0 < delta ≤ tolerance_px` → near-miss; emit a violation.
   - `delta > tolerance_px` is impossible by construction — the
     cluster wouldn't have absorbed the edge in the first place.

The rule MUST skip:

- siblings without rects (off-screen, hidden, not yet laid out);
- groups of size < 2;
- clusters of size < 2 (a lone edge has no neighbour to drift from);
- pixel-perfect alignments (`delta == 0`);
- runs where `alignment.tolerance_px == 0` (no near-miss is possible).

A node may be flagged once per axis; a card whose left and bottom
both drift will produce two violations.

## Why it matters

Near-aligned edges are the visual signature of the design system
losing focus. Three cards whose left edges sit at `x = 0, 1, 2` look
*almost* aligned and *just* sloppy — the eye notices, even when
nobody can name what's off. The rule is the deterministic check that
catches the drift before review.

The fix is emitted at `confidence: low` — Plumb cannot know which
edge is canonical (the centroid is a best-guess) or whether the
adjustment should land on `margin`, `padding`, `transform`, or the
parent's flex track. The description names the centroid; a human
picks the change.

## Example violation

```json
{
  "rule_id": "edge/near-alignment",
  "severity": "info",
  "message": "`html > body > div:nth-child(1)` left edge is 0px; 3 sibling(s) cluster at 1px (1px drift, tolerance 3px).",
  "selector": "html > body > div:nth-child(1)",
  "viewport": "desktop",
  "rect": { "x": 0, "y": 50, "width": 100, "height": 80 },
  "dom_order": 2,
  "fix": {
    "kind": {
      "kind": "description",
      "text": "Snap the left edge to 1px to match the sibling cluster."
    },
    "description": "Align `html > body > div:nth-child(1)`'s left edge with its 3-member cluster (1px).",
    "confidence": "low"
  },
  "doc_url": "https://plumb.aramhammoudeh.com/rules/edge-near-alignment",
  "metadata": {
    "axis": "left",
    "edge_px": 0,
    "cluster_centroid_px": 1,
    "delta_px": 1,
    "cluster_size": 3,
    "tolerance_px": 3
  }
}
```

## Configuration

`alignment.tolerance_px` controls the cluster width. Default is 3 CSS
px:

```toml
[alignment]
tolerance_px = 3
```

Setting it to `0` disables the rule. Bumping it widens the
near-miss net (and makes the rule noisier).

## Suppression

Disable the rule for an entire run:

```toml
[rules."edge/near-alignment"]
enabled = false
```

Bump or lower the severity:

```toml
[rules."edge/near-alignment"]
severity = "warning"
```

For a single intentional offset (a hand-tuned hero, a deliberately
asymmetric callout), suppression at the `[rules]` level is the right
tool today. Per-element suppression follows the standard
`RuleOverride` model.

## See also

- [`sibling/height-consistency`](./sibling-height-consistency.md) —
  the rule that catches sibling height drift.
- PRD §11.4 — the alignment rule family.
