# Rules — overview

Plumb's rules are the catalog of design-system checks the engine runs
against each page snapshot. Every rule has:

- A **stable id**, slash-separated (`<category>/<id>`).
- A **default severity** (`info`, `warning`, `error`).
- A **docs page** — the one you get from `plumb explain <id>`.

## Built-in rules

- [`a11y/touch-target`](./a11y-touch-target.md) — flags interactive
  elements smaller than `a11y.touch_target`.
- [`color/palette-conformance`](./color-palette-conformance.md) —
  flags element colors that aren't members of the configured palette.
- [`edge/near-alignment`](./edge-near-alignment.md) — flags element
  edges that almost-but-not-quite line up with sibling edges.
- [`radius/scale-conformance`](./radius-scale-conformance.md) — flags
  border-radius values that aren't members of `radius.scale`.
- [`sibling/height-consistency`](./sibling-height-consistency.md) —
  flags sibling elements in the same visual row whose heights drift
  from the row's median.
- [`spacing/grid-conformance`](./spacing-grid-conformance.md) — flags
  spacing values that aren't multiples of `spacing.base_unit`.
- [`spacing/scale-conformance`](./spacing-scale-conformance.md) —
  flags spacing values that aren't members of `spacing.scale`.
- [`type/scale-conformance`](./type-scale-conformance.md) — flags
  `font-size` values that aren't members of `type.scale`.

## Coming soon

The PRD lists more rules in the initial set; each will land with its
own docs page and a golden snapshot test.
